use crate::common::mcp::*;

#[tokio::test]
async fn entry_workflow_uses_modern_tools_and_structured_output() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;
    let collection = create_test_collection(&server.base_url, &token, &site_id, "Posts", "posts").await;
    let collection_id = mcp_tool_json(&collection)["id"].as_str().unwrap().to_owned();
    let created = create_test_entry(
        &server.base_url,
        &token,
        &site_id,
        &collection_id,
        "hello",
        serde_json::json!({"title": "Hello"}),
    )
    .await;
    let entry = mcp_tool_json(&created);
    let entry_id = entry["id"].as_str().unwrap();

    let fetched = mcp_call_site_tool(
        &server.base_url,
        &token,
        &site_id,
        "get_entry",
        serde_json::json!({"id": entry_id, "include_drafts": true}),
    )
    .await;
    assert!(!mcp_is_error(&fetched));
    assert_eq!(mcp_tool_json(&fetched)["slug"], "hello");

    let listed = mcp_call_site_tool(
        &server.base_url,
        &token,
        &site_id,
        "list_entries",
        serde_json::json!({"published_only": false}),
    )
    .await;
    assert_eq!(mcp_tool_json(&listed)["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn site_discovery_and_schema_resources_are_available() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;
    let sites = mcp_tool_json(&mcp_call_tool(&server.base_url, &token, "list_sites", serde_json::json!({})).await);
    assert_eq!(sites[0]["id"], site_id);
    let schema = mcp_read_resource(&server.base_url, &token, &format!("cms://{site_id}/schema")).await;
    assert!(schema["contents"].is_array());
}

#[tokio::test]
async fn update_publication_requires_separate_scope() {
    let server = start_mcp_server().await;
    let (site_id, token) = setup_site_token(&server).await;
    let collection = create_test_collection(&server.base_url, &token, &site_id, "Posts", "posts").await;
    let collection = mcp_tool_json(&collection);
    let created = create_test_entry(
        &server.base_url,
        &token,
        &site_id,
        collection["id"].as_str().unwrap(),
        "draft",
        serde_json::json!({"title":"Draft"}),
    )
    .await;
    let entry = mcp_tool_json(&created);
    let http = crate::common::client::http_client();
    let response = server.login_user(&http, "admin@cms.local", "admin").await;
    let (session, csrf) = crate::common::auth::extract_cookies(&response);
    let response = http
        .post(format!("{}/api/dashboard/sites/{site_id}/tokens", server.base_url))
        .headers(crate::common::auth::auth_header(&session, &csrf))
        .json(&serde_json::json!({"name":"writer","scopes":["content.write","mcp.use"]}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let credential: serde_json::Value = response.json().await.unwrap();
    let denied = mcp_call_site_tool(
        &server.base_url,
        credential["token"].as_str().unwrap(),
        &site_id,
        "update_entry",
        serde_json::json!({"id":entry["id"],"published":true}),
    )
    .await;
    assert!(mcp_is_error(&denied));
    assert_eq!(mcp_tool_json(&denied)["error"]["code"], "forbidden");
    let fetched = mcp_call_site_tool(
        &server.base_url,
        &token,
        &site_id,
        "get_entry",
        serde_json::json!({"id":entry["id"],"include_drafts":true}),
    )
    .await;
    assert_eq!(mcp_tool_json(&fetched)["status"], "draft");
}
