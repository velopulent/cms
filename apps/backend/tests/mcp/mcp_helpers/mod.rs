use serde_json::Value;

use crate::common::TestServer;

use std::sync::atomic::{AtomicU32, Ordering};

static REQUEST_ID: AtomicU32 = AtomicU32::new(0);

fn next_id() -> u32 {
    REQUEST_ID.fetch_add(1, Ordering::Relaxed) + 1
}

fn extract_cookies(resp: &reqwest::Response) -> (String, String) {
    let mut jwt = String::new();
    let mut csrf = String::new();
    for cookie in resp.headers().get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if val.starts_with("token=") {
                jwt = val
                    .split(';')
                    .next()
                    .and_then(|c| c.strip_prefix("token="))
                    .unwrap_or("")
                    .to_string();
            }
            if val.starts_with("csrf=") {
                csrf = val
                    .split(';')
                    .next()
                    .and_then(|c| c.strip_prefix("csrf="))
                    .unwrap_or("")
                    .to_string();
            }
        }
    }
    (jwt, csrf)
}

fn auth_header(jwt: &str, csrf: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    let cookie_val = format!("token={}; csrf={}", jwt, csrf);
    headers.insert(
        reqwest::header::COOKIE,
        reqwest::header::HeaderValue::from_str(&cookie_val).unwrap(),
    );
    headers.insert(
        "X-CSRF-Token",
        reqwest::header::HeaderValue::from_str(csrf).unwrap(),
    );
    headers
}

pub async fn start_mcp_server() -> TestServer {
    TestServer::start().await
}

pub async fn setup_site_token(server: &TestServer) -> (String, String) {
    create_site_and_token(server, "write").await
}

pub async fn setup_site_read_token(server: &TestServer) -> (String, String) {
    create_site_and_token(server, "read").await
}

async fn create_site_and_token(server: &TestServer, permission: &str) -> (String, String) {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = server.login_user(&client, "admin", "admin").await;
    let (jwt, csrf) = extract_cookies(&resp);

    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .headers(auth_header(&jwt, &csrf))
        .json(&serde_json::json!({"name": "Test Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "Create site failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    let site: serde_json::Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap().to_string();

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/tokens",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .json(&serde_json::json!({"name": "Test Token", "permission": permission}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "Create token failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    let token_val: serde_json::Value = resp.json().await.unwrap();
    let token = token_val["token"].as_str().unwrap().to_string();

    (site_id, token)
}

pub async fn mcp_request(
    base_url: &str,
    token: &str,
    method: &str,
    params: Option<Value>,
) -> Value {
    let client = reqwest::Client::builder().build().unwrap();
    let id = next_id();
    let mut body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
    });
    if let Some(p) = params {
        body["params"] = p;
    }

    let resp = client
        .post(format!("{}/mcp", base_url))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("Authorization", format!("Bearer {}", token))
        .json(&body)
        .send()
        .await
        .expect("Failed to send MCP request");

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32000 + status as i64,
                "message": text,
            }
        });
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let text = resp.text().await.unwrap();

    if content_type.contains("text/event-stream") {
        parse_sse_response(&text)
    } else {
        serde_json::from_str(&text).unwrap_or_else(|e| {
            panic!("Failed to parse JSON response: {}\nBody: {}", e, text);
        })
    }
}

fn parse_sse_response(text: &str) -> Value {
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ")
            && let Ok(val) = serde_json::from_str::<Value>(data)
            && (val.get("result").is_some() || val.get("error").is_some())
        {
            return val;
        }
    }
    panic!("No JSON-RPC response found in SSE stream:\n{}", text);
}

pub fn mcp_result(response: &Value) -> &Value {
    if let Some(err) = response.get("error") {
        panic!("MCP error: {}", err);
    }
    response
        .get("result")
        .expect("MCP response missing 'result'")
}

pub fn mcp_is_error(response: &Value) -> bool {
    let result = if response.get("result").is_some() {
        response.get("result").unwrap()
    } else {
        response
    };
    result
        .get("isError")
        .or_else(|| result.get("is_error"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn mcp_tool_text(result: &Value) -> String {
    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .expect("result missing 'content' array");
    assert!(!content.is_empty(), "result content is empty");
    content[0]
        .get("text")
        .and_then(|t| t.as_str())
        .expect("content[0] missing 'text'")
        .to_string()
}

pub fn mcp_tool_json(result: &Value) -> Value {
    let text = mcp_tool_text(result);
    serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!("Failed to parse tool result as JSON: {}\nText: {}", e, text);
    })
}

pub async fn mcp_initialize(base_url: &str, token: &str) -> Value {
    let resp = mcp_request(
        base_url,
        token,
        "initialize",
        Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test-client", "version": "1.0.0"}
        })),
    )
    .await;
    mcp_result(&resp).clone()
}

pub async fn mcp_list_tools(base_url: &str, token: &str) -> Vec<Value> {
    let resp = mcp_request(base_url, token, "tools/list", None).await;
    let result = mcp_result(&resp);
    result["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

pub async fn mcp_call_tool(
    base_url: &str,
    token: &str,
    tool_name: &str,
    arguments: Value,
) -> Value {
    let resp = mcp_request(
        base_url,
        token,
        "tools/call",
        Some(serde_json::json!({
            "name": tool_name,
            "arguments": arguments,
        })),
    )
    .await;
    mcp_result(&resp).clone()
}

pub async fn mcp_list_resources(base_url: &str, token: &str) -> Vec<Value> {
    let resp = mcp_request(base_url, token, "resources/list", None).await;
    let result = mcp_result(&resp);
    result["resources"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

pub async fn mcp_read_resource(base_url: &str, token: &str, uri: &str) -> Value {
    let resp = mcp_request(
        base_url,
        token,
        "resources/read",
        Some(serde_json::json!({"uri": uri})),
    )
    .await;
    mcp_result(&resp).clone()
}

pub async fn create_test_collection(
    base_url: &str,
    token: &str,
    name: &str,
    slug: &str,
) -> Value {
    let result = mcp_call_tool(
        base_url,
        token,
        "create_collection",
        serde_json::json!({
            "name": name,
            "slug": slug,
            "definition": {"fields": [{"name": "title", "type": "text", "required": true}]},
        }),
    )
    .await;
    assert!(
        !mcp_is_error(&result),
        "create_collection failed: {}",
        mcp_tool_text(&result)
    );
    result
}

pub async fn create_test_entry(
    base_url: &str,
    token: &str,
    collection_id: &str,
    slug: &str,
    data: Value,
) -> Value {
    let result = mcp_call_tool(
        base_url,
        token,
        "create_entry",
        serde_json::json!({
            "collection_id": collection_id,
            "slug": slug,
            "values": data,
        }),
    )
    .await;
    assert!(
        !mcp_is_error(&result),
        "create_entry failed: {}",
        mcp_tool_text(&result)
    );
    result
}
