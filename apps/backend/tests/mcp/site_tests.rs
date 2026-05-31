use crate::mcp_helpers::*;

#[tokio::test]
async fn test_get_site() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(&server.base_url, &token, "get_site", serde_json::json!({})).await;
    let site = mcp_tool_json(&result);

    assert_eq!(site["id"].as_str().unwrap(), site_id);
    assert_eq!(site["name"].as_str().unwrap(), "Test Site");
}

#[tokio::test]
async fn test_update_site_name() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_site",
        serde_json::json!({"name": "Updated Site"}),
    )
    .await;
    let site = mcp_tool_json(&result);

    assert_eq!(site["name"].as_str().unwrap(), "Updated Site");
}

#[tokio::test]
async fn test_get_site_works_with_read_token() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_read_token(&server).await;

    let result = mcp_call_tool(&server.base_url, &token, "get_site", serde_json::json!({})).await;
    assert!(!mcp_is_error(&result), "get_site should succeed with read token");

    let site = mcp_tool_json(&result);
    assert_eq!(site["id"].as_str().unwrap(), site_id);
}

#[tokio::test]
async fn test_update_site_requires_admin() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_read_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_site",
        serde_json::json!({"name": "Should Fail"}),
    )
    .await;
    assert!(
        mcp_is_error(&result),
        "update_site should fail with read-only token"
    );
}
