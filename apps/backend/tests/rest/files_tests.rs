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

// ── Signed-URL uploads ──────────────────────────────────────────────────────
//
// Tokens are minted directly with the lib (same HMAC secret as TestServer), so
// these tests cover the PUT endpoint without an MCP round-trip.

const TEST_HMAC_SECRET: &str = "test-signed-upload-key";

fn mint_upload_url(server: &TestServer, site_id: &str, filename: &str, mime: &str, expiry_secs: i64) -> String {
    let (_, encoded) = cms::signed_upload::SignedUploadToken::generate_with_storage_provider(
        site_id,
        filename,
        mime,
        "filesystem",
        TEST_HMAC_SECRET,
        expiry_secs,
    );
    format!("{}/api/v1/files/upload/{}", server.base_url, encoded)
}

#[tokio::test]
async fn test_signed_upload_happy_path() {
    let server = TestServer::start().await;
    let (_token, _csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let url = mint_upload_url(&server, &site_id, "signed.txt", "text/plain", 900);
    let resp = client
        .put(&url)
        .header(reqwest::header::CONTENT_TYPE, "text/plain")
        .body("signed upload body")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201, "{}", resp.text().await.unwrap_or_default());
    let body: Value = resp.json().await.unwrap();
    let file_id = body["id"].as_str().unwrap();
    assert_eq!(body["original_name"], "signed.txt");
    assert_eq!(body["mime_type"], "text/plain");
    assert_eq!(body["size"], 18);

    // The stored bytes are servable through the public file route.
    let served = client
        .get(format!("{}/api/files/{}", server.base_url, file_id))
        .send()
        .await
        .unwrap();
    assert_eq!(served.status(), 200);
    assert_eq!(served.text().await.unwrap(), "signed upload body");
}

#[tokio::test]
async fn test_signed_upload_expired_token() {
    let server = TestServer::start().await;
    let (_token, _csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let url = mint_upload_url(&server, &site_id, "late.txt", "text/plain", -10);
    let resp = client.put(&url).body("too late").send().await.unwrap();
    assert_eq!(resp.status(), 410);
}

#[tokio::test]
async fn test_signed_upload_tampered_signature() {
    let server = TestServer::start().await;
    let (_token, _csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    // Minted with a different secret => signature check must fail.
    let (_, encoded) = cms::signed_upload::SignedUploadToken::generate_with_storage_provider(
        &site_id,
        "evil.txt",
        "text/plain",
        "filesystem",
        "not-the-server-secret",
        900,
    );
    let url = format!("{}/api/v1/files/upload/{}", server.base_url, encoded);
    let resp = client.put(&url).body("nope").send().await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_signed_upload_single_use() {
    let server = TestServer::start().await;
    let (_token, _csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let url = mint_upload_url(&server, &site_id, "once.txt", "text/plain", 900);
    let first = client.put(&url).body("first").send().await.unwrap();
    assert_eq!(first.status(), 201);

    let second = client.put(&url).body("second").send().await.unwrap();
    assert_eq!(second.status(), 409, "reused upload URL must be rejected");
}

#[tokio::test]
async fn test_signed_upload_magic_byte_mismatch() {
    let server = TestServer::start().await;
    let (_token, _csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    // Token says PNG; body is plain text => sniffed mismatch => 400.
    let url = mint_upload_url(&server, &site_id, "fake.png", "image/png", 900);
    let resp = client
        .put(&url)
        .header(reqwest::header::CONTENT_TYPE, "image/png")
        .body("definitely not a png")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_signed_upload_content_type_header_mismatch() {
    let server = TestServer::start().await;
    let (_token, _csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let url = mint_upload_url(&server, &site_id, "a.txt", "text/plain", 900);
    let resp = client
        .put(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/pdf")
        .body("hello")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_signed_upload_oversize_rejected() {
    let server = TestServer::start().await;
    let (_token, _csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let url = mint_upload_url(&server, &site_id, "big.txt", "text/plain", 900);
    // 52MB > the 50MB cap; the Content-Length fast path rejects upfront.
    let body = vec![b'a'; 52 * 1024 * 1024];
    let result = client.put(&url).body(body).send().await;
    match result {
        Ok(resp) => assert_eq!(resp.status(), 413, "{}", resp.text().await.unwrap_or_default()),
        Err(e) => assert!(
            !e.is_connect() && !e.is_timeout() && !e.is_builder(),
            "unexpected transport error: {e}"
        ),
    }
}

#[tokio::test]
async fn test_signed_chunked_upload_uses_live_instance_limit() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::new();
    let update = client
        .put(format!("{}/api/dashboard/instance/settings/general", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({
            "public_url": null,
            "public_registration": false,
            "session_lifetime_hours": 24,
            "upload_limit_mb": 1,
            "mcp_enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), 200);

    let url = mint_upload_url(&server, &site_id, "chunked.txt", "text/plain", 900);
    let chunks = futures_util::stream::iter([
        Ok::<_, std::io::Error>(bytes::Bytes::from(vec![b'a'; 700_000])),
        Ok(bytes::Bytes::from(vec![b'b'; 700_000])),
    ]);
    let response = client
        .put(url)
        .header("Content-Type", "text/plain")
        .body(reqwest::Body::wrap_stream(chunks))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
}

/// Multipart path enforces the same sniffing as the signed path.
#[tokio::test]
async fn test_multipart_upload_magic_byte_mismatch() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let part = reqwest::multipart::Part::bytes(b"just some text".to_vec())
        .file_name("fake.png")
        .mime_str("image/png")
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
}
