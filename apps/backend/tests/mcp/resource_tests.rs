use crate::common::mcp::*;

#[tokio::test]
async fn test_list_resources() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let resources = mcp_list_resources(&server.base_url, &token).await;

    assert!(!resources.is_empty(), "Should have at least the schema resource");
    let uris: Vec<&str> = resources.iter().filter_map(|r| r["uri"].as_str()).collect();
    assert!(
        uris.iter().any(|u| u.ends_with("/schema")),
        "Should have schema resource"
    );
}

#[tokio::test]
async fn test_read_schema_resource() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;

    let result = mcp_read_resource(&server.base_url, &token, &format!("cms://{}/schema", site_id)).await;

    let contents = result["contents"].as_array().expect("missing contents array");
    assert!(!contents.is_empty());

    let text = contents[0]["text"].as_str().expect("missing text");
    let schema: serde_json::Value = serde_json::from_str(text).expect("invalid JSON in schema");

    assert!(schema.get("site").is_some());
    assert!(schema.get("collections").is_some());
    assert!(schema.get("field_types").is_some());
}

#[tokio::test]
async fn test_read_collection_resource() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;

    create_test_collection(&server.base_url, &token, "Posts", "posts").await;

    let result = mcp_read_resource(
        &server.base_url,
        &token,
        &format!("cms://{}/collections/posts", site_id),
    )
    .await;

    let contents = result["contents"].as_array().expect("missing contents array");
    let text = contents[0]["text"].as_str().expect("missing text");
    let data: serde_json::Value = serde_json::from_str(text).expect("invalid JSON");

    assert_eq!(data["name"].as_str().unwrap(), "Posts");
    assert_eq!(data["slug"].as_str().unwrap(), "posts");
}

#[tokio::test]
async fn test_read_resource_invalid_uri() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let resp = mcp_request(
        &server.base_url,
        &token,
        "resources/read",
        Some(serde_json::json!({"uri": "invalid://uri"})),
    )
    .await;

    assert!(resp.get("error").is_some(), "Should error for invalid URI");
}

#[tokio::test]
async fn test_read_resource_not_found() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;

    let resp = mcp_request(
        &server.base_url,
        &token,
        "resources/read",
        Some(serde_json::json!({
            "uri": format!("cms://{}/collections/nonexistent", site_id)
        })),
    )
    .await;

    assert!(
        resp.get("error").is_some(),
        "Should error for non-existent collection resource"
    );
}

#[tokio::test]
async fn test_resources_require_auth() {
    let server = start_mcp_server().await;

    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/mcp", server.base_url))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "resources/list"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_resources_reflect_created_collections() {
    let server = start_mcp_server().await;
    let (_site_id, token) = setup_site_token(&server).await;

    create_test_collection(&server.base_url, &token, "Posts", "posts").await;
    create_test_collection(&server.base_url, &token, "Pages", "pages").await;

    let resources = mcp_list_resources(&server.base_url, &token).await;

    let uris: Vec<&str> = resources.iter().filter_map(|r| r["uri"].as_str()).collect();
    assert!(
        uris.iter().any(|u| u.contains("/collections/posts")),
        "Should list posts collection resource"
    );
    assert!(
        uris.iter().any(|u| u.contains("/collections/pages")),
        "Should list pages collection resource"
    );
}
