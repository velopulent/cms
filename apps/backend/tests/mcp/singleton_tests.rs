use crate::mcp_helpers::*;

async fn setup_singleton(base_url: &str, token: &str, slug: &str) {
    let result = mcp_call_tool(
        base_url,
        token,
        "create_collection",
        serde_json::json!({
            "name": "Settings",
            "slug": slug,
            "definition": {"fields": [{"name": "site_title", "type": "text"}]},
            "is_singleton": true,
        }),
    )
    .await;
    assert!(
        !mcp_is_error(&result),
        "setup_singleton: create_collection failed: {}",
        mcp_tool_text(&result)
    );
}

#[tokio::test]
async fn test_list_singletons_empty() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result =
        mcp_call_tool(&server.base_url, &token, "list_singletons", serde_json::json!({})).await;
    let data = mcp_tool_json(&result);

    assert!(data.is_array());
    assert!(data.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_get_singleton() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    setup_singleton(&server.base_url, &token, "settings").await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_singleton",
        serde_json::json!({"slug": "settings"}),
    )
    .await;
    assert!(!mcp_is_error(&result), "get_singleton should succeed");
}

#[tokio::test]
async fn test_update_singleton_data() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    setup_singleton(&server.base_url, &token, "settings").await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_singleton",
        serde_json::json!({
            "slug": "settings",
            "data": {"site_title": "My CMS"}
        }),
    )
    .await;
    assert!(!mcp_is_error(&result), "update_singleton should succeed");
}

#[tokio::test]
async fn test_list_singletons_after_create() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    setup_singleton(&server.base_url, &token, "settings").await;

    let result =
        mcp_call_tool(&server.base_url, &token, "list_singletons", serde_json::json!({})).await;
    let data = mcp_tool_json(&result);
    assert_eq!(data.as_array().unwrap().len(), 1);
}
