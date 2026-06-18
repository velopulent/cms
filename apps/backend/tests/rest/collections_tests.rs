use serde_json::{Value, json};

use crate::common::{TestServer, auth::auth_header, fixtures::setup};

async fn create_collection(server: &TestServer, jwt: &str, csrf: &str, site_id: &str, name: &str, slug: &str) -> Value {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(jwt, csrf))
        .json(&json!({
            "name": name,
            "slug": slug,
            "definition": {"fields": [{"name": "title", "type": "text", "required": true}]}
        }))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "Create collection failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    resp.json().await.unwrap()
}

#[tokio::test]
async fn test_create_collection() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "name": "Posts",
            "slug": "posts",
            "definition": {"fields": [{"name": "title", "type": "text", "required": true}]},
            "is_singleton": false,
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "Posts");
    assert_eq!(body["slug"], "posts");
}

#[tokio::test]
async fn test_list_collections() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_collection(&server, &jwt, &csrf, &site_id, "Posts", "posts").await;

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
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
async fn test_get_collection() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_collection(&server, &jwt, &csrf, &site_id, "Pages", "pages").await;

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/collections/pages",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "Pages");
}

#[tokio::test]
async fn test_get_collection_not_found() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/collections/nonexistent",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_update_collection() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_collection(&server, &jwt, &csrf, &site_id, "Old Name", "old-name").await;

    let resp = client
        .put(format!(
            "{}/api/dashboard/sites/{}/collections/old-name",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "name": "New Name",
            "slug": "new-name",
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "New Name");
    assert_eq!(body["slug"], "new-name");
}

#[tokio::test]
async fn test_delete_collection() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_collection(&server, &jwt, &csrf, &site_id, "To Delete", "to-delete").await;

    let resp = client
        .delete(format!(
            "{}/api/dashboard/sites/{}/collections/to-delete",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);

    let get_resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/collections/to-delete",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 404);
}

#[tokio::test]
async fn test_public_api_collections() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_collection(&server, &jwt, &csrf, &site_id, "Public Col", "public-col").await;

    let token_resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"name": "Test Token", "permission": "read"}))
        .send()
        .await
        .unwrap();
    let token_val: Value = token_resp.json().await.unwrap();
    let api_key = token_val["token"].as_str().unwrap();

    let resp = client
        .get(format!("{}/collections", server.base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.is_array());
}

#[tokio::test]
async fn test_create_collection_invalid_field_type() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "name": "Bad Def",
            "slug": "bad-def",
            "definition": {"fields": [{"name": "title", "type": "string"}]}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    let error = body["error"].as_str().unwrap_or("");
    assert!(
        error.contains("Invalid definition") || error.contains("invalid type") || error.contains("field type"),
        "Expected validation error, got: {}",
        error
    );
}

#[tokio::test]
async fn test_create_collection_duplicate_slug() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;

    create_collection(&server, &jwt, &csrf, &site_id, "Posts", "posts").await;

    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "name": "Posts Again",
            "slug": "posts",
            "definition": {"fields": [{"name": "title", "type": "text"}]}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 409);
}
