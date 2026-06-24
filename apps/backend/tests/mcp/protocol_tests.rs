use crate::common::mcp::*;

#[tokio::test]
async fn test_initialize_returns_server_info() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let info = mcp_initialize(&server.base_url, &token).await;

    let server_info = info.get("serverInfo").expect("missing serverInfo");
    assert_eq!(server_info["name"].as_str().unwrap(), "cms");
    assert!(server_info.get("version").is_some());

    let capabilities = info.get("capabilities").expect("missing capabilities");
    assert!(capabilities.get("tools").is_some());
    assert!(capabilities.get("resources").is_some());
}

#[tokio::test]
async fn test_initialize_reflects_protocol_version() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let resp = mcp_request(
        &server.base_url,
        &token,
        "initialize",
        Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        })),
    )
    .await;
    let result = mcp_result(&resp);
    assert_eq!(result["protocolVersion"].as_str().unwrap(), "2024-11-05");
}

#[tokio::test]
async fn test_list_tools_returns_all_tools() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let tools = mcp_list_tools(&server.base_url, &token).await;

    assert!(tools.len() >= 28, "Expected at least 28 tools, got {}", tools.len());

    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"get_site"));
    assert!(names.contains(&"update_site"));
    assert!(names.contains(&"list_collections"));
    assert!(names.contains(&"get_collection"));
    assert!(names.contains(&"create_collection"));
    assert!(names.contains(&"update_collection"));
    assert!(names.contains(&"delete_collection"));
    assert!(names.contains(&"list_entries"));
    assert!(names.contains(&"get_entry"));
    assert!(names.contains(&"create_entry"));
    assert!(names.contains(&"update_entry"));
    assert!(names.contains(&"delete_entry"));
    assert!(names.contains(&"publish_entry"));
    assert!(names.contains(&"unpublish_entry"));
    assert!(names.contains(&"list_revisions"));
    assert!(names.contains(&"restore_revision"));
    assert!(names.contains(&"list_singletons"));
    assert!(names.contains(&"get_singleton"));
    assert!(names.contains(&"update_singleton"));
    assert!(names.contains(&"list_files"));
    assert!(names.contains(&"get_file"));
    assert!(names.contains(&"create_upload_url"));
    assert!(names.contains(&"delete_file"));
    assert!(names.contains(&"restore_file"));
    assert!(names.contains(&"list_webhooks"));
    assert!(names.contains(&"get_webhook"));
    assert!(names.contains(&"create_webhook"));
    assert!(names.contains(&"update_webhook"));
    assert!(names.contains(&"trigger_webhook"));
    assert!(names.contains(&"delete_webhook"));
    assert!(names.contains(&"list_webhook_deliveries"));
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
        assert!(
            schema.get("$schema").is_none(),
            "tool '{}' inputSchema has $schema",
            name
        );
        assert!(schema.get("title").is_none(), "tool '{}' inputSchema has title", name);

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
async fn test_no_list_sites_tool() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let tools = mcp_list_tools(&server.base_url, &token).await;
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(!names.contains(&"list_sites"), "list_sites tool should not exist");
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

    let resp = mcp_request(&server.base_url, "not-a-cms-token", "tools/list", None).await;

    let error = resp.get("error");
    assert!(error.is_some(), "Expected error for wrong token type, got: {}", resp);
    let error = error.unwrap();
    assert_eq!(
        error["code"].as_i64().unwrap(),
        -32000 + 401,
        "Expected synthesized 401 error code, got: {}",
        error["code"]
    );
    let msg = error["message"].as_str().unwrap();
    assert!(
        msg.contains("MCP requires a vcms_site_* access token"),
        "Expected auth error message, got: {}",
        msg
    );
}

#[tokio::test]
async fn test_auth_invalid_token_returns_401() {
    let server = start_mcp_server().await;

    let resp = mcp_request(&server.base_url, "vcms_site_invalid_token_abc123", "tools/list", None).await;

    let error = resp.get("error");
    assert!(error.is_some(), "Expected error for invalid token, got: {}", resp);
    let msg = error.unwrap()["message"].as_str().unwrap();
    assert!(
        msg.contains("Invalid access token") || msg.contains("error"),
        "Expected auth error message, got: {}",
        msg
    );
}

#[tokio::test]
async fn test_auth_instance_token_rejected() {
    let server = start_mcp_server().await;

    let resp = mcp_request(
        &server.base_url,
        "cms_inst_abcdefghijklmnopqrstuvwxyz",
        "tools/list",
        None,
    )
    .await;

    let error = resp.get("error");
    assert!(error.is_some(), "Instance token should be rejected, got: {}", resp);
    let msg = error.unwrap()["message"].as_str().unwrap();
    assert!(
        msg.contains("MCP requires a vcms_site_* access token"),
        "Expected MCP token error, got: {}",
        msg
    );
}
