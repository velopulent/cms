use serde_json::{Value, json};

use crate::common::{TestServer, auth::auth_header, fixtures::setup};

async fn create_collection_and_get_id(server: &TestServer, token: &str, csrf: &str, site_id: &str) -> String {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(token, csrf))
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
    token: &str,
    csrf: &str,
    site_id: &str,
    collection_id: &str,
    slug: &str,
    data: Value,
) -> Value {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(token, csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;

    let entry = create_entry(
        &server,
        &token,
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
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    create_entry(
        &server,
        &token,
        &csrf,
        &site_id,
        &col_id,
        "post-1",
        json!({"title": "First"}),
    )
    .await;

    let resp = client
        .get(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_list_entries_with_search() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &token,
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
        .headers(auth_header(&token, &csrf))
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
        .headers(auth_header(&token, &csrf))
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

// ───────────────────────── Full-text search (Tantivy) ─────────────────────────

/// GET entries with a `search` query, returning the matching slugs in result
/// (rank) order.
async fn search_slugs(server: &TestServer, token: &str, csrf: &str, site_id: &str, query: &str) -> Vec<String> {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .get(format!(
            "{}/api/dashboard/sites/{}/entries?search={}",
            server.base_url, site_id, query
        ))
        .headers(auth_header(token, csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    body["items"]
        .as_array()
        .expect("items array")
        .iter()
        .filter_map(|i| i["slug"].as_str().map(String::from))
        .collect()
}

/// Indexing is asynchronous (writes enqueue; the indexer applies them), so poll
/// the search endpoint until `pred` holds or a timeout elapses, returning the last
/// observed slugs.
async fn search_until(
    server: &TestServer,
    token: &str,
    csrf: &str,
    site_id: &str,
    query: &str,
    pred: impl Fn(&[String]) -> bool,
) -> Vec<String> {
    let mut slugs = Vec::new();
    for _ in 0..100 {
        slugs = search_slugs(server, token, csrf, site_id, query).await;
        if pred(&slugs) {
            return slugs;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    slugs
}

#[tokio::test]
async fn test_search_stemmed_match_like_would_miss() {
    let server = TestServer::start_with_search().await;
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;

    create_entry(
        &server,
        &token,
        &csrf,
        &site_id,
        &col_id,
        "runner",
        json!({"title": "I love to run"}),
    )
    .await;

    // Stemming reduces both "running" and "run" to the same root. A SQL
    // LIKE '%running%' over the stored text "...run" would find nothing.
    let slugs = search_until(&server, &token, &csrf, &site_id, "running", |s| {
        s.contains(&"runner".to_string())
    })
    .await;
    assert!(
        slugs.contains(&"runner".to_string()),
        "stemmed search missed: {:?}",
        slugs
    );
}

#[tokio::test]
async fn test_search_ranks_by_relevance() {
    let server = TestServer::start_with_search().await;
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;

    create_entry(
        &server,
        &token,
        &csrf,
        &site_id,
        &col_id,
        "sparse",
        json!({"title": "alpha lone"}),
    )
    .await;
    create_entry(
        &server,
        &token,
        &csrf,
        &site_id,
        &col_id,
        "dense",
        json!({"title": "alpha alpha alpha"}),
    )
    .await;

    // Wait until both are indexed, then assert the higher term-frequency one ranks first.
    let slugs = search_until(&server, &token, &csrf, &site_id, "alpha", |s| s.len() >= 2).await;
    assert_eq!(
        slugs.first().map(String::as_str),
        Some("dense"),
        "expected the higher term-frequency entry ranked first, got: {:?}",
        slugs
    );
}

#[tokio::test]
async fn test_search_index_syncs_on_delete() {
    let server = TestServer::start_with_search().await;
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;

    let entry = create_entry(
        &server,
        &token,
        &csrf,
        &site_id,
        &col_id,
        "ephemeral",
        json!({"title": "vanishing content"}),
    )
    .await;
    let entry_id = entry["id"].as_str().unwrap();

    // Becomes searchable once the indexer drains the enqueued write.
    let present = search_until(&server, &token, &csrf, &site_id, "vanishing", |s| {
        s.contains(&"ephemeral".to_string())
    })
    .await;
    assert!(present.contains(&"ephemeral".to_string()), "entry should be searchable");

    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .delete(format!(
            "{}/api/dashboard/sites/{}/entries/{}",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "delete failed: {}", resp.status());

    // And drops out of the index once the delete is drained.
    let after = search_until(&server, &token, &csrf, &site_id, "vanishing", |s| s.is_empty()).await;
    assert!(
        after.is_empty(),
        "entry should be gone from the index after deletion: {:?}",
        after
    );
}

#[tokio::test]
async fn test_create_entry_validation_failed_missing_required() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(&token, &csrf))
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
        .headers(auth_header(&token, &csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(&token, &csrf))
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
        .headers(auth_header(&token, &csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &token,
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
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_update_entry() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &token,
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
        .headers(auth_header(&token, &csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &token,
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
        .headers(auth_header(&token, &csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &token,
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
        .headers(auth_header(&token, &csrf))
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
        .headers(auth_header(&token, &csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &token,
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
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_list_revisions() {
    let server = TestServer::start().await;
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &token,
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
        .headers(auth_header(&token, &csrf))
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
        .headers(auth_header(&token, &csrf))
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
    let (token, csrf, site_id) = setup(&server).await;
    let col_id = create_collection_and_get_id(&server, &token, &csrf, &site_id).await;
    let client = reqwest::Client::builder().build().unwrap();

    let entry = create_entry(
        &server,
        &token,
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
        .headers(auth_header(&token, &csrf))
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
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
}
