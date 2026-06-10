use crate::mcp_helpers::*;

#[tokio::test]
async fn test_list_collections_empty() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(&server.base_url, &token, "list_collections", serde_json::json!({})).await;
    let data = mcp_tool_json(&result);

    assert!(data.is_array(), "Expected array, got: {}", data);
    assert!(data.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_collection() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_collection",
        serde_json::json!({
            "name": "Posts",
            "slug": "posts",
            "definition": {"fields": [{"name": "title", "type": "text", "required": true}]},
        }),
    )
    .await;
    let col = mcp_tool_json(&result);

    assert_eq!(col["name"].as_str().unwrap(), "Posts");
    assert_eq!(col["slug"].as_str().unwrap(), "posts");
    assert!(!col["id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_collection_auto_slug() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_collection",
        serde_json::json!({
            "name": "My Collection",
            "definition": {"fields": []},
        }),
    )
    .await;
    let col = mcp_tool_json(&result);

    assert_eq!(col["slug"].as_str().unwrap(), "my-collection");
}

#[tokio::test]
async fn test_get_collection() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    create_test_collection(&server.base_url, &token, "Posts", "posts").await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_collection",
        serde_json::json!({"slug": "posts"}),
    )
    .await;
    let col = mcp_tool_json(&result);

    assert_eq!(col["name"].as_str().unwrap(), "Posts");
    assert_eq!(col["slug"].as_str().unwrap(), "posts");
}

#[tokio::test]
async fn test_get_collection_not_found() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_collection",
        serde_json::json!({"slug": "nonexistent"}),
    )
    .await;
    assert!(
        mcp_is_error(&result),
        "Should return isError for non-existent collection"
    );
}

#[tokio::test]
async fn test_update_collection_name() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    create_test_collection(&server.base_url, &token, "Posts", "posts").await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_collection",
        serde_json::json!({
            "slug": "posts",
            "name": "Articles"
        }),
    )
    .await;
    let col = mcp_tool_json(&result);
    assert_eq!(col["name"].as_str().unwrap(), "Articles");
}

#[tokio::test]
async fn test_update_collection_definition() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    create_test_collection(&server.base_url, &token, "Posts", "posts").await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_collection",
        serde_json::json!({
            "slug": "posts",
            "definition": {"fields": [
                {"name": "title", "type": "text", "required": true},
                {"name": "body", "type": "textarea", "required": false}
            ]}
        }),
    )
    .await;
    assert!(!mcp_is_error(&result), "update_collection should succeed");
}

#[tokio::test]
async fn test_delete_collection() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    create_test_collection(&server.base_url, &token, "Posts", "posts").await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "delete_collection",
        serde_json::json!({"slug": "posts"}),
    )
    .await;
    let data = mcp_tool_json(&result);
    assert!(data["deleted"].as_bool().unwrap());

    let list = mcp_call_tool(&server.base_url, &token, "list_collections", serde_json::json!({})).await;
    let list_data = mcp_tool_json(&list);
    assert!(list_data.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_delete_collection_not_found() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "delete_collection",
        serde_json::json!({"slug": "nonexistent"}),
    )
    .await;
    assert!(
        mcp_is_error(&result),
        "Should return isError for non-existent collection"
    );
}

#[tokio::test]
async fn test_create_collection_requires_admin() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_read_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_collection",
        serde_json::json!({
            "name": "Posts",
            "definition": {"fields": []},
        }),
    )
    .await;
    assert!(mcp_is_error(&result), "Viewer should not create collection");
}

#[tokio::test]
async fn test_collection_full_lifecycle() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = create_test_collection(&server.base_url, &token, "Posts", "posts").await;
    let col = mcp_tool_json(&result);
    let col_id = col["id"].as_str().unwrap();
    assert!(!col_id.is_empty());

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_collection",
        serde_json::json!({"slug": "posts"}),
    )
    .await;
    let fetched = mcp_tool_json(&result);
    assert_eq!(fetched["id"].as_str().unwrap(), col_id);

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_collection",
        serde_json::json!({"slug": "posts", "name": "Articles"}),
    )
    .await;
    let updated = mcp_tool_json(&result);
    assert_eq!(updated["name"].as_str().unwrap(), "Articles");

    let result = mcp_call_tool(&server.base_url, &token, "list_collections", serde_json::json!({})).await;
    let list = mcp_tool_json(&result);
    assert_eq!(list.as_array().unwrap().len(), 1);

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "delete_collection",
        serde_json::json!({"slug": "posts"}),
    )
    .await;
    let data = mcp_tool_json(&result);
    assert!(data["deleted"].as_bool().unwrap());

    let result = mcp_call_tool(&server.base_url, &token, "list_collections", serde_json::json!({})).await;
    let list = mcp_tool_json(&result);
    assert!(list.as_array().unwrap().is_empty());
}
