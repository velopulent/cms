use serde_json::{json, Value};

use crate::common::TestServer;

async fn setup(server: &TestServer) -> (String, String) {
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
        .header("Cookie", format!("token={}; csrf={}", jwt, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Webhook Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap().to_string();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_id))
        .header("Cookie", format!("token={}; csrf={}", jwt, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Token", "permission": "write"}))
        .send()
        .await
        .unwrap();
    let token_val: Value = resp.json().await.unwrap();
    let token = token_val["token"].as_str().unwrap().to_string();

    (site_id, token)
}

async fn gql(server: &TestServer, token: &str, query: &str) -> Value {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/api/graphql", server.base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"query": query}))
        .send()
        .await
        .unwrap();
    resp.json().await.unwrap()
}

#[tokio::test]
async fn test_webhooks_query() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let query = format!(r#"{{ webhooks(siteId: "{}") {{ id label url }} }}"#, site_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null(), "errors: {:?}", body["errors"]);
    let hooks = body["data"]["webhooks"].as_array().unwrap();
    assert!(hooks.is_empty());
}

#[tokio::test]
async fn test_webhook_not_found() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let query = format!(r#"{{ webhook(siteId: "{}", webhookId: "nonexistent") {{ id }} }}"#, site_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(msg.contains("not found"));
}

#[tokio::test]
async fn test_create_webhook_foreign_key_bug() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let query = r#"mutation CreateWebhook($siteId: String!, $label: String!, $url: String!) {
        createWebhook(siteId: $siteId, label: $label, url: $url) { id label url }
    }"#;
    let vars = json!({"siteId": site_id, "label": "Hook", "url": "https://example.com/hook"});
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/api/graphql", server.base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"query": query, "variables": vars}))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();

    assert!(body["errors"].is_array(), "expected FK error, got: {:?}", body);
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(
        msg.contains("FOREIGN KEY") || msg.contains("Database error"),
        "unexpected error: {}",
        msg
    );
}
