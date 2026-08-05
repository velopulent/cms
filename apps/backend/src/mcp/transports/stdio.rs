//! `vcms mcp stdio` — a thin proxy between an MCP client's stdin/stdout and the
//! running server's Streamable HTTP `/mcp` endpoint.
//!
//! The stdio process runs under the *invoking* user from an arbitrary cwd, while the
//! server runs as the (often privileged) service account that owns the database,
//! secrets, and search index. Rather than open those files directly — which fails
//! when they belong to the service account — this proxy forwards JSON-RPC over HTTP
//! and lets the server do all disk I/O. It needs only a URL and a VCMS access
//! token, which it injects as the `Authorization: Bearer` header.
//!
//! The server runs the HTTP transport in stateless, JSON-response mode (no SSE, no
//! `Mcp-Session-Id`), so a flat per-message proxy is sufficient: read a
//! newline-delimited JSON-RPC message from stdin, POST it, write the JSON reply back.

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Run the proxy loop until stdin closes (EOF → clean exit).
///
/// `endpoint` is the fully-qualified MCP URL (e.g. `http://127.0.0.1:3000/mcp`) and
/// `token` is the VCMS access token forwarded as the bearer credential.
pub async fn serve(endpoint: String, token: String) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    tracing::info!(%endpoint, "MCP stdio proxy active");

    while let Some(line) = lines.next_line().await? {
        let message = line.trim();
        if message.is_empty() {
            continue;
        }
        if let Some(response) = forward(&client, &endpoint, &token, message).await {
            stdout.write_all(response.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }

    tracing::info!("MCP stdio proxy stopped (stdin closed)");
    Ok(())
}

/// Forward one JSON-RPC message to the server and return the line to write to stdout,
/// or `None` when nothing should be written (a notification, which carries no `id`
/// and expects no reply). On a transport failure for a request we synthesize a
/// JSON-RPC error so the client sees a clean failure and the proxy keeps running.
async fn forward(client: &reqwest::Client, endpoint: &str, token: &str, message: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(message).ok();
    // Requests and batches expect a response; a lone notification object (no `id`)
    // does not. Unparseable input is forwarded so the server returns a proper
    // JSON-RPC parse error.
    let (needs_response, id) = match &parsed {
        Some(Value::Object(obj)) => (obj.contains_key("id"), obj.get("id").cloned()),
        Some(Value::Array(_)) => (true, None), // batch
        _ => (true, None),                     // unparseable → let the server reject it
    };

    let result = client
        .post(endpoint)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/json, text/event-stream")
        .body(message.to_string())
        .send()
        .await;

    let response = match result {
        Ok(response) => response,
        Err(error) => {
            return error_line(
                needs_response,
                &id,
                format!("cannot reach vcms server at {endpoint}: {error}"),
            );
        }
    };

    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();

    let body = match response.text().await {
        Ok(body) => body,
        Err(error) => {
            return error_line(
                needs_response,
                &id,
                format!("reading vcms server response failed: {error}"),
            );
        }
    };

    if !needs_response {
        return None; // notification: server replies 202 with an empty body
    }

    // A non-2xx is an auth/host/transport failure (e.g. 401 for a bad token), not a
    // JSON-RPC reply. Wrap it so the client sees a proper JSON-RPC error envelope.
    if !status.is_success() {
        let detail = http_error_detail(&body);
        return error_line(needs_response, &id, format!("vcms server returned {status}: {detail}"));
    }

    // json-response mode returns `application/json`; defensively unwrap a single SSE
    // frame if the server ever streams one back.
    let payload = if content_type.contains("text/event-stream") {
        extract_sse_data(&body).unwrap_or(body)
    } else {
        body
    };
    let payload = payload.trim();
    if payload.is_empty() {
        return error_line(
            needs_response,
            &id,
            "vcms server returned an empty response".to_string(),
        );
    }

    // Re-serialize compactly so the line carries no embedded newlines (MCP stdio
    // framing is one message per line). Fall back to the raw payload if it isn't JSON.
    Some(
        serde_json::from_str::<Value>(payload)
            .map(|value| value.to_string())
            .unwrap_or_else(|_| payload.replace('\n', " ")),
    )
}

/// Build a JSON-RPC error line for a failed request, or `None` for a notification
/// (which has no `id` to respond to).
fn error_line(needs_response: bool, id: &Option<Value>, message: String) -> Option<String> {
    if !needs_response {
        tracing::warn!(%message, "MCP notification forward failed");
        return None;
    }
    let error = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id.clone().unwrap_or(Value::Null),
        "error": { "code": -32603, "message": message },
    });
    Some(error.to_string())
}

/// Concatenate the `data:` lines of an SSE payload into a single JSON string.
fn extract_sse_data(body: &str) -> Option<String> {
    let mut data = String::new();
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            data.push_str(rest.trim_start());
        }
    }
    (!data.is_empty()).then_some(data)
}

/// Pull a human-readable detail out of an error response body. The server's auth
/// failures are `{"error": "..."}`; fall back to the raw trimmed body otherwise.
fn http_error_detail(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value.get("error").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| body.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Bytes;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use std::net::SocketAddr;

    /// A stub `/mcp` that echoes the received Authorization/Accept headers back inside
    /// a JSON-RPC result (for requests) and 202-accepts notifications.
    async fn spawn_stub() -> SocketAddr {
        let app = Router::new().route(
            "/mcp",
            post(|headers: HeaderMap, body: Bytes| async move {
                let header = |name: &str| {
                    headers
                        .get(name)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string()
                };
                let auth = header("authorization");
                let accept = header("accept");
                let parsed: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                let response_headers = [("content-type", "application/json")];
                match parsed.get("id") {
                    None => (StatusCode::ACCEPTED, response_headers, String::new()),
                    Some(id) => {
                        let reply = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": { "auth": auth, "accept": accept },
                        });
                        (StatusCode::OK, response_headers, reply.to_string())
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        addr
    }

    #[tokio::test]
    async fn forwards_bearer_and_relays_response() {
        let addr = spawn_stub().await;
        let endpoint = format!("http://{addr}/mcp");
        let out = forward(
            &reqwest::Client::new(),
            &endpoint,
            "vcms_site_test",
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        )
        .await
        .expect("a request must produce a response line");
        let value: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["id"], 1);
        assert_eq!(value["result"]["auth"], "Bearer vcms_site_test");
        assert!(value["result"]["accept"].as_str().unwrap().contains("application/json"));
    }

    #[tokio::test]
    async fn notification_produces_no_output() {
        let addr = spawn_stub().await;
        let endpoint = format!("http://{addr}/mcp");
        let out = forward(
            &reqwest::Client::new(),
            &endpoint,
            "t",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        )
        .await;
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn unreachable_server_yields_jsonrpc_error_for_request() {
        // Port 1 refuses immediately on all supported platforms.
        let out = forward(
            &reqwest::Client::new(),
            "http://127.0.0.1:1/mcp",
            "t",
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/list"}"#,
        )
        .await
        .expect("a request must produce an error line");
        let value: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["id"], 7);
        assert_eq!(value["error"]["code"], -32603);
    }

    #[tokio::test]
    async fn unreachable_server_swallows_notification() {
        let out = forward(
            &reqwest::Client::new(),
            "http://127.0.0.1:1/mcp",
            "t",
            r#"{"jsonrpc":"2.0","method":"ping"}"#,
        )
        .await;
        assert!(out.is_none());
    }
}
