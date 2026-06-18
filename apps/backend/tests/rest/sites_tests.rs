use serde_json::{Value, json};

use crate::common::{TestServer, auth::auth_header};

async fn login_and_get_cookies(
    server: &TestServer,
    client: &reqwest::Client,
    username: &str,
    password: &str,
) -> (String, String) {
    let resp = server.login_user(client, username, password).await;
    assert_eq!(
        resp.status(),
        200,
        "Login failed: {}",
        resp.text().await.unwrap_or_default()
    );

    let headers = resp.headers();
    let mut jwt = String::new();
    let mut csrf = String::new();

    let cookies = headers.get_all("set-cookie").iter();
    for cookie in cookies {
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

    (jwt, csrf)
}

async fn create_site(server: &TestServer, jwt: &str, csrf: &str, name: &str) -> Value {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .headers(auth_header(jwt, csrf))
        .json(&json!({
            "name": name,
            "storage_provider": "filesystem",
        }))
        .send()
        .await
        .unwrap();

    assert!(
        resp.status().is_success(),
        "Create site failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    resp.json().await.unwrap()
}

#[tokio::test]
async fn test_create_site() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();
    let (jwt, csrf) = login_and_get_cookies(&server, &client, "admin", "admin").await;

    let body = create_site(&server, &jwt, &csrf, "Test Site").await;
    assert_eq!(body["name"], "Test Site");
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn test_list_sites() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();
    let (jwt, csrf) = login_and_get_cookies(&server, &client, "admin", "admin").await;

    create_site(&server, &jwt, &csrf, "Site One").await;

    let resp = client
        .get(format!("{}/api/dashboard/sites", server.base_url))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.is_array());
    let sites = body.as_array().unwrap();
    assert!(!sites.is_empty());
}

#[tokio::test]
async fn test_get_site() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();
    let (jwt, csrf) = login_and_get_cookies(&server, &client, "admin", "admin").await;

    let created = create_site(&server, &jwt, &csrf, "My Site").await;
    let site_id = created["id"].as_str().unwrap();

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "My Site");
}

#[tokio::test]
async fn test_update_site() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();
    let (jwt, csrf) = login_and_get_cookies(&server, &client, "admin", "admin").await;

    let created = create_site(&server, &jwt, &csrf, "Old Name").await;
    let site_id = created["id"].as_str().unwrap();

    let resp = client
        .put(format!("{}/api/dashboard/sites/{}", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "name": "New Name",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "New Name");
}

#[tokio::test]
async fn test_delete_site() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();
    let (jwt, csrf) = login_and_get_cookies(&server, &client, "admin", "admin").await;

    let created = create_site(&server, &jwt, &csrf, "To Delete").await;
    let site_id = created["id"].as_str().unwrap();

    let resp = client
        .delete(format!("{}/api/dashboard/sites/{}", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_list_members() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();
    let (jwt, csrf) = login_and_get_cookies(&server, &client, "admin", "admin").await;

    let created = create_site(&server, &jwt, &csrf, "Members Site").await;
    let site_id = created["id"].as_str().unwrap();

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/members", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.is_array());
}

#[tokio::test]
async fn test_unauthenticated_site_access() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .get(format!("{}/api/dashboard/sites", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}
