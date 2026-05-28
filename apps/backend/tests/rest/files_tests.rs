use serde_json::{json, Value};

use crate::common::TestServer;

async fn setup(server: &TestServer) -> (String, String, String) {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = server.login_user(&client, "admin", "admin").await;
    let headers = resp.headers();
    let mut jwt = String::new();
    let mut csrf = String::new();
    for cookie in headers.get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if val.starts_with("token=") {
                jwt = val.split(';').next().and_then(|c| c.strip_prefix("token=")).unwrap_or("").to_string();
            }
            if val.starts_with("csrf=") {
                csrf = val.split(';').next().and_then(|c| c.strip_prefix("csrf=")).unwrap_or("").to_string();
            }
        }
    }
    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"name": "Test Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap().to_string();
    (jwt, csrf, site_id)
}

fn auth_header(jwt: &str, csrf: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    let cookie_val = format!("token={}; csrf={}", jwt, csrf);
    headers.insert(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_str(&cookie_val).unwrap());
    headers.insert("X-CSRF-Token", reqwest::header::HeaderValue::from_str(csrf).unwrap());
    headers
}

fn api_key_header(api_key: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, reqwest::header::HeaderValue::from_str(&format!("Bearer {}", api_key)).unwrap());
    headers
}

async fn get_api_key(server: &TestServer, jwt: &str, csrf: &str, site_id: &str) -> String {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_id))
        .headers(auth_header(jwt, csrf))
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
    let (jwt, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &jwt, &csrf, &site_id).await;
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
    let (jwt, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &jwt, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let part = reqwest::multipart::Part::bytes(b"file content".to_vec())
        .file_name("list-test.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    client
        .post(format!("{}/files", server.base_url))
        .headers(api_key_header(&api_key))
        .multipart(form)
        .send()
        .await
        .unwrap();

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
    let (jwt, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &jwt, &csrf, &site_id).await;
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
    let (jwt, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &jwt, &csrf, &site_id).await;
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
    let (jwt, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &jwt, &csrf, &site_id).await;
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
    let (jwt, csrf, site_id) = setup(&server).await;
    let api_key = get_api_key(&server, &jwt, &csrf, &site_id).await;
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
