use crate::common::mcp::*;

async fn setup_singleton(base_url: &str, _token: &str, site_id: &str, slug: &str) {
    let result = create_test_singleton(base_url, site_id, "Settings", slug).await;
    assert!(result["id"].as_str().is_some());
}

#[tokio::test]
async fn test_list_singletons_empty() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;

    let result = mcp_call_site_tool(
        &server.base_url,
        &token,
        &site_id,
        "list_singletons",
        serde_json::json!({}),
    )
    .await;
    let data = mcp_tool_json(&result);

    assert!(data.is_array());
    assert!(data.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_get_singleton() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;

    setup_singleton(&server.base_url, &token, &site_id, "settings").await;

    let result = mcp_call_site_tool(
        &server.base_url,
        &token,
        &site_id,
        "get_singleton",
        serde_json::json!({"include_drafts": true, "slug": "settings"}),
    )
    .await;
    assert!(!mcp_is_error(&result), "get_singleton should succeed");
}

#[tokio::test]
async fn test_update_singleton_data() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;

    setup_singleton(&server.base_url, &token, &site_id, "settings").await;

    let result = mcp_call_site_tool(
        &server.base_url,
        &token,
        &site_id,
        "update_singleton",
        serde_json::json!({
            "slug": "settings",
            "data": {"site_title": "My CMS"}
        }),
    )
    .await;
    assert!(!mcp_is_error(&result), "update_singleton should succeed: {result}");
}

#[tokio::test]
async fn test_list_singletons_after_create() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;

    setup_singleton(&server.base_url, &token, &site_id, "settings").await;

    let result = mcp_call_site_tool(
        &server.base_url,
        &token,
        &site_id,
        "list_singletons",
        serde_json::json!({}),
    )
    .await;
    let data = mcp_tool_json(&result);
    assert_eq!(data.as_array().unwrap().len(), 1);
}
