use crate::common::mcp::*;

#[tokio::test]
async fn test_get_site() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;

    let result = mcp_call_site_tool(&server.base_url, &token, &site_id, "get_site", serde_json::json!({})).await;
    let site = mcp_tool_json(&result);

    assert_eq!(site["id"].as_str().unwrap(), site_id);
    assert_eq!(site["name"].as_str().unwrap(), "Test Site");
}

#[tokio::test]
async fn test_get_site_works_with_read_token() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_read_token(&server).await;

    let result = mcp_call_site_tool(&server.base_url, &token, &site_id, "get_site", serde_json::json!({})).await;
    assert!(!mcp_is_error(&result), "get_site should succeed with read token");

    let site = mcp_tool_json(&result);
    assert_eq!(site["id"].as_str().unwrap(), site_id);
}

#[tokio::test]
async fn site_management_tools_are_not_exposed() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_read_token(&server).await;
    let tools = mcp_list_tools(&server.base_url, &token).await;
    assert!(tools.iter().any(|tool| tool["name"] == "get_site"));
    for name in [
        "update_site",
        "create_collection",
        "update_collection",
        "delete_collection",
        "create_webhook",
    ] {
        assert!(
            !tools.iter().any(|tool| tool["name"] == name),
            "Public management tool exposed: {name}"
        );
    }
}
