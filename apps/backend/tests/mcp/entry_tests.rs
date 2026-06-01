use crate::mcp_helpers::*;

async fn setup_collection(base_url: &str, token: &str) -> String {
    let result = create_test_collection(base_url, token, "Posts", "posts").await;
    let col = mcp_tool_json(&result);
    col["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_list_entries_empty() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result =
        mcp_call_tool(&server.base_url, &token, "list_entries", serde_json::json!({})).await;
    let data = mcp_tool_json(&result);

    assert!(data["items"].as_array().unwrap().is_empty());
    assert_eq!(data["total"].as_i64().unwrap(), 0);
}

#[tokio::test]
async fn test_create_entry() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let col_id = setup_collection(&server.base_url, &token).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_entry",
        serde_json::json!({
            "collection_id": col_id,
            "slug": "hello-world",
            "values": {"title": "Hello World"},
        }),
    )
    .await;
    let entry = mcp_tool_json(&result);

    assert_eq!(entry["slug"].as_str().unwrap(), "hello-world");
    assert!(!entry["id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_get_entry() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let col_id = setup_collection(&server.base_url, &token).await;

    let result = create_test_entry(
        &server.base_url,
        &token,
        &col_id,
        "hello-world",
        serde_json::json!({"title": "Hello World"}),
    )
    .await;
    let entry = mcp_tool_json(&result);
    let entry_id = entry["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "publish_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let wrapped = serde_json::json!({"result": result});
    assert!(!mcp_is_error(&wrapped), "publish_entry should succeed");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let entry = mcp_tool_json(&result);
    assert_eq!(entry["slug"].as_str().unwrap(), "hello-world");
}

#[tokio::test]
async fn test_get_entry_not_found() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_entry",
        serde_json::json!({"id": "nonexistent-id"}),
    )
    .await;
    assert!(mcp_is_error(&result), "Should return isError for non-existent entry");
}

#[tokio::test]
async fn test_update_entry_values() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let col_id = setup_collection(&server.base_url, &token).await;

    let result = create_test_entry(
        &server.base_url,
        &token,
        &col_id,
        "hello-world",
        serde_json::json!({"title": "Hello"}),
    )
    .await;
    let entry_id = mcp_tool_json(&result)["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_entry",
        serde_json::json!({
            "id": entry_id,
            "values": {"title": "Hello World Updated"},
        }),
    )
    .await;
    assert!(!mcp_is_error(&result), "update_entry should succeed");
}

#[tokio::test]
async fn test_delete_entry() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let col_id = setup_collection(&server.base_url, &token).await;

    let result = create_test_entry(
        &server.base_url,
        &token,
        &col_id,
        "hello-world",
        serde_json::json!({"title": "Hello"}),
    )
    .await;
    let entry_id = mcp_tool_json(&result)["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "delete_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let data = mcp_tool_json(&result);
    assert!(data["deleted"].as_bool().unwrap());
}

#[tokio::test]
async fn test_publish_entry() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let col_id = setup_collection(&server.base_url, &token).await;

    let result = create_test_entry(
        &server.base_url,
        &token,
        &col_id,
        "hello-world",
        serde_json::json!({"title": "Hello"}),
    )
    .await;
    let entry_id = mcp_tool_json(&result)["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "publish_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let entry = mcp_tool_json(&result);
    assert_eq!(entry["status"].as_str().unwrap(), "published");
}

#[tokio::test]
async fn test_unpublish_entry() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let col_id = setup_collection(&server.base_url, &token).await;

    let result = create_test_entry(
        &server.base_url,
        &token,
        &col_id,
        "hello-world",
        serde_json::json!({"title": "Hello"}),
    )
    .await;
    let entry_id = mcp_tool_json(&result)["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "publish_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let wrapped = serde_json::json!({"result": result});
    assert!(!mcp_is_error(&wrapped), "publish_entry should succeed");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "unpublish_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let entry = mcp_tool_json(&result);
    assert_eq!(entry["status"].as_str().unwrap(), "draft");
}

#[tokio::test]
async fn test_list_revisions() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let col_id = setup_collection(&server.base_url, &token).await;

    let result = create_test_entry(
        &server.base_url,
        &token,
        &col_id,
        "hello-world",
        serde_json::json!({"title": "V1"}),
    )
    .await;
    let entry_id = mcp_tool_json(&result)["id"].as_str().unwrap().to_string();

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_entry",
        serde_json::json!({"id": entry_id, "values": {"title": "V2"}}),
    )
    .await;
    let wrapped = serde_json::json!({"result": result});
    assert!(!mcp_is_error(&wrapped), "update_entry should succeed");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "list_revisions",
        serde_json::json!({"entry_id": entry_id}),
    )
    .await;
    let data = mcp_tool_json(&result);
    assert!(
        data["total"].as_i64().unwrap() >= 2,
        "Expected at least 2 revisions"
    );
}

#[tokio::test]
async fn test_create_entry_requires_editor() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_read_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_entry",
        serde_json::json!({
            "collection_id": "any",
            "values": {"title": "Test"},
        }),
    )
    .await;
    assert!(mcp_is_error(&result), "Viewer should not create entry");
}

#[tokio::test]
async fn test_entry_full_lifecycle() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let col_id = setup_collection(&server.base_url, &token).await;

    let result = create_test_entry(
        &server.base_url,
        &token,
        &col_id,
        "my-post",
        serde_json::json!({"title": "My Post"}),
    )
    .await;
    let entry = mcp_tool_json(&result);
    let entry_id = entry["id"].as_str().unwrap().to_string();
    assert_eq!(entry["slug"].as_str().unwrap(), "my-post");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "publish_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let wrapped = serde_json::json!({"result": result});
    assert!(!mcp_is_error(&wrapped), "publish_entry should succeed");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let fetched = mcp_tool_json(&result);
    assert_eq!(fetched["slug"].as_str().unwrap(), "my-post");
    assert_eq!(fetched["status"].as_str().unwrap(), "published");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "update_entry",
        serde_json::json!({"id": entry_id, "values": {"title": "Updated Post"}}),
    )
    .await;
    let wrapped = serde_json::json!({"result": result});
    assert!(!mcp_is_error(&wrapped), "update_entry should succeed");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "unpublish_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let wrapped = serde_json::json!({"result": result});
    assert!(!mcp_is_error(&wrapped), "unpublish_entry should succeed");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "delete_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let wrapped = serde_json::json!({"result": result});
    assert!(!mcp_is_error(&wrapped), "delete_entry should succeed");
}

#[tokio::test]
async fn test_list_entries_filter_by_collection() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let r1 = create_test_collection(&server.base_url, &token, "Posts", "posts").await;
    let col1_id = mcp_tool_json(&r1)["id"].as_str().unwrap().to_string();

    let r2 = create_test_collection(&server.base_url, &token, "Pages", "pages").await;
    let col2_id = mcp_tool_json(&r2)["id"].as_str().unwrap().to_string();

    create_test_entry(
        &server.base_url,
        &token,
        &col1_id,
        "post-1",
        serde_json::json!({"title": "Post 1"}),
    )
    .await;
    create_test_entry(
        &server.base_url,
        &token,
        &col2_id,
        "page-1",
        serde_json::json!({"title": "Page 1"}),
    )
    .await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "list_entries",
        serde_json::json!({"collection_slug": "posts", "published_only": false}),
    )
    .await;
    let data = mcp_tool_json(&result);
    assert_eq!(data["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_list_entries_with_search() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;
    let col_id = setup_collection(&server.base_url, &token).await;

    let created = create_test_entry(
        &server.base_url,
        &token,
        &col_id,
        "searchable",
        serde_json::json!({"title": "Unique Title"}),
    )
    .await;
    let entry = mcp_tool_json(&created);
    let entry_id = entry["id"].as_str().unwrap().to_string();

    let publish_result = mcp_call_tool(
        &server.base_url,
        &token,
        "publish_entry",
        serde_json::json!({"id": entry_id}),
    )
    .await;
    let wrapped = serde_json::json!({"result": publish_result});
    assert!(!mcp_is_error(&wrapped), "publish_entry should succeed");

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "list_entries",
        serde_json::json!({"search": "Unique", "published_only": false}),
    )
    .await;
    let data = mcp_tool_json(&result);
    let items = data["items"].as_array().expect("items array");
    let slugs: Vec<&str> = items.iter().filter_map(|i| i["slug"].as_str()).collect();
    assert!(slugs.contains(&"searchable"), "expected slug 'searchable' in search results, got: {:?}", slugs);
}
