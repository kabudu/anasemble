mod common;

use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anasemble::canonical::bytes_digest;
use anasemble::protocol;
use anasemble::stateful::{
    PostgresAdapter, RedisStreamAdapter, S3Adapter, TransactionalStateAdapter, bind_activation_plan,
};
use postgres::{Client, NoTls};
use redis::Commands;
use tempfile::tempdir;

use common::{build_workspace, write_json};

struct Container {
    name: String,
}

impl Container {
    fn run(name: String, args: &[&str]) -> Self {
        let status = Command::new("docker")
            .args(["run", "--detach", "--name", &name])
            .args(args)
            .status()
            .expect("Docker must be available for authoritative P2 validation");
        assert!(status.success(), "failed to start {name}");
        Self { name }
    }

    fn port(&self, private: &str) -> u16 {
        for _ in 0..60 {
            let output = Command::new("docker")
                .args(["port", &self.name, private])
                .output()
                .unwrap();
            if output.status.success() {
                let text = String::from_utf8(output.stdout).unwrap();
                if let Some(port) = text.trim().rsplit(':').next().and_then(|p| p.parse().ok()) {
                    return port;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("container port was not published");
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        assert!(self.name.starts_with("anasemble-p2-"));
        let _ = Command::new("docker")
            .args(["rm", "--force", &self.name])
            .status();
    }
}

fn suffix() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

fn retry<T>(mut operation: impl FnMut() -> Option<T>) -> T {
    for _ in 0..120 {
        if let Some(value) = operation() {
            return value;
        }
        thread::sleep(Duration::from_millis(250));
    }
    panic!("backend did not become ready within 30 seconds");
}

#[test]
fn representative_http_recovery_binds_and_restores_all_p2_backends() {
    let suffix = suffix();
    let postgres_name = format!("anasemble-p2-postgres-{suffix}");
    let postgres_container = Container::run(
        postgres_name,
        &[
            "--env",
            "POSTGRES_PASSWORD=p2-password",
            "--publish",
            "127.0.0.1::5432",
            "postgres:18-alpine",
        ],
    );
    let postgres_port = postgres_container.port("5432/tcp");
    let postgres_url = format!(
        "host=127.0.0.1 port={postgres_port} user=postgres password=p2-password dbname=postgres"
    );
    let mut postgres = retry(|| Client::connect(&postgres_url, NoTls).ok());
    postgres
        .batch_execute(
            "CREATE SCHEMA source_state;
             CREATE TABLE source_state.accounts(id bigint PRIMARY KEY, name text NOT NULL UNIQUE);
             CREATE TABLE source_state.orders(id bigint PRIMARY KEY, account_id bigint NOT NULL REFERENCES source_state.accounts(id), note text NOT NULL);
             INSERT INTO source_state.accounts VALUES (1,'Ada'),(2,'Grace');
             INSERT INTO source_state.orders VALUES (10,1,'first'),(11,2,'second');
             CREATE SCHEMA recovered_state;
             CREATE TABLE recovered_state.accounts(id bigint PRIMARY KEY, name text NOT NULL);
             INSERT INTO recovered_state.accounts VALUES (99,'previous');",
        )
        .unwrap();
    drop(postgres);
    let mut postgres_adapter = PostgresAdapter::connect(&postgres_url, "source_state").unwrap();
    let postgres_snapshot = postgres_adapter.snapshot().unwrap();
    let mut source_destroyer = Client::connect(&postgres_url, NoTls).unwrap();
    source_destroyer
        .batch_execute("DROP SCHEMA source_state CASCADE")
        .unwrap();
    let postgres_plan = postgres_adapter
        .plan(&postgres_snapshot, "recovered_state")
        .unwrap();
    let postgres_receipt = postgres_adapter
        .restore(&postgres_snapshot, &postgres_plan)
        .unwrap();
    let mut postgres = Client::connect(&postgres_url, NoTls).unwrap();
    let recovered_count: i64 = postgres
        .query_one("SELECT count(*) FROM recovered_state.orders", &[])
        .unwrap()
        .get(0);
    assert_eq!(recovered_count, 2);
    assert!(
        postgres
            .execute("DELETE FROM recovered_state.accounts WHERE id=1", &[])
            .is_err()
    );
    postgres_adapter.rollback(&postgres_receipt).unwrap();
    let previous: String = postgres
        .query_one("SELECT name FROM recovered_state.accounts", &[])
        .unwrap()
        .get(0);
    assert_eq!(previous, "previous");

    let minio_name = format!("anasemble-p2-minio-{suffix}");
    let minio_container = Container::run(
        minio_name,
        &[
            "--env",
            "MINIO_ROOT_USER=p2-access",
            "--env",
            "MINIO_ROOT_PASSWORD=p2-secret-password",
            "--publish",
            "127.0.0.1::9000",
            "quay.io/minio/minio:latest",
            "server",
            "/data",
        ],
    );
    let minio_port = minio_container.port("9000/tcp");
    let endpoint = format!("http://127.0.0.1:{minio_port}");
    let bucket_name = format!("anasemble-p2-{suffix}");
    retry(|| {
        Command::new("docker")
            .args([
                "run",
                "--rm",
                "--network",
                &format!("container:{}", minio_container.name),
                "--env",
                "MC_HOST_local=http://p2-access:p2-secret-password@127.0.0.1:9000",
                "minio/mc:RELEASE.2025-08-13T08-35-41Z",
                "mb",
                &format!("local/{bucket_name}"),
            ])
            .status()
            .ok()
            .filter(|status| status.success())
    });
    let mut s3_adapter = S3Adapter::connect(
        &endpoint,
        "us-east-1",
        &bucket_name,
        "p2-access",
        "p2-secret-password",
        "source/",
    )
    .unwrap();
    s3_adapter
        .put_object("source/avatar.bin", b"avatar-v2")
        .unwrap();
    s3_adapter
        .put_object("source/report.txt", b"report-v2")
        .unwrap();
    s3_adapter
        .put_object("active/old.txt", b"previous-object")
        .unwrap();
    let s3_snapshot = s3_adapter.snapshot().unwrap();
    let s3_plan = s3_adapter.plan(&s3_snapshot, "active/").unwrap();
    let s3_receipt = s3_adapter.restore(&s3_snapshot, &s3_plan).unwrap();
    assert_eq!(
        s3_adapter.get_object("active/avatar.bin").unwrap(),
        b"avatar-v2"
    );
    s3_adapter.rollback(&s3_receipt).unwrap();
    assert_eq!(
        s3_adapter.get_object("active/old.txt").unwrap(),
        b"previous-object"
    );

    let redis_name = format!("anasemble-p2-redis-{suffix}");
    let redis_container = Container::run(
        redis_name,
        &[
            "--publish",
            "127.0.0.1::6379",
            "redis:8.8.0-alpine",
            "redis-server",
            "--appendonly",
            "yes",
        ],
    );
    let redis_port = redis_container.port("6379/tcp");
    let redis_url = format!("redis://127.0.0.1:{redis_port}/");
    let redis_client = retry(|| redis::Client::open(redis_url.clone()).ok());
    let mut redis = retry(|| redis_client.get_connection().ok());
    redis::cmd("XADD")
        .arg("source-events")
        .arg("1000-0")
        .arg("event")
        .arg("created")
        .query::<String>(&mut redis)
        .unwrap();
    redis::cmd("XADD")
        .arg("source-events")
        .arg("1001-0")
        .arg("event")
        .arg("paid")
        .query::<String>(&mut redis)
        .unwrap();
    redis::cmd("XGROUP")
        .arg("CREATE")
        .arg("source-events")
        .arg("workers")
        .arg("1000-0")
        .query::<()>(&mut redis)
        .unwrap();
    redis::cmd("XADD")
        .arg("active-events")
        .arg("1-0")
        .arg("event")
        .arg("previous")
        .query::<String>(&mut redis)
        .unwrap();
    drop(redis);
    let mut redis_adapter = RedisStreamAdapter::connect(&redis_url, "source-events").unwrap();
    let redis_snapshot = redis_adapter.snapshot().unwrap();
    let redis_plan = redis_adapter
        .plan(&redis_snapshot, "active-events")
        .unwrap();
    let redis_receipt = redis_adapter.restore(&redis_snapshot, &redis_plan).unwrap();
    let mut redis = redis_client.get_connection().unwrap();
    assert_eq!(redis.xlen::<_, i64>("active-events").unwrap(), 2);
    let groups: redis::streams::StreamInfoGroupsReply =
        redis.xinfo_groups("active-events").unwrap();
    assert_eq!(groups.groups[0].last_delivered_id, "1000-0");
    redis_adapter.rollback(&redis_receipt).unwrap();
    assert_eq!(redis.xlen::<_, i64>("active-events").unwrap(), 1);
    redis::cmd("XREADGROUP")
        .arg("GROUP")
        .arg("workers")
        .arg("worker-1")
        .arg("COUNT")
        .arg(1)
        .arg("STREAMS")
        .arg("source-events")
        .arg(">")
        .query::<redis::Value>(&mut redis)
        .unwrap();
    assert!(
        redis_adapter
            .snapshot()
            .unwrap_err()
            .to_string()
            .contains("pending consumer entries")
    );

    let directory = tempdir().unwrap();
    let workspace = build_workspace(directory.path(), true);
    assert!(!workspace.artifact.exists());
    assert_eq!(workspace.artifact_digest.len(), 64);
    let service = serde_json::json!({"version":"service-v1","component":"turnstile","interface_version":"1","http":{"endpoints":[{"method":"POST","path":"/transition","request_schema_sha256":"11".repeat(32),"response_schema_sha256":"22".repeat(32)}]},"effects":[{"kind":"state","target":"database","access":"read_write"},{"kind":"state","target":"objects","access":"read_write"},{"kind":"state","target":"events","access":"read_write"}],"state_dependencies":[{"name":"database","adapter":"postgres","consistency":"transactional","required":true},{"name":"objects","adapter":"object_store","consistency":"snapshot","required":true},{"name":"events","adapter":"queue","consistency":"snapshot","required":true}],"limits":{"request_bytes":4096,"response_bytes":4096,"wall_time_ms":1000,"concurrent_requests":8}});
    let service_digest = bytes_digest(&anasemble::canonical::encode(&service).unwrap());
    let registry_path = workspace.recovery.join("registry.json");
    let mut registry: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&registry_path).unwrap()).unwrap();
    registry["service_manifest"] = service;
    write_json(&registry_path, &registry);
    let recovery = protocol::run(&workspace.recovery);
    let activation = bind_activation_plan(
        &recovery,
        &service_digest,
        &[
            (&postgres_snapshot, &postgres_plan),
            (&s3_snapshot, &s3_plan),
            (&redis_snapshot, &redis_plan),
        ],
    )
    .unwrap();
    assert_eq!(activation.states.len(), 3);
    assert_eq!(activation.plan_sha256.len(), 64);
    assert!(
        bind_activation_plan(
            &recovery,
            &"00".repeat(32),
            &[(&postgres_snapshot, &postgres_plan)]
        )
        .is_err()
    );
}
