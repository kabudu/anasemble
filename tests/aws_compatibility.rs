use std::env;
use std::sync::Arc;
use std::time::Duration;

use anasemble::stateful::{
    PostgresAdapter, RedisStreamAdapter, S3Adapter, TransactionalStateAdapter,
};
use postgres::Client;
use redis::Commands;
use rustls::pki_types::{CertificateDer, pem::PemObject};
use rustls::{ClientConfig, RootCertStore};
use tokio_postgres_rustls::MakeRustlsConnect;

#[test]
#[ignore = "requires tagged ephemeral AWS fixtures"]
fn aws_remote_state_profiles_restore_and_rollback_over_tls() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let postgres_uri = required("ANASEMBLE_AWS_POSTGRES_URI");
    let postgres_ca = std::fs::read(required("ANASEMBLE_AWS_POSTGRES_CA")).unwrap();
    let redis_url = required("ANASEMBLE_AWS_REDIS_URL");
    let s3_endpoint = required("ANASEMBLE_AWS_S3_ENDPOINT");
    let s3_region = required("ANASEMBLE_AWS_S3_REGION");
    let s3_bucket = required("ANASEMBLE_AWS_S3_BUCKET");
    let s3_access_key = required("ANASEMBLE_AWS_S3_ACCESS_KEY");
    let s3_secret_key = required("ANASEMBLE_AWS_S3_SECRET_KEY");
    let s3_session_token = required("ANASEMBLE_AWS_S3_SESSION_TOKEN");
    let suffix = required("ANASEMBLE_AWS_RUN_SUFFIX");

    let source_schema = format!("source_{suffix}");
    let target_schema = format!("target_{suffix}");
    let mut postgres = postgres_client(&postgres_uri, &postgres_ca);
    postgres
        .batch_execute(&format!(
            "CREATE SCHEMA {source_schema}; CREATE TABLE {source_schema}.records (id bigint PRIMARY KEY, value text NOT NULL); INSERT INTO {source_schema}.records VALUES (1, 'survives'); CREATE SCHEMA {target_schema}; CREATE TABLE {target_schema}.records (id bigint PRIMARY KEY, value text NOT NULL); INSERT INTO {target_schema}.records VALUES (9, 'rollback');"
        ))
        .unwrap();
    drop(postgres);
    let mut postgres_adapter =
        PostgresAdapter::connect_tls(&postgres_uri, &source_schema, &postgres_ca).unwrap();
    let postgres_snapshot = postgres_adapter.snapshot().unwrap();
    let postgres_plan = postgres_adapter
        .plan(&postgres_snapshot, &target_schema)
        .unwrap();
    let postgres_receipt = postgres_adapter
        .restore(&postgres_snapshot, &postgres_plan)
        .unwrap();
    postgres_adapter
        .verify(&postgres_snapshot, &target_schema)
        .unwrap();
    postgres_adapter.rollback(&postgres_receipt).unwrap();
    let mut postgres = postgres_client(&postgres_uri, &postgres_ca);
    let value: String = postgres
        .query_one(&format!("SELECT value FROM {target_schema}.records"), &[])
        .unwrap()
        .get(0);
    assert_eq!(value, "rollback");
    postgres
        .batch_execute(&format!(
            "DROP SCHEMA {source_schema} CASCADE; DROP SCHEMA {target_schema} CASCADE; DROP SCHEMA IF EXISTS {target_schema}_anasemble_failed CASCADE;"
        ))
        .unwrap();

    let source_stream = format!("source-{suffix}");
    let target_stream = format!("target-{suffix}");
    let redis_client = redis::Client::open(redis_url.clone()).unwrap();
    let mut redis = redis_client
        .get_connection_with_timeout(Duration::from_secs(10))
        .unwrap();
    redis::cmd("XADD")
        .arg(&source_stream)
        .arg("1-0")
        .arg("value")
        .arg("survives")
        .query::<String>(&mut redis)
        .unwrap();
    redis::cmd("XADD")
        .arg(&target_stream)
        .arg("1-0")
        .arg("value")
        .arg("rollback")
        .query::<String>(&mut redis)
        .unwrap();
    drop(redis);
    let mut redis_adapter = RedisStreamAdapter::connect(&redis_url, &source_stream).unwrap();
    let redis_snapshot = redis_adapter.snapshot().unwrap();
    let redis_plan = redis_adapter.plan(&redis_snapshot, &target_stream).unwrap();
    let redis_receipt = redis_adapter.restore(&redis_snapshot, &redis_plan).unwrap();
    redis_adapter
        .verify(&redis_snapshot, &target_stream)
        .unwrap();
    redis_adapter.rollback(&redis_receipt).unwrap();
    let mut redis = redis_client.get_connection().unwrap();
    assert_eq!(redis.xlen::<_, i64>(&target_stream).unwrap(), 1);
    redis::cmd("DEL")
        .arg(&source_stream)
        .arg(&target_stream)
        .arg(format!("{target_stream}:anasemble:failed"))
        .query::<i64>(&mut redis)
        .unwrap();

    let source_prefix = format!("source-{suffix}/");
    let target_prefix = format!("target-{suffix}/");
    let mut s3 = S3Adapter::connect_with_token(
        &s3_endpoint,
        &s3_region,
        &s3_bucket,
        &s3_access_key,
        &s3_secret_key,
        Some(&s3_session_token),
        &source_prefix,
    )
    .unwrap();
    s3.put_object(&format!("{source_prefix}record.bin"), b"survives")
        .unwrap();
    s3.put_object(&format!("{target_prefix}record.bin"), b"rollback")
        .unwrap();
    let s3_snapshot = s3.snapshot().unwrap();
    let s3_plan = s3.plan(&s3_snapshot, &target_prefix).unwrap();
    let s3_receipt = s3.restore(&s3_snapshot, &s3_plan).unwrap();
    s3.verify(&s3_snapshot, &target_prefix).unwrap();
    s3.rollback(&s3_receipt).unwrap();
    assert_eq!(
        s3.get_object(&format!("{target_prefix}record.bin"))
            .unwrap(),
        b"rollback"
    );
}

fn postgres_client(uri: &str, ca_pem: &[u8]) -> Client {
    let mut roots = RootCertStore::empty();
    let certificates = CertificateDer::pem_slice_iter(ca_pem)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for certificate in certificates {
        roots.add(certificate).unwrap();
    }
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
    Client::connect(uri, MakeRustlsConnect::new(config)).unwrap()
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
}
