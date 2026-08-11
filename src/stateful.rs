//! Transactional adapters for bounded production state.
//!
//! The adapters deliberately support finite, declared resources. They refuse
//! snapshots that exceed their bounds or cannot be shown to be stable.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};

use futures_util::TryStreamExt;
use object_store::{
    ObjectStore, ObjectStoreExt, PutPayload, aws::AmazonS3Builder, path::Path as ObjectPath,
};
use postgres::{Client, IsolationLevel, NoTls};
use redis::{Commands, Connection};
use serde::{Deserialize, Serialize};

use crate::canonical::{bytes_digest, digest};
use crate::model::Error;
use crate::protocol::RecoveryResult;

pub const MAX_STATE_ITEMS: usize = 10_000;
pub const MAX_STATE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    PostgreSql,
    S3Compatible,
    RedisStream,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StateSchema {
    pub backend: BackendKind,
    pub resource: String,
    pub definition: String,
    pub invariants: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StateItem {
    pub key: String,
    pub fields: BTreeMap<String, String>,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StateSnapshot {
    pub version: String,
    pub schema: StateSchema,
    pub consistency: String,
    pub items: Vec<StateItem>,
    pub schema_sha256: String,
    pub data_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MigrationPlan {
    pub version: String,
    pub backend: BackendKind,
    pub source_schema_sha256: String,
    pub target_resource: String,
    pub operations: Vec<String>,
    pub rollback_strategy: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RestoreReceipt {
    pub backend: BackendKind,
    pub target_resource: String,
    pub snapshot_sha256: String,
    pub migration_sha256: String,
    pub verified_data_sha256: String,
    pub rollback_token: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActivationStateBinding {
    pub backend: BackendKind,
    pub resource: String,
    pub schema_sha256: String,
    pub snapshot_sha256: String,
    pub migration_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActivationPlan {
    pub version: String,
    pub candidate_sha256: String,
    pub certificate_sha256: String,
    pub service_manifest_sha256: String,
    pub states: Vec<ActivationStateBinding>,
    pub plan_sha256: String,
}

pub trait TransactionalStateAdapter {
    fn discover(&mut self) -> Result<StateSchema, Error>;
    fn snapshot(&mut self) -> Result<StateSnapshot, Error>;
    fn plan(&mut self, snapshot: &StateSnapshot, target: &str) -> Result<MigrationPlan, Error>;
    fn restore(
        &mut self,
        snapshot: &StateSnapshot,
        plan: &MigrationPlan,
    ) -> Result<RestoreReceipt, Error>;
    fn verify(&mut self, snapshot: &StateSnapshot, target: &str) -> Result<(), Error>;
    fn rollback(&mut self, receipt: &RestoreReceipt) -> Result<(), Error>;
}

pub fn bind_activation_plan(
    recovery: &RecoveryResult,
    service_manifest_sha256: &str,
    states: &[(&StateSnapshot, &MigrationPlan)],
) -> Result<ActivationPlan, Error> {
    let RecoveryResult::Certified {
        candidate,
        certificate,
        ..
    } = recovery
    else {
        return Err(invalid("activation requires a certified recovery"));
    };
    require_digest(service_manifest_sha256)?;
    let certificate_value = serde_json::to_value(certificate)?;
    if certificate_value
        .get("service_manifest_digest")
        .and_then(serde_json::Value::as_str)
        != Some(service_manifest_sha256)
    {
        return Err(invalid(
            "activation service manifest is not bound to the recovery certificate",
        ));
    }
    if states.is_empty() || states.len() > 16 {
        return Err(invalid("activation requires one to sixteen state bindings"));
    }
    let mut bindings = Vec::with_capacity(states.len());
    let mut unique = BTreeSet::new();
    for (snapshot, migration) in states {
        validate_snapshot(snapshot)?;
        validate_plan(snapshot, migration)?;
        let identity = (
            snapshot.schema.backend.clone(),
            migration.target_resource.clone(),
        );
        if !unique.insert(identity) {
            return Err(invalid("activation state resources must be unique"));
        }
        bindings.push(ActivationStateBinding {
            backend: snapshot.schema.backend.clone(),
            resource: migration.target_resource.clone(),
            schema_sha256: snapshot.schema_sha256.clone(),
            snapshot_sha256: digest(snapshot)?,
            migration_sha256: digest(migration)?,
        });
    }
    bindings.sort_by(|left, right| {
        (&left.backend, &left.resource).cmp(&(&right.backend, &right.resource))
    });
    let mut plan = ActivationPlan {
        version: "activation-plan-v1".into(),
        candidate_sha256: digest(candidate.as_ref())?,
        certificate_sha256: digest(certificate)?,
        service_manifest_sha256: service_manifest_sha256.into(),
        states: bindings,
        plan_sha256: String::new(),
    };
    plan.plan_sha256 = digest(&plan)?;
    Ok(plan)
}

pub struct PostgresAdapter {
    client: Client,
    source_schema: String,
}

#[derive(Deserialize, Serialize)]
struct PgSchema {
    tables: Vec<PgTable>,
}
#[derive(Deserialize, Serialize)]
struct PgTable {
    name: String,
    columns: Vec<PgColumn>,
    constraints: Vec<PgConstraint>,
}
#[derive(Deserialize, Serialize)]
struct PgColumn {
    name: String,
    data_type: String,
    nullable: bool,
}
#[derive(Deserialize, Serialize)]
struct PgConstraint {
    name: String,
    definition: String,
}

impl PostgresAdapter {
    pub fn connect(connection: &str, source_schema: &str) -> Result<Self, Error> {
        validate_identifier(source_schema)?;
        Ok(Self {
            client: Client::connect(connection, NoTls).map_err(backend)?,
            source_schema: source_schema.into(),
        })
    }

    fn snapshot_schema(&mut self, schema: &str) -> Result<StateSchema, Error> {
        let rows = self.client.query(
            "SELECT table_name, column_name, data_type, is_nullable, ordinal_position FROM information_schema.columns WHERE table_schema=$1 ORDER BY table_name, ordinal_position",
            &[&schema],
        ).map_err(backend)?;
        if rows.is_empty() {
            return Err(invalid("PostgreSQL schema has no discoverable tables"));
        }
        let mut tables = BTreeMap::<String, PgTable>::new();
        for row in rows {
            let data_type: String = row.get(2);
            if !matches!(
                data_type.as_str(),
                "bigint" | "integer" | "text" | "boolean" | "bytea"
            ) {
                return Err(invalid(
                    "PostgreSQL schema contains an unsupported column type",
                ));
            }
            let table: String = row.get(0);
            let column: String = row.get(1);
            validate_identifier(&table)?;
            validate_identifier(&column)?;
            tables
                .entry(table.clone())
                .or_insert_with(|| PgTable {
                    name: table,
                    columns: Vec::new(),
                    constraints: Vec::new(),
                })
                .columns
                .push(PgColumn {
                    name: column,
                    data_type,
                    nullable: row.get::<_, String>(3) == "YES",
                });
        }
        let constraints = self.client.query(
            "SELECT t.relname, c.conname, pg_get_constraintdef(c.oid) FROM pg_constraint c JOIN pg_class t ON t.oid=c.conrelid JOIN pg_namespace n ON n.oid=t.relnamespace WHERE n.nspname=$1 AND c.contype IN ('p','u','f') ORDER BY t.relname,c.conname",
            &[&schema],
        ).map_err(backend)?;
        let mut invariants = Vec::new();
        for row in constraints {
            let table: String = row.get(0);
            let name: String = row.get(1);
            let definition: String = row.get(2);
            validate_identifier(&name)?;
            tables
                .get_mut(&table)
                .ok_or_else(|| invalid("PostgreSQL constraint references an unknown table"))?
                .constraints
                .push(PgConstraint {
                    name: name.clone(),
                    definition: definition.clone(),
                });
            invariants.push(format!("{table}:{name}:{definition}"));
        }
        let definition = serde_json::to_string(&PgSchema {
            tables: tables.into_values().collect(),
        })?;
        Ok(StateSchema {
            backend: BackendKind::PostgreSql,
            resource: schema.into(),
            definition,
            invariants,
        })
    }

    fn capture(&mut self, schema: &str) -> Result<Vec<StateItem>, Error> {
        let tables = self.client.query("SELECT table_name FROM information_schema.tables WHERE table_schema=$1 AND table_type='BASE TABLE' ORDER BY table_name", &[&schema]).map_err(backend)?;
        let mut items = Vec::new();
        for table in tables {
            let table: String = table.get(0);
            validate_identifier(&table)?;
            let order: Option<String> = self.client.query_one("SELECT string_agg(quote_ident(a.attname), ',' ORDER BY array_position(i.indkey::smallint[],a.attnum)) FROM pg_index i JOIN pg_class t ON t.oid=i.indrelid JOIN pg_namespace n ON n.oid=t.relnamespace JOIN pg_attribute a ON a.attrelid=t.oid AND a.attnum=ANY(i.indkey) WHERE n.nspname=$1 AND t.relname=$2 AND i.indisprimary", &[&schema,&table]).map_err(backend)?.get(0);
            let order = order.ok_or_else(|| {
                invalid("PostgreSQL tables require a primary key for canonical snapshot order")
            })?;
            let query = format!(
                "COPY (SELECT * FROM {}.{} ORDER BY {}) TO STDOUT WITH (FORMAT binary)",
                quote(schema),
                quote(&table),
                order
            );
            let mut reader = self.client.copy_out(&query).map_err(backend)?;
            let mut payload = Vec::new();
            reader.read_to_end(&mut payload).map_err(Error::Io)?;
            items.push(StateItem {
                key: table,
                fields: BTreeMap::new(),
                payload,
            });
            enforce_bounds(&items)?;
        }
        Ok(items)
    }
}

impl TransactionalStateAdapter for PostgresAdapter {
    fn discover(&mut self) -> Result<StateSchema, Error> {
        let source = self.source_schema.clone();
        self.snapshot_schema(&source)
    }

    fn snapshot(&mut self) -> Result<StateSnapshot, Error> {
        let source = self.source_schema.clone();
        let schema_before = self.snapshot_schema(&source)?;
        let mut transaction = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .map_err(backend)?;
        let tables = transaction.query("SELECT table_name FROM information_schema.tables WHERE table_schema=$1 AND table_type='BASE TABLE' ORDER BY table_name", &[&source]).map_err(backend)?;
        let mut items = Vec::new();
        for row in tables {
            let table: String = row.get(0);
            validate_identifier(&table)?;
            let order: Option<String> = transaction.query_one("SELECT string_agg(quote_ident(a.attname), ',' ORDER BY array_position(i.indkey::smallint[],a.attnum)) FROM pg_index i JOIN pg_class t ON t.oid=i.indrelid JOIN pg_namespace n ON n.oid=t.relnamespace JOIN pg_attribute a ON a.attrelid=t.oid AND a.attnum=ANY(i.indkey) WHERE n.nspname=$1 AND t.relname=$2 AND i.indisprimary", &[&source,&table]).map_err(backend)?.get(0);
            let order = order.ok_or_else(|| {
                invalid("PostgreSQL tables require a primary key for canonical snapshot order")
            })?;
            let query = format!(
                "COPY (SELECT * FROM {}.{} ORDER BY {}) TO STDOUT WITH (FORMAT binary)",
                quote(&source),
                quote(&table),
                order
            );
            let mut reader = transaction.copy_out(&query).map_err(backend)?;
            let mut payload = Vec::new();
            reader.read_to_end(&mut payload).map_err(Error::Io)?;
            items.push(StateItem {
                key: table,
                fields: BTreeMap::new(),
                payload,
            });
            enforce_bounds(&items)?;
        }
        transaction.commit().map_err(backend)?;
        let schema = self.snapshot_schema(&source)?;
        if schema != schema_before {
            return Err(invalid(
                "PostgreSQL schema mutated during snapshot; retry after quiescing migrations",
            ));
        }
        let mut snapshot = StateSnapshot {
            version: "state-snapshot-v1".into(),
            schema,
            consistency: "PostgreSQL REPEATABLE READ read-only transaction".into(),
            items,
            schema_sha256: String::new(),
            data_sha256: String::new(),
        };
        seal_snapshot(&mut snapshot)?;
        Ok(snapshot)
    }

    fn plan(&mut self, snapshot: &StateSnapshot, target: &str) -> Result<MigrationPlan, Error> {
        validate_snapshot(snapshot)?;
        validate_identifier(target)?;
        if snapshot.schema.backend != BackendKind::PostgreSql {
            return Err(invalid("snapshot backend does not match PostgreSQL"));
        }
        Ok(MigrationPlan {
            version: "state-migration-v1".into(),
            backend: BackendKind::PostgreSql,
            source_schema_sha256: snapshot.schema_sha256.clone(),
            target_resource: target.into(),
            operations: vec![
                "clone source schema DDL into staging schema".into(),
                "COPY binary rows into staging tables".into(),
                "validate constraints and compare canonical snapshot".into(),
                "atomically swap target and staging schemas".into(),
            ],
            rollback_strategy: "transactional schema rename retains the prior target".into(),
        })
    }

    fn restore(
        &mut self,
        snapshot: &StateSnapshot,
        plan: &MigrationPlan,
    ) -> Result<RestoreReceipt, Error> {
        validate_plan(snapshot, plan)?;
        validate_identifier(&plan.target_resource)?;
        let target = &plan.target_resource;
        let stage = format!("{target}_anasemble_stage");
        let rollback = format!("{target}_anasemble_rollback");
        for name in [&stage, &rollback] {
            validate_identifier(name)?;
        }
        let mut tx = self.client.transaction().map_err(backend)?;
        let exists: bool = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name=$1)",
                &[&stage],
            )
            .map_err(backend)?
            .get(0);
        if exists {
            return Err(invalid(
                "stale PostgreSQL staging schema requires operator action",
            ));
        }
        tx.batch_execute(&format!("CREATE SCHEMA {};", quote(&stage)))
            .map_err(backend)?;
        let schema: PgSchema = serde_json::from_str(&snapshot.schema.definition)?;
        if schema.tables.len() != snapshot.items.len() {
            return Err(invalid("PostgreSQL schema and data table sets diverge"));
        }
        for table in &schema.tables {
            validate_identifier(&table.name)?;
            let mut columns = Vec::new();
            for column in &table.columns {
                validate_identifier(&column.name)?;
                if !matches!(
                    column.data_type.as_str(),
                    "bigint" | "integer" | "text" | "boolean" | "bytea"
                ) {
                    return Err(invalid("PostgreSQL snapshot contains an unsupported type"));
                }
                columns.push(format!(
                    "{} {}{}",
                    quote(&column.name),
                    column.data_type,
                    if column.nullable { "" } else { " NOT NULL" }
                ));
            }
            tx.batch_execute(&format!(
                "CREATE TABLE {}.{} ({});",
                quote(&stage),
                quote(&table.name),
                columns.join(",")
            ))
            .map_err(backend)?;
        }
        for item in &snapshot.items {
            validate_identifier(&item.key)?;
            let sql = format!(
                "COPY {}.{} FROM STDIN WITH (FORMAT binary)",
                quote(&stage),
                quote(&item.key)
            );
            let mut writer = tx.copy_in(&sql).map_err(backend)?;
            writer.write_all(&item.payload).map_err(Error::Io)?;
            writer.finish().map_err(backend)?;
        }
        for table in &schema.tables {
            for constraint in &table.constraints {
                let definition = constraint.definition.replace(
                    &format!("{}.", snapshot.schema.resource),
                    &format!("{stage}."),
                );
                tx.batch_execute(&format!(
                    "ALTER TABLE {}.{} ADD CONSTRAINT {} {};",
                    quote(&stage),
                    quote(&table.name),
                    quote(&constraint.name),
                    definition
                ))
                .map_err(backend)?;
            }
        }
        let target_exists: bool = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name=$1)",
                &[target],
            )
            .map_err(backend)?
            .get(0);
        let rollback_exists: bool = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name=$1)",
                &[&rollback],
            )
            .map_err(backend)?
            .get(0);
        if rollback_exists {
            return Err(invalid(
                "stale PostgreSQL rollback schema requires operator action",
            ));
        }
        if target_exists {
            tx.batch_execute(&format!(
                "ALTER SCHEMA {} RENAME TO {};",
                quote(target),
                quote(&rollback)
            ))
            .map_err(backend)?;
        }
        tx.batch_execute(&format!(
            "ALTER SCHEMA {} RENAME TO {};",
            quote(&stage),
            quote(target)
        ))
        .map_err(backend)?;
        tx.commit().map_err(backend)?;
        let restore_receipt = receipt(
            snapshot,
            plan,
            if target_exists {
                rollback
            } else {
                String::new()
            },
        )?;
        if let Err(error) = self.verify(snapshot, target) {
            if !restore_receipt.rollback_token.is_empty() {
                self.rollback(&restore_receipt)?;
            }
            return Err(error);
        }
        Ok(restore_receipt)
    }

    fn verify(&mut self, snapshot: &StateSnapshot, target: &str) -> Result<(), Error> {
        let items = self.capture(target)?;
        if data_digest(&items)? != snapshot.data_sha256 {
            return Err(invalid("PostgreSQL restore verification failed"));
        }
        Ok(())
    }

    fn rollback(&mut self, receipt: &RestoreReceipt) -> Result<(), Error> {
        if receipt.backend != BackendKind::PostgreSql || receipt.rollback_token.is_empty() {
            return Err(invalid("PostgreSQL rollback is unavailable"));
        }
        validate_identifier(&receipt.target_resource)?;
        validate_identifier(&receipt.rollback_token)?;
        let failed = format!("{}_anasemble_failed", receipt.target_resource);
        validate_identifier(&failed)?;
        self.client.batch_execute(&format!("BEGIN; DROP SCHEMA IF EXISTS {} CASCADE; ALTER SCHEMA {} RENAME TO {}; ALTER SCHEMA {} RENAME TO {}; COMMIT;",quote(&failed),quote(&receipt.target_resource),quote(&failed),quote(&receipt.rollback_token),quote(&receipt.target_resource))).map_err(backend)
    }
}

pub struct S3Adapter {
    store: object_store::aws::AmazonS3,
    runtime: tokio::runtime::Runtime,
    prefix: String,
}

impl S3Adapter {
    pub fn connect(
        endpoint: &str,
        region: &str,
        bucket: &str,
        access_key: &str,
        secret_key: &str,
        prefix: &str,
    ) -> Result<Self, Error> {
        validate_prefix(prefix)?;
        let loopback_http =
            endpoint.starts_with("http://127.0.0.1:") || endpoint.starts_with("http://localhost:");
        if endpoint.starts_with("http://") && !loopback_http {
            return Err(invalid("remote S3-compatible endpoints require HTTPS"));
        }
        let store = AmazonS3Builder::new()
            .with_endpoint(endpoint)
            .with_region(region)
            .with_bucket_name(bucket)
            .with_access_key_id(access_key)
            .with_secret_access_key(secret_key)
            .with_virtual_hosted_style_request(false)
            .with_allow_http(loopback_http)
            .build()
            .map_err(backend)?;
        let runtime = tokio::runtime::Runtime::new().map_err(Error::Io)?;
        Ok(Self {
            store,
            runtime,
            prefix: prefix.into(),
        })
    }
    pub fn put_object(&self, key: &str, payload: &[u8]) -> Result<(), Error> {
        let path = object_path(key)?;
        self.runtime
            .block_on(self.store.put(&path, PutPayload::from(payload.to_vec())))
            .map_err(backend)?;
        Ok(())
    }
    pub fn get_object(&self, key: &str) -> Result<Vec<u8>, Error> {
        let path = object_path(key)?;
        Ok(self
            .runtime
            .block_on(async { self.store.get(&path).await?.bytes().await })
            .map_err(backend)?
            .to_vec())
    }
    fn metadata(&self, prefix: &str) -> Result<Vec<object_store::ObjectMeta>, Error> {
        let path = ObjectPath::parse(prefix).map_err(backend)?;
        self.runtime
            .block_on(self.store.list(Some(&path)).try_collect())
            .map_err(backend)
    }
    fn capture(&self, prefix: &str) -> Result<Vec<StateItem>, Error> {
        let mut items = Vec::new();
        for object in self.metadata(prefix)? {
            let key = object.location.to_string();
            let payload = self.get_object(&key)?;
            let mut fields = BTreeMap::new();
            fields.insert("etag".into(), object.e_tag.unwrap_or_default());
            items.push(StateItem {
                key,
                fields,
                payload,
            });
            enforce_bounds(&items)?;
        }
        items.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(items)
    }
    fn replace_prefix(&self, prefix: &str, items: &[StateItem]) -> Result<(), Error> {
        self.delete_prefix(prefix)?;
        for item in items {
            let suffix = item
                .key
                .strip_prefix(&self.prefix)
                .ok_or_else(|| invalid("S3 snapshot key escaped its declared prefix"))?;
            let key = format!("{prefix}{suffix}");
            self.put_object(&key, &item.payload)?;
        }
        Ok(())
    }
    fn restore_exact(&self, prefix: &str, items: &[StateItem]) -> Result<(), Error> {
        self.delete_prefix(prefix)?;
        for item in items {
            if !item.key.starts_with(prefix) {
                return Err(invalid("S3 rollback key escaped its prefix"));
            }
            self.put_object(&item.key, &item.payload)?;
        }
        Ok(())
    }
    fn delete_prefix(&self, prefix: &str) -> Result<(), Error> {
        for object in self.metadata(prefix)? {
            self.runtime
                .block_on(self.store.delete(&object.location))
                .map_err(backend)?;
        }
        Ok(())
    }
}

impl TransactionalStateAdapter for S3Adapter {
    fn discover(&mut self) -> Result<StateSchema, Error> {
        Ok(StateSchema {
            backend: BackendKind::S3Compatible,
            resource: self.prefix.clone(),
            definition: "ordered object keys, bytes, and ETags".into(),
            invariants: vec![
                "two listings around reads must match".into(),
                "keys remain within the declared prefix".into(),
            ],
        })
    }
    fn snapshot(&mut self) -> Result<StateSnapshot, Error> {
        let first = self.capture(&self.prefix)?;
        let second = self.capture(&self.prefix)?;
        if first != second {
            return Err(invalid(
                "S3 prefix mutated during snapshot; retry after quiescing writers",
            ));
        }
        let schema = self.discover()?;
        let mut snapshot = StateSnapshot {
            version: "state-snapshot-v1".into(),
            schema,
            consistency: "two-pass stable S3 listing and byte read; concurrent mutation is refused"
                .into(),
            items: first,
            schema_sha256: String::new(),
            data_sha256: String::new(),
        };
        seal_snapshot(&mut snapshot)?;
        Ok(snapshot)
    }
    fn plan(&mut self, snapshot: &StateSnapshot, target: &str) -> Result<MigrationPlan, Error> {
        validate_snapshot(snapshot)?;
        validate_prefix(target)?;
        Ok(MigrationPlan {
            version: "state-migration-v1".into(),
            backend: BackendKind::S3Compatible,
            source_schema_sha256: snapshot.schema_sha256.clone(),
            target_resource: target.into(),
            operations: vec![
                "copy prior target to rollback prefix".into(),
                "replace target objects".into(),
                "verify bytes and key set".into(),
            ],
            rollback_strategy: "S3-native object copies retained under a unique rollback prefix"
                .into(),
        })
    }
    fn restore(
        &mut self,
        snapshot: &StateSnapshot,
        plan: &MigrationPlan,
    ) -> Result<RestoreReceipt, Error> {
        validate_plan(snapshot, plan)?;
        let rollback = format!(
            "anasemble-rollback/{}/",
            bytes_digest(plan.target_resource.as_bytes())
        );
        if !self.capture(&rollback)?.is_empty() {
            return Err(invalid("stale S3 rollback prefix requires operator action"));
        }
        let previous = self.capture(&plan.target_resource)?;
        for item in &previous {
            let suffix = item
                .key
                .strip_prefix(&plan.target_resource)
                .ok_or_else(|| invalid("invalid S3 target key"))?;
            self.put_object(&format!("{rollback}{suffix}"), &item.payload)?;
        }
        if let Err(error) = self
            .replace_prefix(&plan.target_resource, &snapshot.items)
            .and_then(|()| self.verify(snapshot, &plan.target_resource))
        {
            self.restore_exact(&plan.target_resource, &previous)?;
            return Err(error);
        }
        receipt(snapshot, plan, rollback)
    }
    fn verify(&mut self, snapshot: &StateSnapshot, target: &str) -> Result<(), Error> {
        let mut items = self.capture(target)?;
        for item in &mut items {
            let suffix = item
                .key
                .strip_prefix(target)
                .ok_or_else(|| invalid("S3 verification key escaped prefix"))?;
            item.key = format!("{}{}", snapshot.schema.resource, suffix);
        }
        if data_digest(&items)? != snapshot.data_sha256 {
            return Err(invalid("S3 restore verification failed"));
        }
        Ok(())
    }
    fn rollback(&mut self, receipt: &RestoreReceipt) -> Result<(), Error> {
        if receipt.backend != BackendKind::S3Compatible || receipt.rollback_token.is_empty() {
            return Err(invalid("S3 rollback is unavailable"));
        }
        let old = self.capture(&receipt.rollback_token)?;
        let mut source_items = old;
        for item in &mut source_items {
            let suffix = item
                .key
                .strip_prefix(&receipt.rollback_token)
                .ok_or_else(|| invalid("invalid S3 rollback key"))?;
            item.key = format!("{}{}", self.prefix, suffix);
        }
        self.replace_prefix(&receipt.target_resource, &source_items)?;
        self.replace_prefix(&receipt.rollback_token, &[])
    }
}

#[derive(Deserialize, Serialize)]
struct RedisEntry {
    id: String,
    fields: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Deserialize, Serialize)]
struct RedisGroup {
    name: String,
    last_delivered_id: String,
}

pub struct RedisStreamAdapter {
    connection: Connection,
    stream: String,
}
impl RedisStreamAdapter {
    pub fn connect(url: &str, stream: &str) -> Result<Self, Error> {
        validate_redis_key(stream)?;
        let client = redis::Client::open(url).map_err(backend)?;
        Ok(Self {
            connection: client.get_connection().map_err(backend)?,
            stream: stream.into(),
        })
    }
    fn capture(&mut self, key: &str) -> Result<Vec<StateItem>, Error> {
        let raw: redis::Value = redis::cmd("XRANGE")
            .arg(key)
            .arg("-")
            .arg("+")
            .arg("COUNT")
            .arg(MAX_STATE_ITEMS + 1)
            .query(&mut self.connection)
            .map_err(backend)?;
        let redis::Value::Array(entries) = raw else {
            return Err(invalid("Redis XRANGE returned an unexpected response"));
        };
        if entries.len() > MAX_STATE_ITEMS {
            return Err(invalid("Redis stream exceeds item bound"));
        }
        let mut items = Vec::with_capacity(entries.len());
        for entry in entries {
            let redis::Value::Array(mut parts) = entry else {
                return Err(invalid("Redis stream entry is malformed"));
            };
            if parts.len() != 2 {
                return Err(invalid("Redis stream entry has invalid arity"));
            }
            let fields_value = parts.pop().expect("length checked");
            let id_value = parts.pop().expect("length checked");
            let id = value_bytes(id_value).and_then(|v| {
                String::from_utf8(v).map_err(|_| invalid("Redis stream ID is not UTF-8"))
            })?;
            let redis::Value::Array(field_values) = fields_value else {
                return Err(invalid("Redis stream fields are malformed"));
            };
            if field_values.len() % 2 != 0 {
                return Err(invalid("Redis stream fields have invalid arity"));
            }
            let mut fields = Vec::new();
            let mut iter = field_values.into_iter();
            while let Some(name) = iter.next() {
                fields.push((
                    value_bytes(name)?,
                    value_bytes(iter.next().expect("even arity"))?,
                ));
            }
            let payload = serde_json::to_vec(&RedisEntry {
                id: id.clone(),
                fields,
            })?;
            items.push(StateItem {
                key: id,
                fields: BTreeMap::new(),
                payload,
            });
            enforce_bounds(&items)?;
        }
        let groups: redis::streams::StreamInfoGroupsReply =
            self.connection.xinfo_groups(key).map_err(backend)?;
        for group in groups.groups {
            if group.pending != 0 {
                return Err(invalid(
                    "Redis stream has pending consumer entries; snapshot refused",
                ));
            }
            let payload = serde_json::to_vec(&RedisGroup {
                name: group.name.clone(),
                last_delivered_id: group.last_delivered_id,
            })?;
            items.push(StateItem {
                key: format!("@group/{}", group.name),
                fields: BTreeMap::new(),
                payload,
            });
            enforce_bounds(&items)?;
        }
        items.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(items)
    }
}
impl TransactionalStateAdapter for RedisStreamAdapter {
    fn discover(&mut self) -> Result<StateSchema, Error> {
        Ok(StateSchema {
            backend: BackendKind::RedisStream,
            resource: self.stream.clone(),
            definition: "Redis Stream ordered entry IDs and field/value pairs".into(),
            invariants: vec![
                "no pending consumer entries at snapshot".into(),
                "stream length and last-generated ID remain stable".into(),
            ],
        })
    }
    fn snapshot(&mut self) -> Result<StateSnapshot, Error> {
        let first = self.capture(&self.stream.clone())?;
        let second = self.capture(&self.stream.clone())?;
        if first != second {
            return Err(invalid(
                "Redis stream mutated during snapshot; retry after quiescing producers",
            ));
        }
        let schema = self.discover()?;
        let mut s = StateSnapshot {
            version: "state-snapshot-v1".into(),
            schema,
            consistency: "two-pass stable Redis Stream read with empty pending-entry lists".into(),
            items: first,
            schema_sha256: String::new(),
            data_sha256: String::new(),
        };
        seal_snapshot(&mut s)?;
        Ok(s)
    }
    fn plan(&mut self, s: &StateSnapshot, target: &str) -> Result<MigrationPlan, Error> {
        validate_snapshot(s)?;
        validate_redis_key(target)?;
        Ok(MigrationPlan {
            version: "state-migration-v1".into(),
            backend: BackendKind::RedisStream,
            source_schema_sha256: s.schema_sha256.clone(),
            target_resource: target.into(),
            operations: vec![
                "materialize stream in staging key".into(),
                "verify ordered entries".into(),
                "atomically rename target to rollback and staging to target".into(),
            ],
            rollback_strategy: "Redis atomic RENAME preserves the previous stream key".into(),
        })
    }
    fn restore(&mut self, s: &StateSnapshot, p: &MigrationPlan) -> Result<RestoreReceipt, Error> {
        validate_plan(s, p)?;
        let stage = format!("{}:anasemble:stage", p.target_resource);
        let rollback = format!("{}:anasemble:rollback", p.target_resource);
        validate_redis_key(&stage)?;
        validate_redis_key(&rollback)?;
        let stale: i64 = self.connection.exists(&stage).map_err(backend)?;
        let stale_rollback: i64 = self.connection.exists(&rollback).map_err(backend)?;
        if stale != 0 || stale_rollback != 0 {
            return Err(invalid(
                "stale Redis staging or rollback key requires operator action",
            ));
        }
        for item in &s.items {
            if item.key.starts_with("@group/") {
                continue;
            }
            let entry: RedisEntry = serde_json::from_slice(&item.payload)?;
            let mut command = redis::cmd("XADD");
            command.arg(&stage).arg(&entry.id);
            for (name, value) in entry.fields {
                command.arg(name).arg(value);
            }
            command
                .query::<String>(&mut self.connection)
                .map_err(backend)?;
        }
        for item in &s.items {
            if !item.key.starts_with("@group/") {
                continue;
            }
            let group: RedisGroup = serde_json::from_slice(&item.payload)?;
            redis::cmd("XGROUP")
                .arg("CREATE")
                .arg(&stage)
                .arg(group.name)
                .arg(group.last_delivered_id)
                .query::<()>(&mut self.connection)
                .map_err(backend)?;
        }
        let target_exists: i64 = self
            .connection
            .exists(&p.target_resource)
            .map_err(backend)?;
        let script = redis::Script::new(
            "if redis.call('EXISTS',KEYS[2])==1 then return redis.error_reply('rollback exists') end; if redis.call('EXISTS',KEYS[1])==1 then redis.call('RENAME',KEYS[1],KEYS[2]) end; redis.call('RENAME',KEYS[3],KEYS[1]); return 1",
        );
        script
            .key(&p.target_resource)
            .key(&rollback)
            .key(&stage)
            .invoke::<i64>(&mut self.connection)
            .map_err(backend)?;
        let restore_receipt = receipt(
            s,
            p,
            if target_exists != 0 {
                rollback
            } else {
                String::new()
            },
        )?;
        if let Err(error) = self.verify(s, &p.target_resource) {
            if !restore_receipt.rollback_token.is_empty() {
                self.rollback(&restore_receipt)?;
            }
            return Err(error);
        }
        Ok(restore_receipt)
    }
    fn verify(&mut self, s: &StateSnapshot, target: &str) -> Result<(), Error> {
        if data_digest(&self.capture(target)?)? != s.data_sha256 {
            return Err(invalid("Redis stream restore verification failed"));
        }
        Ok(())
    }
    fn rollback(&mut self, r: &RestoreReceipt) -> Result<(), Error> {
        if r.backend != BackendKind::RedisStream || r.rollback_token.is_empty() {
            return Err(invalid("Redis rollback is unavailable"));
        }
        redis::cmd("RENAME")
            .arg(&r.rollback_token)
            .arg(&r.target_resource)
            .query::<()>(&mut self.connection)
            .map_err(backend)
    }
}

fn seal_snapshot(snapshot: &mut StateSnapshot) -> Result<(), Error> {
    snapshot.schema_sha256 = digest(&snapshot.schema)?;
    snapshot.data_sha256 = data_digest(&snapshot.items)?;
    validate_snapshot(snapshot)
}
fn data_digest(items: &[StateItem]) -> Result<String, Error> {
    enforce_bounds(items)?;
    digest(&items.to_vec())
}
fn validate_snapshot(s: &StateSnapshot) -> Result<(), Error> {
    if s.version != "state-snapshot-v1"
        || s.schema_sha256 != digest(&s.schema)?
        || s.data_sha256 != data_digest(&s.items)?
    {
        return Err(invalid("state snapshot integrity is invalid"));
    }
    Ok(())
}
fn validate_plan(s: &StateSnapshot, p: &MigrationPlan) -> Result<(), Error> {
    validate_snapshot(s)?;
    if p.version != "state-migration-v1"
        || p.backend != s.schema.backend
        || p.source_schema_sha256 != s.schema_sha256
        || p.operations.is_empty()
        || p.operations.len() > 16
        || p.rollback_strategy.is_empty()
    {
        return Err(invalid("state migration plan does not match its snapshot"));
    }
    Ok(())
}
fn receipt(
    s: &StateSnapshot,
    p: &MigrationPlan,
    rollback_token: String,
) -> Result<RestoreReceipt, Error> {
    Ok(RestoreReceipt {
        backend: p.backend.clone(),
        target_resource: p.target_resource.clone(),
        snapshot_sha256: digest(s)?,
        migration_sha256: digest(p)?,
        verified_data_sha256: s.data_sha256.clone(),
        rollback_token,
    })
}
fn enforce_bounds(items: &[StateItem]) -> Result<(), Error> {
    if items.len() > MAX_STATE_ITEMS
        || items
            .iter()
            .try_fold(0usize, |n, i| {
                n.checked_add(
                    i.key.len()
                        + i.payload.len()
                        + i.fields
                            .iter()
                            .map(|(k, v)| k.len() + v.len())
                            .sum::<usize>(),
                )
            })
            .is_none_or(|n| n > MAX_STATE_BYTES)
    {
        return Err(invalid("state snapshot exceeds item or byte bounds"));
    }
    Ok(())
}
fn validate_identifier(v: &str) -> Result<(), Error> {
    if v.is_empty()
        || v.len() > 63
        || !v
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_lowercase() || (!i.eq(&0) && b.is_ascii_digit()))
    {
        return Err(invalid(
            "database identifiers must be lowercase ASCII identifiers",
        ));
    }
    Ok(())
}
fn quote(v: &str) -> String {
    format!("\"{v}\"")
}
fn validate_prefix(v: &str) -> Result<(), Error> {
    if v.is_empty()
        || v.len() > 256
        || !v.ends_with('/')
        || v.starts_with('/')
        || v.contains("..")
        || v.chars().any(char::is_control)
    {
        return Err(invalid("S3 prefix is invalid"));
    }
    Ok(())
}
fn validate_redis_key(v: &str) -> Result<(), Error> {
    if v.is_empty() || v.len() > 256 || v.chars().any(char::is_control) {
        return Err(invalid("Redis key is invalid"));
    }
    Ok(())
}
fn require_digest(v: &str) -> Result<(), Error> {
    if v.len() != 64 || hex::decode(v).is_err() {
        return Err(invalid("expected a SHA-256 digest"));
    }
    Ok(())
}
fn invalid(message: &str) -> Error {
    Error::InvalidEvidence(message.into())
}
fn backend(error: impl std::fmt::Display) -> Error {
    Error::InvalidEvidence(format!("state backend error: {error}"))
}
fn value_bytes(value: redis::Value) -> Result<Vec<u8>, Error> {
    match value {
        redis::Value::BulkString(bytes) => Ok(bytes),
        redis::Value::SimpleString(text) => Ok(text.into_bytes()),
        _ => Err(invalid("Redis stream value is not binary data")),
    }
}
fn object_path(value: &str) -> Result<ObjectPath, Error> {
    ObjectPath::parse(value).map_err(backend)
}
