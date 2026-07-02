use crate::common::mcp::*;

#[tokio::test]
async fn test_list_files_empty() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(&server.base_url, &token, "list_files", serde_json::json!({})).await;
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

/// End-to-end: mint an upload URL via MCP, PUT the bytes to it, and verify the
/// file exists (both via the MCP get_file tool and the returned record).
#[tokio::test]
async fn test_create_upload_url_then_put_uploads_file() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_upload_url",
        serde_json::json!({
            "filename": "e2e.txt",
            "content_type": "text/plain",
        }),
    )
    .await;
    let data = mcp_tool_json(&result);
    let upload_url = data["upload_url"].as_str().unwrap();
    let file_id = data["file_id"].as_str().unwrap();

    // The URL derives its host from the request's Host header, so it points at
    // this test server and is directly PUT-able.
    let client = reqwest::Client::new();
    let resp = client
        .put(upload_url)
        .header(reqwest::header::CONTENT_TYPE, "text/plain")
        .body("mcp e2e upload")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "{}", resp.text().await.unwrap_or_default());
    let created: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(created["id"].as_str().unwrap(), file_id);

    // Reuse must be rejected (single-use).
    let again = client.put(upload_url).body("again").send().await.unwrap();
    assert_eq!(again.status(), 409);

    // The file is visible through MCP.
    let got = mcp_call_tool(
        &server.base_url,
        &token,
        "get_file",
        serde_json::json!({"file_id": file_id}),
    )
    .await;
    let got_data = mcp_tool_json(&got);
    assert_eq!(got_data["original_name"].as_str().unwrap(), "e2e.txt");
}

#[tokio::test]
async fn test_create_upload_url_requires_editor() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_read_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_upload_url",
        serde_json::json!({"filename": "x.txt", "content_type": "text/plain"}),
    )
    .await;
    assert!(mcp_is_error(&result), "read-only token must not mint upload URLs");
}

#[tokio::test]
async fn test_create_upload_url_rejects_disallowed_content_type() {
    let server = start_mcp_server().await;
    let (_, token) = setup_site_token(&server).await;

    let result = mcp_call_tool(
        &server.base_url,
        &token,
        "create_upload_url",
        serde_json::json!({"filename": "x.exe", "content_type": "application/x-executable"}),
    )
    .await;
    assert!(mcp_is_error(&result), "disallowed content type must fail at mint time");
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
