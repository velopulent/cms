use crate::mcp_helpers::*;

#[tokio::test]
async fn test_list_webhooks_empty() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result =
        mcp_call_tool(&server.base_url, &token, "list_webhooks", serde_json::json!({})).await;
    let data = mcp_tool_json(&result);

    assert!(data.is_array());
    assert!(data.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_webhook() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_webhook",
        serde_json::json!({
            "label": "Test Hook",
            "url": "https://example.com/hook",
        }),
    )
    .await;
    let webhook = mcp_tool_json(&result);

    assert_eq!(webhook["label"].as_str().unwrap(), "Test Hook");
    assert_eq!(
        webhook["url"].as_str().unwrap(),
        "https://example.com/hook"
    );
    assert!(!webhook["id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_get_webhook() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_webhook",
        serde_json::json!({
            "label": "Test Hook",
            "url": "https://example.com/hook",
        }),
    )
    .await;
    let webhook = mcp_tool_json(&result);
    let hook_id = webhook["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_webhook",
        serde_json::json!({"webhook_id": hook_id}),
    )
    .await;
    let fetched = mcp_tool_json(&result);
    assert_eq!(fetched["label"].as_str().unwrap(), "Test Hook");
}

#[tokio::test]
async fn test_get_webhook_not_found() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_webhook",
        serde_json::json!({"webhook_id": "nonexistent"}),
    )
    .await;
    assert!(mcp_is_error(&result), "Should return isError for non-existent webhook");
}

#[tokio::test]
async fn test_update_webhook() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_webhook",
        serde_json::json!({
            "label": "Old Label",
            "url": "https://example.com/hook",
        }),
    )
    .await;
    let webhook = mcp_tool_json(&result);
    let hook_id = webhook["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_webhook",
        serde_json::json!({
            "webhook_id": hook_id,
            "label": "New Label",
        }),
    )
    .await;
    let updated = mcp_tool_json(&result);
    assert_eq!(updated["label"].as_str().unwrap(), "New Label");
}

#[tokio::test]
async fn test_delete_webhook() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_webhook",
        serde_json::json!({
            "label": "To Delete",
            "url": "https://example.com/hook",
        }),
    )
    .await;
    let webhook = mcp_tool_json(&result);
    let hook_id = webhook["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "delete_webhook",
        serde_json::json!({"webhook_id": hook_id}),
    )
    .await;
    let data = mcp_tool_json(&result);
    assert!(data["deleted"].as_bool().unwrap());
}

#[tokio::test]
async fn test_list_webhook_deliveries() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_webhook",
        serde_json::json!({
            "label": "Delivery Test",
            "url": "https://example.com/hook",
        }),
    )
    .await;
    let webhook = mcp_tool_json(&result);
    let hook_id = webhook["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "list_webhook_deliveries",
        serde_json::json!({"webhook_id": hook_id}),
    )
    .await;
    let data = mcp_tool_json(&result);
    assert!(data["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_webhook_requires_admin() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_read_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_webhook",
        serde_json::json!({
            "label": "Should Fail",
            "url": "https://example.com/hook",
        }),
    )
    .await;
    assert!(mcp_is_error(&result), "Viewer should not create webhook");
}

#[tokio::test]
async fn test_webhook_full_lifecycle() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_webhook",
        serde_json::json!({
            "label": "Lifecycle Hook",
            "url": "https://example.com/hook",
        }),
    )
    .await;
    let webhook = mcp_tool_json(&result);
    let hook_id = webhook["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_webhook",
        serde_json::json!({"webhook_id": hook_id}),
    )
    .await;
    let fetched = mcp_tool_json(&result);
    assert_eq!(fetched["label"].as_str().unwrap(), "Lifecycle Hook");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_webhook",
        serde_json::json!({
            "webhook_id": hook_id,
            "label": "Updated Hook",
        }),
    )
    .await;
    let updated = mcp_tool_json(&result);
    assert_eq!(updated["label"].as_str().unwrap(), "Updated Hook");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "list_webhook_deliveries",
        serde_json::json!({"webhook_id": hook_id}),
    )
    .await;
    let deliveries = mcp_tool_json(&result);
    assert!(deliveries["items"].as_array().unwrap().is_empty());

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "delete_webhook",
        serde_json::json!({"webhook_id": hook_id}),
    )
    .await;
    let data = mcp_tool_json(&result);
    assert!(data["deleted"].as_bool().unwrap());
}
