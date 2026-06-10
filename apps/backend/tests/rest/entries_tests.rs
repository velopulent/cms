use serde_json::{Value, json};

use crate::common::TestServer;

async fn setup(server: &TestServer) -> (String, String, String) {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = server.login_user(&client, "admin", "admin").await;
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
    headers.insert(
        reqwest::header::COOKIE,
        reqwest::header::HeaderValue::from_str(&cookie_val).unwrap(),
    );
    headers.insert("X-CSRF-Token", reqwest::header::HeaderValue::from_str(csrf).unwrap());
    headers
}

async fn create_collection_and_get_id(server: &TestServer, jwt: &str, csrf: &str, site_id: &str) -> String {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(jwt, csrf))
        .json(&json!({
            "name": "Posts",
            "slug": "posts",
            "definition": {"fields": [{"name": "title", "type": "text", "required": true}]},
        }))
        .send()
        .await
        .unwrap();
    let col: Value = resp.json().await.unwrap();
    col["id"].as_str().unwrap().to_string()
}

async fn create_entry(
    server: &TestServer,
    jwt: &str,
    csrf: &str,
    site_id: &str,
    collection_id: &str,
    slug: &str,
    data: Value,
) -> Value {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(jwt, csrf))
        .json(&json!({
            "collection_id": collection_id,
            "slug": slug,
            "data": data,
        }))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "Create entry failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    resp.json().await.unwrap()
}

#[tokio::test]
async fn test_create_entry() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &jwt, &csrf, &site_id).await;

    let entry = create_entry(
        &server,
        &jwt,
        &csrf,
        &site_id,
        &col_id,
        "my-post",
        json!({"title": "Hello World"}),
    )
    .await;
    assert_eq!(entry["slug"], "my-post");
    assert_eq!(entry["status"], "draft");
}

#[tokio::test]
async fn test_list_entries() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &jwt, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_entry(
        &server,
        &jwt,
        &csrf,
        &site_id,
        &col_id,
        "post-1",
        json!({"title": "First"}),
    )
    .await;

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_list_entries_with_search() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &jwt, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &jwt,
        &csrf,
        &site_id,
        &col_id,
        "searchable",
        json!({"title": "Unique Title"}),
    )
    .await;
    let entry_id = entry["id"].as_str().unwrap();

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/entries/{}/publish",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "publish entry failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/entries?search=Unique",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("items array");
    let slugs: Vec<&str> = items.iter().filter_map(|i| i["slug"].as_str()).collect();
    assert!(
        slugs.contains(&"searchable"),
        "expected slug 'searchable' in search results, got: {:?}",
        slugs
    );
}

#[tokio::test]
async fn test_create_entry_validation_failed_missing_required() {
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
            "name": "Required Fields",
            "slug": "required-fields",
            "definition": {"fields": [{"name": "title", "type": "text", "required": true}]}
        }))
        .send()
        .await
        .unwrap();
    let col: Value = resp.json().await.unwrap();
    let col_id = col["id"].as_str().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "collection_id": col_id,
            "slug": "missing-title",
            "data": {}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    let error = body["error"].as_str().unwrap_or("");
    assert!(
        error.contains("required") || error.contains("Required"),
        "Expected required field error, got: {}",
        error
    );
}

#[tokio::test]
async fn test_create_entry_validation_failed_wrong_type() {
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
            "name": "Number Field",
            "slug": "number-field",
            "definition": {"fields": [{"name": "count", "type": "number", "required": true}]}
        }))
        .send()
        .await
        .unwrap();
    let col: Value = resp.json().await.unwrap();
    let col_id = col["id"].as_str().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "collection_id": col_id,
            "slug": "wrong-type",
            "data": {"count": "not-a-number"}
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    let error = body["error"].as_str().unwrap_or("");
    assert!(
        error.contains("number") || error.contains("type"),
        "Expected type error, got: {}",
        error
    );
}

#[tokio::test]
async fn test_create_entry_invalid_collection() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "collection_id": "nonexistent-collection",
            "slug": "orphan",
            "data": {"title": "Hello"}
        }))
        .send()
        .await
        .unwrap();

    let status = resp.status();
    assert!(
        status.is_client_error() || status.is_server_error(),
        "Expected error status for nonexistent collection, got: {}",
        status
    );
}

#[tokio::test]
async fn test_get_entry() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &jwt, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &jwt,
        &csrf,
        &site_id,
        &col_id,
        "test-entry",
        json!({"title": "Test"}),
    )
    .await;
    let entry_id = entry["id"].as_str().unwrap();

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/entries/{}",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_update_entry() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &jwt, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &jwt,
        &csrf,
        &site_id,
        &col_id,
        "to-update",
        json!({"title": "Old"}),
    )
    .await;
    let entry_id = entry["id"].as_str().unwrap();

    let resp = client
        .put(format!(
            "{}/api/dashboard/sites/{}/entries/{}",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"data": {"title": "Updated"}}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let data: Value = serde_json::from_str(body["data"].as_str().unwrap_or("{}")).unwrap_or_default();
    assert_eq!(data["title"], "Updated");
}

#[tokio::test]
async fn test_publish_entry() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &jwt, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &jwt,
        &csrf,
        &site_id,
        &col_id,
        "to-publish",
        json!({"title": "Draft"}),
    )
    .await;
    let entry_id = entry["id"].as_str().unwrap();

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/entries/{}/publish",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "published");
}

#[tokio::test]
async fn test_unpublish_entry() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &jwt, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &jwt,
        &csrf,
        &site_id,
        &col_id,
        "to-unpublish",
        json!({"title": "Published"}),
    )
    .await;
    let entry_id = entry["id"].as_str().unwrap();

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/entries/{}/publish",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "publish entry failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/entries/{}/unpublish",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "draft");
}

#[tokio::test]
async fn test_delete_entry() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &jwt, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &jwt,
        &csrf,
        &site_id,
        &col_id,
        "to-delete",
        json!({"title": "Bye"}),
    )
    .await;
    let entry_id = entry["id"].as_str().unwrap();

    let resp = client
        .delete(format!(
            "{}/api/dashboard/sites/{}/entries/{}",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_list_revisions() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &jwt, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &jwt,
        &csrf,
        &site_id,
        &col_id,
        "revisioned",
        json!({"title": "V1"}),
    )
    .await;
    let entry_id = entry["id"].as_str().unwrap();

    let resp = client
        .put(format!(
            "{}/api/dashboard/sites/{}/entries/{}",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"data": {"title": "V2"}}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "update entry for revisions failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/entries/{}/revisions",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body["items"].is_array());
    assert!(body["total"].as_i64().unwrap() >= 2);
}

#[tokio::test]
async fn test_restore_revision() {
    let server = TestServer::start().await;
    let (jwt, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &jwt, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &jwt,
        &csrf,
        &site_id,
        &col_id,
        "restorable",
        json!({"title": "Original"}),
    )
    .await;
    let entry_id = entry["id"].as_str().unwrap();

    let resp = client
        .put(format!(
            "{}/api/dashboard/sites/{}/entries/{}",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"data": {"title": "Changed"}}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "update entry for restore failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/entries/{}/revisions/1/restore",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}
