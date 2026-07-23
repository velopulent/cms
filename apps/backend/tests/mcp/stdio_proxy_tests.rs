//! End-to-end tests for `vcms mcp stdio` as a thin HTTP proxy.
//!
//! The real `vcms` binary is spawned in stdio mode and pointed at a live
//! `TestServer`'s `/mcp` endpoint via `VCMS_MCP_URL` + `VCMS_MCP_TOKEN`. It touches
//! no database or secrets of its own — everything is forwarded to the server.

use std::process::Stdio;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::common::{TestServer, fixtures};

struct StdioClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl StdioClient {
    /// Spawn the proxy the way an MCP client does: it knows only the server URL and a
    /// access token and no server configuration.
    async fn start(url: &str, token: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_vcms"))
            .args(["mcp", "stdio"])
            .env("VCMS_MCP_URL", url)
            .env("VCMS_MCP_TOKEN", token)
            .env_remove("DATABASE_URL")
            .env_remove("HMAC_SECRET")
            .env_remove("VCMS_HOME")
            .env("RUST_LOG", "warn")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn vcms mcp stdio");

        let stdin = child.stdin.take().expect("child stdin");
        let stdout = BufReader::new(child.stdout.take().expect("child stdout"));
        Self { child, stdin, stdout }
    }

    async fn request(&mut self, id: u64, method: &str, params: Option<Value>) -> Value {
        let mut request = json!({"jsonrpc": "2.0", "id": id, "method": method});
        if let Some(params) = params {
            request["params"] = params;
        }
        self.stdin
            .write_all(format!("{request}\n").as_bytes())
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
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("write MCP notification");
        self.stdin.flush().await.expect("flush MCP notification");
    }

    async fn close(mut self) -> std::process::ExitStatus {
        drop(self.stdin);
        let mut stderr = self.child.stderr.take().expect("child stderr");
        let status = self.child.wait().await.expect("wait for stdio process");
        let mut logs = String::new();
        stderr.read_to_string(&mut logs).await.expect("read stderr logs");
        status
    }
}

async fn initialize(client: &mut StdioClient) -> Value {
    let response = client
        .request(
            1,
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "stdio-proxy-test", "version": "1.0"}
            })),
        )
        .await;
    client.notify("notifications/initialized").await;
    response
}

#[tokio::test]
async fn stdio_proxy_round_trips_tools_and_calls_through_the_server() {
    let server = TestServer::start().await;
    let (site_id, token) = fixtures::create_site_and_token(&server, "write").await;

    let mut client = StdioClient::start(&server.base_url, &token).await;
    let init = initialize(&mut client).await;
    assert_eq!(init["result"]["serverInfo"]["name"], "cms");

    let tools = client.request(2, "tools/list", None).await;
    assert!(
        tools["result"]["tools"].as_array().is_some_and(|t| !t.is_empty()),
        "tools/list should return the site's tools: {tools}"
    );

    let site = client
        .request(
            3,
            "tools/call",
            Some(json!({"name": "get_site", "arguments": {"site_id": site_id}})),
        )
        .await;
    assert_eq!(
        site["result"]["isError"], false,
        "get_site should succeed through the proxy: {site}"
    );

    assert!(client.close().await.success(), "proxy should exit cleanly on stdin EOF");
}

#[tokio::test]
async fn stdio_proxy_enforces_token_permission() {
    let server = TestServer::start().await;
    // A read-only token: reads pass, writes are denied by the server.
    let (site_id, token) = fixtures::create_site_and_token(&server, "read").await;

    let mut client = StdioClient::start(&server.base_url, &token).await;
    initialize(&mut client).await;

    let mutation = client
        .request(
            2,
            "tools/call",
            Some(json!({
                "name": "update_site",
                "arguments": {"site_id": site_id, "name": "Forbidden"}
            })),
        )
        .await;
    assert_eq!(
        mutation["result"]["isError"], true,
        "read token must not write: {mutation}"
    );

    assert!(client.close().await.success());
}

#[tokio::test]
async fn stdio_proxy_wraps_bad_token_as_jsonrpc_error() {
    let server = TestServer::start().await;

    let mut client = StdioClient::start(&server.base_url, "vcms_site_definitely_invalid").await;
    let response = client
        .request(
            1,
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "bad", "version": "1.0"}
            })),
        )
        .await;
    assert!(
        response.get("error").is_some(),
        "an invalid token must surface as a JSON-RPC error: {response}"
    );

    assert!(client.close().await.success());
}
