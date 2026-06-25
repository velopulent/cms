use serde_json::{Value, json};

use crate::common::{TestServer, auth::auth_header, fixtures::setup};

fn api_key_header(api_key: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Bearer {}", api_key)).unwrap(),
    );
    headers
}

async fn get_api_key(server: &TestServer, token: &str, csrf: &str, site_id: &str) -> String {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_id))
        .headers(auth_header(token, csrf))
        .json(&json!({"name": "File Token", "permission": "write"}))
        .send()
        .await
        .unwrap();
    let val: Value = resp.json().await.unwrap();
    val["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_upload_file() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let file_content = b"Hello, this is a test file!";
    let part = reqwest::multipart::Part::bytes(file_content.to_vec())
        .file_name("test.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let resp = client
        .post(format!("{}/files", server.base_url))
        .headers(api_key_header(&api_key))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: Value = resp.json().await.unwrap();
    assert!(body["id"].is_string());
    assert_eq!(body["original_name"], "test.txt");
    assert_eq!(body["mime_type"], "text/plain");
}

#[tokio::test]
async fn test_list_files() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let part = reqwest::multipart::Part::bytes(b"file content".to_vec())
        .file_name("list-test.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let resp = client
        .post(format!("{}/files", server.base_url))
        .headers(api_key_header(&api_key))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "upload file failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    let resp = client
        .get(format!("{}/files", server.base_url))
        .headers(api_key_header(&api_key))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body["items"].is_array());
    assert!(body["total"].as_i64().unwrap() >= 1);
}

#[tokio::test]
async fn test_get_file() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let part = reqwest::multipart::Part::bytes(b"content".to_vec())
        .file_name("get-test.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let upload_resp = client
        .post(format!("{}/files", server.base_url))
        .headers(api_key_header(&api_key))
        .multipart(form)
        .send()
        .await
        .unwrap();
    let uploaded: Value = upload_resp.json().await.unwrap();
    let file_id = uploaded["id"].as_str().unwrap();

    let resp = client
        .get(format!("{}/files/{}", server.base_url, file_id))
        .headers(api_key_header(&api_key))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["id"], file_id);
}

#[tokio::test]
async fn test_delete_file() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let part = reqwest::multipart::Part::bytes(b"delete me".to_vec())
        .file_name("del-test.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let upload_resp = client
        .post(format!("{}/files", server.base_url))
        .headers(api_key_header(&api_key))
        .multipart(form)
        .send()
        .await
        .unwrap();
    let uploaded: Value = upload_resp.json().await.unwrap();
    let file_id = uploaded["id"].as_str().unwrap();

    let resp = client
        .delete(format!("{}/files/{}", server.base_url, file_id))
        .headers(api_key_header(&api_key))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_get_file_references() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let part = reqwest::multipart::Part::bytes(b"ref content".to_vec())
        .file_name("ref-test.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let upload_resp = client
        .post(format!("{}/files", server.base_url))
        .headers(api_key_header(&api_key))
        .multipart(form)
        .send()
        .await
        .unwrap();
    let uploaded: Value = upload_resp.json().await.unwrap();
    let file_id = uploaded["id"].as_str().unwrap();

    let resp = client
        .get(format!("{}/files/{}/references", server.base_url, file_id))
        .headers(api_key_header(&api_key))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_batch_delete_files() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let mut ids = Vec::new();
    for i in 0..3 {
        let part = reqwest::multipart::Part::bytes(format!("batch content {}", i).as_bytes().to_vec())
            .file_name(format!("batch-{}.txt", i))
            .mime_str("text/plain")
            .unwrap();
        let form = reqwest::multipart::Form::new().part("file", part);

        let resp = client
            .post(format!("{}/files", server.base_url))
            .headers(api_key_header(&api_key))
            .multipart(form)
            .send()
            .await
            .unwrap();
        let val: Value = resp.json().await.unwrap();
        ids.push(val["id"].as_str().unwrap().to_string());
    }

    let resp = client
        .post(format!("{}/files/batch-delete", server.base_url))
        .headers(api_key_header(&api_key))
        .json(&json!({"ids": ids}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["deleted"], 3);
}

/// Regression: a file larger than the global 10MB body limit but within the
/// configured upload cap (50MB in tests) must upload successfully. Before the
/// fix, the global `RequestBodyLimitLayer(10MB)` capped every route regardless
/// of the per-route override, so this returned 413 / aborted the connection.
#[tokio::test]
async fn test_upload_file_above_global_body_limit() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    // 12MB > the 10MB global limit, < the 50MB configured upload cap.
    let file_content = vec![b'a'; 12 * 1024 * 1024];
    let part = reqwest::multipart::Part::bytes(file_content)
        .file_name("big.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let resp = client
        .post(format!("{}/files", server.base_url))
        .headers(api_key_header(&api_key))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        201,
        "12MB upload should succeed under the 50MB cap: {}",
        resp.text().await.unwrap_or_default()
    );
}

/// A file larger than the configured upload cap is rejected. The body-limit
/// layer may reset the connection before the response is read (the documented
/// tradeoff), so a transport error is an acceptable rejection too — what must
/// never happen is a successful (2xx) upload.
#[tokio::test]
async fn test_upload_file_exceeds_max_size() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    // 52MB > the 50MB configured upload cap.
    let file_content = vec![b'a'; 52 * 1024 * 1024];
    let part = reqwest::multipart::Part::bytes(file_content)
        .file_name("toobig.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let result = client
        .post(format!("{}/files", server.base_url))
        .headers(api_key_header(&api_key))
        .multipart(form)
        .send()
        .await;

    match result {
        Ok(resp) => assert_eq!(
            resp.status(),
            413,
            "over-cap upload should be rejected with 413: {}",
            resp.text().await.unwrap_or_default()
        ),
        // The only acceptable transport failure is the body-limit layer aborting
        // the connection mid-upload (the request body was already being streamed).
        // A connect/timeout/builder error means the request never reached that
        // path, so it points at an unrelated failure and must fail the test.
        Err(e) => assert!(
            !e.is_connect() && !e.is_timeout() && !e.is_builder(),
            "over-cap upload failed with an unexpected transport error (not a body-limit abort): {e}"
        ),
    }
}

#[tokio::test]
async fn test_upload_file_invalid_mime_type() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let part = reqwest::multipart::Part::bytes(b"executable content".to_vec())
        .file_name("malware.exe")
        .mime_str("application/x-executable")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let resp = client
        .post(format!("{}/files", server.base_url))
        .headers(api_key_header(&api_key))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    let error = body["error"].as_str().unwrap_or("");
    assert!(
        error.contains("Content type") || error.contains("content type") || error.contains("Invalid"),
        "Expected MIME error, got: {}",
        error
    );
}
