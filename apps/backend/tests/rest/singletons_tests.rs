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

async fn create_singleton(server: &TestServer, jwt: &str, csrf: &str, site_id: &str, slug: &str) -> Value {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/collections", server.base_url, site_id))
        .headers(auth_header(jwt, csrf))
        .json(&json!({
            "name": slug,
            "slug": slug,
            "definition": {"fields": [{"name": "title", "type": "text"}]},
            "is_singleton": true,
        }))
        .send()
        .await
        .unwrap();
    resp.json().await.unwrap()
}

#[tokio::test]
async fn test_list_singletons() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_singleton(&server, &jwt, &csrf, &site_id, "settings").await;

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/singletons", server.base_url, site_id))
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
async fn test_get_singleton() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_singleton(&server, &jwt, &csrf, &site_id, "homepage").await;

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/singletons/homepage", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["slug"], "homepage");
}

#[tokio::test]
async fn test_update_singleton() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_singleton(&server, &jwt, &csrf, &site_id, "about").await;

    let resp = client
        .put(format!("{}/api/dashboard/sites/{}/singletons/about", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"data": {"title": "About Us", "body": "We are awesome"}}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let entry_id = body["entry_id"].as_str().expect("entry_id should be present on singleton response");
    assert!(!entry_id.is_empty(), "entry_id should not be empty");

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/entries/{}/revisions", server.base_url, site_id, entry_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let revisions: Value = resp.json().await.unwrap();
    let revs = revisions["items"].as_array().expect("revisions.items should be an array");
    assert!(!revs.is_empty(), "singleton upsert should produce at least one revision");
}

#[tokio::test]
async fn test_get_singleton_not_found() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/singletons/nonexistent", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_get_not_a_singleton() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    client
        .post(format!("{}/api/dashboard/sites/{}/collections", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "name": "Posts",
            "slug": "posts",
            "definition": {"fields": [{"name": "title", "type": "text"}]},
            "is_singleton": false,
        }))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/singletons/posts", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_update_singleton_validation_failed() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    client
        .post(format!("{}/api/dashboard/sites/{}/collections", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "name": "Settings",
            "slug": "settings",
            "definition": {"fields": [{"name": "count", "type": "number", "required": true}]},
            "is_singleton": true,
        }))
        .send()
        .await
        .unwrap();

    let resp = client
        .put(format!("{}/api/dashboard/sites/{}/singletons/settings", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"data": {"count": "not-a-number"}}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}