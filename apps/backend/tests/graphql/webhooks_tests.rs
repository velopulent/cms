use serde_json::{Value, json};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::TestServer;

async fn setup(server: &TestServer) -> (String, String) {
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server.login_user(&client, "admin", "admin").await;
    let headers = resp.headers();
    let mut token = String::new();
    let mut csrf = String::new();
    for cookie in headers.get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if val.starts_with("token=") {
                token = val
                    .split(';')
                    .next()
                    .and_then(|c| c.strip_prefix("token="))
                    .unwrap_or("")
                    .to_string();
            }
            if val.starts_with("csrf=") {
                csrf = val
                    .split(';')
                    .next()
                    .and_then(|c| c.strip_prefix("csrf="))
                    .unwrap_or("")
                    .to_string();
            }
        }
    }

    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .header("Cookie", format!("token={}; csrf={}", token, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Webhook Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap().to_string();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_id))
        .header("Cookie", format!("token={}; csrf={}", token, csrf))
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

async fn gql_with_vars(server: &TestServer, token: &str, query: &str, variables: Value) -> Value {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/api/graphql", server.base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"query": query, "variables": variables}))
        .send()
        .await
        .unwrap();
    resp.json().await.unwrap()
}

async fn create_webhook(server: &TestServer, token: &str, site_id: &str, label: &str, url: &str) -> Value {
    let query = r#"mutation CreateWebhook($siteId: String!, $label: String!, $url: String!) {
        createWebhook(siteId: $siteId, label: $label, url: $url) { id label url }
    }"#;
    let vars = json!({"siteId": site_id, "label": label, "url": url});
    gql_with_vars(server, token, query, vars).await
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

    let query = format!(
        r#"{{ webhook(siteId: "{}", webhookId: "nonexistent") {{ id }} }}"#,
        site_id
    );
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(msg.contains("not found"));
}

#[tokio::test]
async fn test_create_webhook_mutation() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let body = create_webhook(&server, &token, &site_id, "New Hook", "https://example.com/hook").await;
    assert!(body["errors"].is_null(), "errors: {:?}", body["errors"]);
    assert_eq!(body["data"]["createWebhook"]["label"].as_str().unwrap(), "New Hook");
}

#[tokio::test]
async fn test_webhook_by_id() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let created = create_webhook(&server, &token, &site_id, "My Hook", "https://example.com/hook").await;
    assert!(
        created["errors"].is_null(),
        "create_webhook errors: {:?}",
        created["errors"]
    );
    let hook_id = created["data"]["createWebhook"]["id"].as_str().unwrap();

    let query = format!(
        r#"{{ webhook(siteId: "{}", webhookId: "{}") {{ id label }} }}"#,
        site_id, hook_id
    );
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["webhook"]["label"].as_str().unwrap(), "My Hook");
}

#[tokio::test]
async fn test_delete_webhook_mutation() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let created = create_webhook(&server, &token, &site_id, "Delete Me", "https://example.com/hook").await;
    let hook_id = created["data"]["createWebhook"]["id"].as_str().unwrap();

    let query = format!(
        r#"mutation {{ deleteWebhook(siteId: "{}", webhookId: "{}") }}"#,
        site_id, hook_id
    );
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert!(body["data"]["deleteWebhook"].as_bool().unwrap());
}

#[tokio::test]
async fn test_update_webhook_mutation() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let created = create_webhook(&server, &token, &site_id, "Old Label", "https://example.com/hook").await;
    let hook_id = created["data"]["createWebhook"]["id"].as_str().unwrap();

    let query = r#"mutation UpdateWebhook($siteId: String!, $webhookId: String!, $label: String!) {
        updateWebhook(siteId: $siteId, webhookId: $webhookId, label: $label) { id label }
    }"#;
    let vars = json!({"siteId": site_id, "webhookId": hook_id, "label": "New Label"});
    let body = gql_with_vars(&server, &token, query, vars).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["updateWebhook"]["label"].as_str().unwrap(), "New Label");
}

#[tokio::test]
async fn test_trigger_webhook_mutation() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let created = create_webhook(&server, &token, &site_id, "Trigger Me", &mock_server.uri()).await;
    let hook_id = created["data"]["createWebhook"]["id"].as_str().unwrap();

    let query = format!(
        r#"mutation {{ triggerWebhook(siteId: "{}", webhookId: "{}") {{ id status }} }}"#,
        site_id, hook_id
    );
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["triggerWebhook"]["status"].as_str().unwrap(), "success");
}

#[tokio::test]
async fn test_webhook_deliveries_query() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let created = create_webhook(&server, &token, &site_id, "Delivery Hook", &mock_server.uri()).await;
    let hook_id = created["data"]["createWebhook"]["id"].as_str().unwrap();

    let query = format!(
        r#"mutation {{ triggerWebhook(siteId: "{}", webhookId: "{}") {{ id status }} }}"#,
        site_id, hook_id
    );
    let trigger_body = gql(&server, &token, &query).await;
    assert!(
        trigger_body["errors"].is_null(),
        "triggerWebhook should succeed: {:?}",
        trigger_body["errors"]
    );

    let query = format!(
        r#"{{ webhookDeliveries(siteId: "{}", webhookId: "{}") {{ id status }} }}"#,
        site_id, hook_id
    );
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    let deliveries = body["data"]["webhookDeliveries"].as_array().unwrap();
    assert!(!deliveries.is_empty(), "expected webhook deliveries");
}
