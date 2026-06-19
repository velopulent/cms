use serde_json::{Value, json};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::{TestServer, auth::auth_header, fixtures::setup};

#[tokio::test]
async fn test_create_webhook() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"label": "Hook 1", "url": "https://example.com/h1"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        201,
        "create webhook failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let create_resp = client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"label": "My Hook", "url": "https://example.com/hook"}))
        .send()
        .await
        .unwrap();
    let created: Value = create_resp.json().await.unwrap();
    let hook_id = created["id"].as_str().unwrap();

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/webhooks/{}",
            server.base_url, site_id, hook_id
        ))
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_delete_webhook() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let create_resp = client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"label": "Delete Me", "url": "https://example.com/del"}))
        .send()
        .await
        .unwrap();
    let created: Value = create_resp.json().await.unwrap();
    let hook_id = created["id"].as_str().unwrap();

    let resp = client
        .delete(format!(
            "{}/api/dashboard/sites/{}/webhooks/{}",
            server.base_url, site_id, hook_id
        ))
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_list_deliveries_empty() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let create_resp = client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"label": "Deliveries", "url": "https://example.com/d"}))
        .send()
        .await
        .unwrap();
    let created: Value = create_resp.json().await.unwrap();
    let hook_id = created["id"].as_str().unwrap();

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/webhooks/{}/deliveries",
            server.base_url, site_id, hook_id
        ))
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body["items"].is_array());
}

#[tokio::test]
async fn test_trigger_webhook() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let create_resp = client
        .post(format!("{}/api/dashboard/sites/{}/webhooks", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"label": "Trigger Me", "url": mock_server.uri()}))
        .send()
        .await
        .unwrap();
    let created: Value = create_resp.json().await.unwrap();
    let hook_id = created["id"].as_str().unwrap();

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/webhooks/{}/trigger",
            server.base_url, site_id, hook_id
        ))
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "success");
}
