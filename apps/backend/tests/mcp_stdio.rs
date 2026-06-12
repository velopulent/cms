use std::process::Stdio;

use cms::config::Config;
use cms::database::{init_db_with_config, pool::DbPool};
use cms::models::access_token::AccessTokenPermission;
use cms::repository::Repository;
use cms::services::access_token::AccessTokenService;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

struct StdioClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl StdioClient {
    async fn start(database_url: &str, hmac_secret: &str, token: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_cms"))
            .args(["mcp", "stdio"])
            .env("DATABASE_URL", database_url)
            .env("HMAC_SECRET", hmac_secret)
            .env("CMS_MCP_TOKEN", token)
            .env("DB_MIN_CONNECTIONS", "1")
            .env("DB_MAX_CONNECTIONS", "2")
            .env("RUST_LOG", "cms=debug")
            .env("LOG_FORMAT", "pretty")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn cms mcp stdio");

        let stdin = child.stdin.take().expect("child stdin");
        let stdout = BufReader::new(child.stdout.take().expect("child stdout"));
        Self { child, stdin, stdout }
    }

    async fn request(&mut self, id: u64, method: &str, params: Option<Value>) -> Value {
        let mut request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
        });
        if let Some(params) = params {
            request["params"] = params;
        }
        self.stdin
            .write_all(format!("{}\n", request).as_bytes())
            .await
            .expect("write MCP request");
        self.stdin.flush().await.expect("flush MCP request");

        let mut line = String::new();
        self.stdout.read_line(&mut line).await.expect("read MCP response");
        serde_json::from_str(&line).unwrap_or_else(|error| panic!("stdout was not pure MCP JSON: {line:?}: {error}"))
    }

    async fn notify(&mut self, method: &str) {
        let request = json!({"jsonrpc": "2.0", "method": method});
        self.stdin
            .write_all(format!("{}\n", request).as_bytes())
            .await
            .expect("write MCP notification");
        self.stdin.flush().await.expect("flush MCP notification");
    }

    async fn close(mut self) -> (std::process::ExitStatus, String) {
        drop(self.stdin);
        let mut stderr = self.child.stderr.take().expect("child stderr");
        let status = self.child.wait().await.expect("wait for stdio process");
        let mut logs = String::new();
        stderr.read_to_string(&mut logs).await.expect("read stderr logs");
        (status, logs)
    }
}

async fn setup_database(permission: AccessTokenPermission) -> (tempfile::TempDir, Config, String, String) {
    let directory = tempfile::tempdir().expect("temp directory");
    let database_path = directory.path().join("cms.db");
    let database_url = format!("sqlite://{}", database_path.to_string_lossy().replace('\\', "/"));
    let hmac_secret = "stdio-test-hmac-secret".to_string();
    let config = Config {
        database_url,
        hmac_secret: hmac_secret.clone(),
        bcrypt_cost: 4,
        db_max_connections: 2,
        db_min_connections: 1,
        db_acquire_timeout_secs: 5,
        db_idle_timeout_secs: 60,
        ..Config::default()
    };
    let pool = init_db_with_config(&config).await.expect("initialize database");
    let repository = Repository::new(&pool);
    let password_hash = bcrypt::hash("password", 4).expect("password hash");
    repository
        .user
        .create("stdio-user", "stdio", "stdio@example.com", &password_hash)
        .await
        .expect("create user");
    repository
        .site
        .create("stdio-site", "Stdio Site", "filesystem", "stdio-user")
        .await
        .expect("create site");
    let token = AccessTokenService::new(repository.access_token.clone(), hmac_secret.clone(), 4)
        .create_site_token("stdio-site", "stdio".to_string(), permission, Some("stdio-user"))
        .await
        .expect("create token");
    drop(repository);
    drop(pool);

    (directory, config, token.id, token.token)
}

async fn initialize(client: &mut StdioClient) {
    let response = client
        .request(
            1,
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "stdio-test", "version": "1.0"}
            })),
        )
        .await;
    assert_eq!(response["result"]["serverInfo"]["name"], "cms");
    client.notify("notifications/initialized").await;
}

#[tokio::test]
async fn stdio_serves_mcp_with_protocol_only_stdout_and_lifecycle_logs_on_stderr() {
    let (_directory, config, _token_id, token) = setup_database(AccessTokenPermission::Write).await;
    let mut client = StdioClient::start(&config.database_url, &config.hmac_secret, &token).await;
    initialize(&mut client).await;

    let tools = client.request(2, "tools/list", None).await;
    assert!(
        tools["result"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
    );

    let site = client
        .request(3, "tools/call", Some(json!({"name": "get_site", "arguments": {}})))
        .await;
    assert_eq!(site["result"]["isError"], false);

    let (status, logs) = client.close().await;
    assert!(status.success());
    assert!(logs.contains("Starting standalone MCP stdio process"));
    assert!(logs.contains("no migrations were run"));
    assert!(logs.contains("MCP stdio transport active"));
    assert!(logs.contains("exited cleanly"));
}

#[tokio::test]
async fn stdio_enforces_read_permission_and_revalidates_deleted_token() {
    let (_directory, config, token_id, token) = setup_database(AccessTokenPermission::Read).await;
    let mut client = StdioClient::start(&config.database_url, &config.hmac_secret, &token).await;
    initialize(&mut client).await;

    let mutation = client
        .request(
            2,
            "tools/call",
            Some(json!({
                "name": "update_site",
                "arguments": {"name": "Forbidden"}
            })),
        )
        .await;
    assert_eq!(mutation["result"]["isError"], true);

    let pool = DbPool::from_existing_with_config(&config).await.expect("open database");
    let repository = Repository::new(&pool);
    repository
        .access_token
        .delete(&token_id, "stdio-site")
        .await
        .expect("delete token");
    drop(repository);
    drop(pool);

    let rejected = client.request(3, "tools/list", None).await;
    assert!(rejected.get("error").is_some(), "deleted token must be rejected");

    let (status, _logs) = client.close().await;
    assert!(status.success());
}
