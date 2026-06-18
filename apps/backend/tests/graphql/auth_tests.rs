use serde_json::{Value, json};

use crate::common::TestServer;

async fn gql(server: &TestServer, token: Option<&str>, query: &str) -> reqwest::Response {
    let client = reqwest::Client::builder().build().unwrap();
    let mut req = client
        .post(format!("{}/api/graphql", server.base_url))
        .json(&json!({"query": query}));

    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }

    req.send().await.unwrap()
}

async fn setup_site_token(server: &TestServer) -> (reqwest::Client, String) {
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server.login_user(&client, "admin", "admin").await;
    let headers = resp.headers();
    let mut jwt = String::new();
    let mut csrf = String::new();
    for cookie in headers.get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if val.starts_with("token=") {
                jwt = val
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
        .header("Cookie", format!("token={}; csrf={}", jwt, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Test Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_id))
        .header("Cookie", format!("token={}; csrf={}", jwt, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "GraphQL Token", "permission": "write"}))
        .send()
        .await
        .unwrap();
    let token_val: Value = resp.json().await.unwrap();
    let api_key = token_val["token"].as_str().unwrap().to_string();

    (client, api_key)
}

async fn setup_read_token(server: &TestServer) -> String {
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server.login_user(&client, "admin", "admin").await;
    let headers = resp.headers();
    let mut jwt = String::new();
    let mut csrf = String::new();
    for cookie in headers.get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if val.starts_with("token=") {
                jwt = val
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
        .header("Cookie", format!("token={}; csrf={}", jwt, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Read Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_id))
        .header("Cookie", format!("token={}; csrf={}", jwt, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Read Token", "permission": "read"}))
        .send()
        .await
        .unwrap();
    let token_val: Value = resp.json().await.unwrap();
    token_val["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_unauthenticated_query() {
    let server = TestServer::start().await;
    let resp = gql(&server, None, "{ currentSite { id name } }").await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(msg.contains("authentication") || msg.contains("token"));
}

#[tokio::test]
async fn test_invalid_token() {
    let server = TestServer::start().await;
    let resp = gql(&server, Some("cms_invalid_token"), "{ currentSite { id name } }").await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["errors"].is_array());
}

#[tokio::test]
async fn test_read_token_cannot_write() {
    let server = TestServer::start().await;
    let token = setup_read_token(&server).await;

    let resp = gql(
        &server,
        Some(&token),
        r#"mutation { createCollection(input: {name: "Test", slug: "test", definition: "{}"}) { id } }"#,
    )
    .await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(msg.contains("write") || msg.contains("permission"));
}

#[tokio::test]
async fn test_wrong_site_token() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server.login_user(&client, "admin", "admin").await;
    let headers = resp.headers();
    let mut jwt = String::new();
    let mut csrf = String::new();
    for cookie in headers.get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if val.starts_with("token=") {
                jwt = val
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
        .header("Cookie", format!("token={}; csrf={}", jwt, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Site A", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site_a: Value = resp.json().await.unwrap();
    let site_a_id = site_a["id"].as_str().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .header("Cookie", format!("token={}; csrf={}", jwt, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Site B", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site_b: Value = resp.json().await.unwrap();
    let site_b_id = site_b["id"].as_str().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_a_id))
        .header("Cookie", format!("token={}; csrf={}", jwt, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Token A", "permission": "write"}))
        .send()
        .await
        .unwrap();
    let token_val: Value = resp.json().await.unwrap();
    let token_a = token_val["token"].as_str().unwrap();

    let query = format!(r#"{{ webhooks(siteId: "{}") {{ id label }} }}"#, site_b_id);
    let resp = gql(&server, Some(token_a), &query).await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(msg.contains("access") || msg.contains("site"));
}

#[tokio::test]
async fn test_graphiql_served() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .get(format!("{}/api/graphql", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/html"));
}

#[tokio::test]
async fn test_introspection_query() {
    let server = TestServer::start().await;
    let (_, token) = setup_site_token(&server).await;

    let resp = gql(&server, Some(&token), "{ __schema { types { name } } }").await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["data"]["__schema"]["types"].is_array());
    let types = body["data"]["__schema"]["types"].as_array().unwrap();
    let type_names: Vec<&str> = types.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(type_names.contains(&"Site"));
    assert!(type_names.contains(&"Collection"));
    assert!(type_names.contains(&"Entry"));
}

#[tokio::test]
async fn test_valid_read_token_query() {
    let server = TestServer::start().await;
    let token = setup_read_token(&server).await;

    let resp = gql(&server, Some(&token), "{ currentSite { id name } }").await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["data"].is_object());
    assert!(body["errors"].is_null() || body["errors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_valid_write_token_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup_site_token(&server).await;

    let resp = gql(
        &server,
        Some(&token),
        r#"mutation { createCollection(input: {name: "Test", slug: "test-mut", definition: "{}"}) { id name } }"#,
    )
    .await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["data"].is_object());
    assert!(body["data"]["createCollection"]["name"].as_str().unwrap() == "Test");
}
