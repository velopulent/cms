use crate::mcp_helpers::*;

#[tokio::test]
async fn test_list_files_empty() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result =
        mcp_call_tool(&server.base_url, &token, "list_files", serde_json::json!({})).await;
    let data = mcp_tool_json(&result);

    assert!(data["items"].as_array().unwrap().is_empty());
    assert_eq!(data["total"].as_i64().unwrap(), 0);
}

#[tokio::test]
async fn test_get_file_not_found() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "get_file",
        serde_json::json!({"file_id": "nonexistent"}),
    )
    .await;
    assert!(mcp_is_error(&result), "Should return isError for non-existent file");
}

#[tokio::test]
async fn test_create_upload_url() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_upload_url",
        serde_json::json!({
            "filename": "test.txt",
            "content_type": "text/plain",
        }),
    )
    .await;
    let data = mcp_tool_json(&result);

    assert!(
        data["upload_url"].as_str().unwrap().contains("upload"),
        "upload_url should be an upload endpoint"
    );
    assert!(!data["file_id"].as_str().unwrap().is_empty());
    assert_eq!(data["method"].as_str().unwrap(), "PUT");
    assert_eq!(data["content_type"].as_str().unwrap(), "text/plain");
}

#[tokio::test]
async fn test_list_files_with_pagination() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "list_files",
        serde_json::json!({"page": 1, "per_page": 10}),
    )
    .await;
    let data = mcp_tool_json(&result);
    assert!(data["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_delete_file_requires_editor() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_read_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "delete_file",
        serde_json::json!({"file_id": "any"}),
    )
    .await;
    assert!(mcp_is_error(&result), "Viewer should not delete file");
}
