//! Per-test database provisioning across SQLite / Postgres / MySQL.
//!
//! Integration tests are black-box (HTTP / gRPC against a real server that owns
//! its own connection pool), so transaction-rollback isolation is impossible —
//! test and code-under-test never share a connection. Each test therefore gets
//! its own *physical* database.
//!
//! The backend is chosen once per `cargo test` invocation via `TEST_DATABASE`
//! (`sqlite` — the default — `postgres`, or `mysql`). The admin / maintenance
//! connection used to create and drop databases comes from `TEST_DATABASE_URL`
//! (sensible localhost defaults matching `docker-compose.test.yml` otherwise).
//!
//! - **SQLite** stays `sqlite::memory:` — no Docker, identical to the original
//!   behaviour, so a bare `cargo test` keeps working with zero setup.
//! - **Postgres / MySQL** get a fresh `cms_test_<uuidv7>` database created here
//!   and dropped on teardown (best-effort). A startup sweep drops leftover
//!   `cms_test_*` databases from a previous aborted run (only ones older than
//!   [`SWEEP_MIN_AGE_MS`], so concurrent test processes never sweep each
//!   other), keeping local re-runs self-healing.

use sqlx::Connection;
use tokio::sync::OnceCell;

/// Prefix for every per-test database. Greppable and used by the leak sweep.
const DB_PREFIX: &str = "cms_test_";

/// Backend the integration suite runs against for this process.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Backend {
    Sqlite,
    Postgres,
    MySql,
}

impl Backend {
    fn from_env() -> Self {
        match std::env::var("TEST_DATABASE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "" | "sqlite" => Backend::Sqlite,
            "postgres" | "postgresql" | "pg" => Backend::Postgres,
            "mysql" | "mariadb" => Backend::MySql,
            other => panic!("unknown TEST_DATABASE={other:?} (expected sqlite | postgres | mysql)"),
        }
    }

    /// Admin / maintenance URL used to create & drop per-test databases.
    fn admin_url(self) -> String {
        if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
            return url;
        }
        match self {
            Backend::Sqlite => String::new(),
            Backend::Postgres => "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            Backend::MySql => "mysql://root:root@localhost:3306/mysql".to_string(),
        }
    }
}

/// Teardown handle returned alongside a provisioned database URL. Dropping it
/// best-effort drops the per-test database (no-op for SQLite `:memory:`).
pub struct TestDbHandle {
    drop_info: Option<DropInfo>,
}

struct DropInfo {
    admin_url: String,
    db_name: String,
    backend: Backend,
}

impl TestDbHandle {
    fn noop() -> Self {
        TestDbHandle { drop_info: None }
    }
}

impl Drop for TestDbHandle {
    fn drop(&mut self) {
        let Some(info) = self.drop_info.take() else {
            return;
        };
        // `Drop` is sync and we are usually inside the test's tokio runtime,
        // where `block_on` would panic ("cannot start a runtime from within a
        // runtime"). Run the async DROP on a fresh thread with its own
        // current-thread runtime, and join so teardown completes before the
        // test returns.
        let _ = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build teardown runtime");
            rt.block_on(async move {
                if let Err(e) = drop_database(&info.admin_url, &info.db_name, info.backend).await {
                    eprintln!("warning: failed to drop test database {}: {e}", info.db_name);
                }
            });
        })
        .join();
    }
}

/// Provision an isolated database for one `TestServer`. Returns its connection
/// URL (hand straight to `init_db_with_config`) and a teardown handle the
/// `TestServer` must hold for the lifetime of the server.
pub async fn provision() -> (String, TestDbHandle) {
    let backend = Backend::from_env();
    if backend == Backend::Sqlite {
        return ("sqlite::memory:".to_string(), TestDbHandle::noop());
    }

    let admin_url = backend.admin_url();
    ensure_swept(&admin_url, backend).await;

    let db_name = format!("{DB_PREFIX}{}", uuid::Uuid::now_v7().simple());
    create_database(&admin_url, &db_name, backend)
        .await
        .unwrap_or_else(|e| panic!("failed to create test database {db_name}: {e}"));

    let url = per_test_url(&admin_url, &db_name);
    let handle = TestDbHandle {
        drop_info: Some(DropInfo {
            admin_url,
            db_name,
            backend,
        }),
    };
    (url, handle)
}

/// Build a per-test connection URL by swapping the database/path component of
/// the admin URL for `db_name`.
fn per_test_url(admin_url: &str, db_name: &str) -> String {
    let mut url = url::Url::parse(admin_url).expect("TEST_DATABASE_URL must be a valid URL");
    url.set_path(db_name);
    url.to_string()
}

async fn create_database(admin_url: &str, db_name: &str, backend: Backend) -> Result<(), sqlx::Error> {
    match backend {
        Backend::Postgres => {
            let mut conn = sqlx::postgres::PgConnection::connect(admin_url).await?;
            let sql = format!("CREATE DATABASE \"{db_name}\"");
            sqlx::query(sqlx::AssertSqlSafe(sql)).execute(&mut conn).await?;
            conn.close().await?;
        }
        Backend::MySql => {
            let mut conn = sqlx::mysql::MySqlConnection::connect(admin_url).await?;
            let sql = format!("CREATE DATABASE `{db_name}`");
            sqlx::query(sqlx::AssertSqlSafe(sql)).execute(&mut conn).await?;
            conn.close().await?;
        }
        Backend::Sqlite => {}
    }
    Ok(())
}

/// Drop a per-test database over a fresh, single connection. Used by both the
/// teardown (which runs on its own thread + runtime, so it must NOT touch the
/// main-runtime maintenance pool) and the startup sweep.
async fn drop_database(admin_url: &str, db_name: &str, backend: Backend) -> Result<(), sqlx::Error> {
    match backend {
        Backend::Postgres => {
            let mut conn = sqlx::postgres::PgConnection::connect(admin_url).await?;
            // FORCE (PG13+) evicts any straggler connections so the drop can't hang.
            let sql = format!("DROP DATABASE IF EXISTS \"{db_name}\" WITH (FORCE)");
            sqlx::query(sqlx::AssertSqlSafe(sql)).execute(&mut conn).await?;
            conn.close().await?;
        }
        Backend::MySql => {
            let mut conn = sqlx::mysql::MySqlConnection::connect(admin_url).await?;
            let sql = format!("DROP DATABASE IF EXISTS `{db_name}`");
            sqlx::query(sqlx::AssertSqlSafe(sql)).execute(&mut conn).await?;
            conn.close().await?;
        }
        Backend::Sqlite => {}
    }
    Ok(())
}

/// Run the leftover-database sweep exactly once per test process.
///
/// Under `cargo nextest` every test runs in its own process, so the sweep runs
/// many times concurrently — the age gate in `sweep_leftovers` is what keeps
/// those concurrent sweeps from dropping each other's live databases.
static SWEEP: OnceCell<()> = OnceCell::const_new();

/// Only databases older than this are swept. Live tests never get close (a
/// single test finishes in seconds); anything past it is a leak from an
/// aborted run.
const SWEEP_MIN_AGE_MS: u64 = 5 * 60 * 1000;

/// Age gate for the sweep: `true` when the database was created more than
/// [`SWEEP_MIN_AGE_MS`] ago. The name embeds a UUIDv7 (`cms_test_<uuid>`) whose
/// first 12 hex chars are the 48-bit unix-ms creation time. Names that don't
/// parse are legacy leftovers — treat as stale so they still get reclaimed.
fn is_stale(db_name: &str) -> bool {
    let Some(ts_hex) = db_name.strip_prefix(DB_PREFIX).and_then(|uuid| uuid.get(..12)) else {
        return true;
    };
    let Ok(created_ms) = u64::from_str_radix(ts_hex, 16) else {
        return true;
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_millis() as u64;
    now_ms.saturating_sub(created_ms) > SWEEP_MIN_AGE_MS
}

async fn ensure_swept(admin_url: &str, backend: Backend) {
    SWEEP
        .get_or_init(|| async {
            if let Err(e) = sweep_leftovers(admin_url, backend).await {
                eprintln!("warning: failed to sweep leftover test databases: {e}");
            }
        })
        .await;
}

/// Drop stale `cms_test_*` databases — reclaims leaks from a previous run that
/// aborted before teardown. CI containers are ephemeral, so this mainly keeps
/// local dev idempotent. Only databases older than [`SWEEP_MIN_AGE_MS`] are
/// dropped so concurrent test processes (nextest) can't sweep away databases
/// of tests that are still running.
async fn sweep_leftovers(admin_url: &str, backend: Backend) -> Result<(), sqlx::Error> {
    let names: Vec<String> = match backend {
        Backend::Postgres => {
            let mut conn = sqlx::postgres::PgConnection::connect(admin_url).await?;
            let names = sqlx::query_scalar::<_, String>("SELECT datname FROM pg_database WHERE datname LIKE $1")
                .bind(format!("{DB_PREFIX}%"))
                .fetch_all(&mut conn)
                .await?;
            conn.close().await?;
            names
        }
        Backend::MySql => {
            let mut conn = sqlx::mysql::MySqlConnection::connect(admin_url).await?;
            // `SHOW DATABASES LIKE ?` can't be a bound prepared statement; list
            // all and filter by prefix in Rust. MySQL returns the column as
            // binary, so decode as bytes then to UTF-8.
            let all = sqlx::query_scalar::<_, Vec<u8>>("SHOW DATABASES")
                .fetch_all(&mut conn)
                .await?;
            conn.close().await?;
            all.into_iter()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .filter(|n| n.starts_with(DB_PREFIX))
                .collect()
        }
        Backend::Sqlite => return Ok(()),
    };

    for name in names.into_iter().filter(|n| is_stale(n)) {
        drop_database(admin_url, &name, backend).await?;
    }
    Ok(())
}
