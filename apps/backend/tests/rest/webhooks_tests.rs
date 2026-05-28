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

#[tokio::test]
async fn test_create_webhook() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "label": "Test Hook",
            "url": "https://example.com/hook",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["label"], "Test Hook");
}

#[tokio::test]
async fn test_list_webhooks() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"label": "Hook 1", "url": "https://example.com/h1"}))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.is_array());
    assert!(!body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_get_webhook() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let create_resp = client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"label": "My Hook", "url": "https://example.com/hook"}))
        .send()
        .await
        .unwrap();
    let created: Value = create_resp.json().await.unwrap();
    let hook_id = created["id"].as_str().unwrap();

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/webhooks/{}", server.base_url, site_id, hook_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_delete_webhook() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let create_resp = client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"label": "Delete Me", "url": "https://example.com/del"}))
        .send()
        .await
        .unwrap();
    let created: Value = create_resp.json().await.unwrap();
    let hook_id = created["id"].as_str().unwrap();

    let resp = client
        .delete(format!("{}/api/dashboard/sites/{}/webhooks/{}", server.base_url, site_id, hook_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_list_deliveries_empty() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let create_resp = client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"label": "Deliveries", "url": "https://example.com/d"}))
        .send()
        .await
        .unwrap();
    let created: Value = create_resp.json().await.unwrap();
    let hook_id = created["id"].as_str().unwrap();

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/webhooks/{}/deliveries", server.base_url, site_id, hook_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body["items"].is_array());
}
