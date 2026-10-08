use crate::common::mcp::*;

#[tokio::test]
async fn test_initialize_returns_server_info() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let info = mcp_initialize(&server.base_url, &token).await;

    let server_info = info["_meta"]["io.modelcontextprotocol/serverInfo"]
        .as_object()
        .expect("missing namespaced serverInfo");
    assert_eq!(server_info["name"].as_str().unwrap(), "velopulent-cms");
    assert!(server_info.get("version").is_some());

    let capabilities = info.get("capabilities").expect("missing capabilities");
    assert!(capabilities.get("tools").is_some());
    assert!(capabilities.get("resources").is_some());
}

#[tokio::test]
async fn test_discover_advertises_modern_protocol() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_initialize(&server.base_url, &token).await;
    assert_eq!(result["supportedVersions"][0], "2026-07-28");
}

#[tokio::test]
async fn test_list_tools_returns_all_tools() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let tools = mcp_list_tools(&server.base_url, &token).await;

    assert!(tools.len() >= 15, "Expected at least 15 tools, got {}", tools.len());

    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"get_site"));
    assert!(names.contains(&"list_entries"));
    assert!(names.contains(&"get_entry"));
    assert!(names.contains(&"create_entry"));
    assert!(names.contains(&"update_entry"));
    assert!(names.contains(&"delete_entry"));
    assert!(names.contains(&"set_entry_publication"));
    assert!(names.contains(&"list_revisions"));
    assert!(names.contains(&"get_revision"));
    assert!(names.contains(&"restore_revision"));
    assert!(names.contains(&"list_singletons"));
    assert!(names.contains(&"get_singleton"));
    assert!(names.contains(&"update_singleton"));
    assert!(names.contains(&"list_files"));
    assert!(names.contains(&"get_file"));
    assert!(names.contains(&"create_file_upload"));
    assert!(names.contains(&"delete_file"));
    assert!(names.contains(&"restore_file"));
    assert!(!names.iter().any(|name| name.contains("webhook")));
}

#[tokio::test]
async fn test_tool_schemas_are_valid() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let tools = mcp_list_tools(&server.base_url, &token).await;

    for tool in &tools {
        let name = tool["name"].as_str().unwrap();
        let schema = tool
            .get("inputSchema")
            .unwrap_or_else(|| panic!("tool '{}' missing inputSchema", name));

        assert_eq!(
            schema["type"].as_str(),
            Some("object"),
            "tool '{}' inputSchema must have type 'object'",
            name
        );
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            for (key, prop) in props {
                assert!(
                    prop.is_object(),
                    "tool '{}' property '{}' is not an object: {:?}",
                    name,
                    key,
                    prop
                );
                assert!(!prop.is_boolean(), "tool '{}' property '{}' is boolean", name, key);
            }
        }
    }
}

#[tokio::test]
async fn test_list_sites_tool_is_available() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let tools = mcp_list_tools(&server.base_url, &token).await;
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"list_sites"), "list_sites tool should be available");
}

#[tokio::test]
async fn test_call_nonexistent_tool_returns_error() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let resp = mcp_request(
        &server.base_url,
        &token,
        "tools/call",
        Some(serde_json::json!({
            "name": "nonexistent_tool",
            "arguments": {}
        })),
    )
    .await;

    assert!(
        resp.get("error").is_some(),
        "Expected JSON-RPC error for nonexistent tool"
    );
}

#[tokio::test]
async fn test_auth_missing_token_returns_401() {
    let server = start_mcp_server().await;

    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/mcp", server.base_url))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_auth_wrong_token_type_returns_401() {
    let server = start_mcp_server().await;
    let response = mcp_request(&server.base_url, "not-a-cms-token", "tools/list", None).await;
    assert_eq!(response["http_status"], 401);
    assert_eq!(response["transport_error"]["error"], "MCP requires a VCMS access token");
}

#[tokio::test]
async fn test_auth_invalid_token_returns_401() {
    let server = start_mcp_server().await;
    let response = mcp_request(&server.base_url, "vcms_site_invalid_token_abc123", "tools/list", None).await;
    assert_eq!(response["http_status"], 401);
    assert_eq!(response["transport_error"]["error"], "Invalid access token");
}

#[tokio::test]
async fn test_auth_instance_token_rejected() {
    let server = start_mcp_server().await;
    let response = mcp_request(
        &server.base_url,
        "cms_inst_abcdefghijklmnopqrstuvwxyz",
        "tools/list",
        None,
    )
    .await;
    assert_eq!(response["http_status"], 401);
    assert_eq!(response["transport_error"]["error"], "MCP requires a VCMS access token");
}

#[tokio::test]
async fn unsupported_capabilities_do_not_inherit_successful_sdk_defaults() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    for method in ["prompts/list", "completion/complete"] {
        let params = if method == "completion/complete" {
            Some(
                serde_json::json!({"ref":{"type":"ref/prompt","name":"unsupported"},"argument":{"name":"value","value":"x"}}),
            )
        } else {
            None
        };
        let response = mcp_request(&server.base_url, &token, method, params).await;
        assert_eq!(
            response["error"]["code"], -32601,
            "Undeclared capability handled: {response}"
        );
    }
    let response = mcp_request(&server.base_url, &token, "tools/list", None).await;
    assert_eq!(response["result"]["ttlMs"], 10_000);
    assert_eq!(response["result"]["cacheScope"], "public");
}

#[tokio::test]
async fn default_origin_policy_accepts_same_origin_and_rejects_other_origins() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientCapabilities":{}}}});
    let client = crate::common::client::http_client();
    for (origin, status) in [(&server.base_url, 200), (&"http://evil.example".to_owned(), 403)] {
        let response = client
            .post(format!("{}/mcp", server.base_url))
            .bearer_auth(&token)
            .header("origin", origin)
            .header("accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2026-07-28")
            .header("Mcp-Method", "tools/list")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status, "Origin policy failed for {origin}");
    }
}

#[tokio::test]
async fn unknown_resource_uses_native_not_found_code_and_uri() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let uri = "test://missing-resource";
    let response = mcp_request(
        &server.base_url,
        &token,
        "resources/read",
        Some(serde_json::json!({"uri":uri})),
    )
    .await;
    assert_eq!(response["error"]["code"], -32602);
    assert_eq!(response["error"]["data"]["uri"], uri);
}
