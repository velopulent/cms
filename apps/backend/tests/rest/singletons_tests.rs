use serde_json::{Value, json};

use crate::common::{TestServer, auth::auth_header, fixtures::setup};

async fn create_singleton(server: &TestServer, token: &str, csrf: &str, site_id: &str, slug: &str) -> Value {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(token, csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_singleton(&server, &token, &csrf, &site_id, "settings").await;

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/singletons",
            server.base_url, site_id
        ))
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
async fn test_get_singleton() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_singleton(&server, &token, &csrf, &site_id, "homepage").await;

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/singletons/homepage",
            server.base_url, site_id
        ))
        .headers(auth_header(&token, &csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_singleton(&server, &token, &csrf, &site_id, "about").await;

    let resp = client
        .put(format!(
            "{}/api/dashboard/sites/{}/singletons/about",
            server.base_url, site_id
        ))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"data": {"title": "About Us", "body": "We are awesome"}}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let entry_id = body["entry_id"]
        .as_str()
        .expect("entry_id should be present on singleton response");
    assert!(!entry_id.is_empty(), "entry_id should not be empty");

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/entries/{}/revisions",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let revisions: Value = resp.json().await.unwrap();
    let revs = revisions["items"]
        .as_array()
        .expect("revisions.items should be an array");
    assert!(
        !revs.is_empty(),
        "singleton upsert should produce at least one revision"
    );
}

#[tokio::test]
async fn test_get_singleton_not_found() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/singletons/nonexistent",
            server.base_url, site_id
        ))
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_get_not_a_singleton() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(&token, &csrf))
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
        .get(format!(
            "{}/api/dashboard/sites/{}/singletons/posts",
            server.base_url, site_id
        ))
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_update_singleton_validation_failed() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(&token, &csrf))
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
        .put(format!(
            "{}/api/dashboard/sites/{}/singletons/settings",
            server.base_url, site_id
        ))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"data": {"count": "not-a-number"}}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}
