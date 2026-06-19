use serde_json::{Value, json};

use crate::common::TestServer;

async fn setup(server: &TestServer) -> (reqwest::Client, String, String) {
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
        .json(&json!({"name": "GQL Site", "storage_provider": "filesystem"}))
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

    (client, site_id, token)
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
async fn test_current_site() {
    let server = TestServer::start().await;
    let (_, _, token) = setup(&server).await;

    let body = gql(&server, &token, "{ currentSite { id name storageProvider } }").await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["currentSite"]["name"].as_str().unwrap(), "GQL Site");
    assert_eq!(
        body["data"]["currentSite"]["storageProvider"].as_str().unwrap(),
        "filesystem"
    );
}
