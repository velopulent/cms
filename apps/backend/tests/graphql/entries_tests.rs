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
        .json(&json!({"name": "Entry Site", "storage_provider": "filesystem"}))
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

async fn create_collection(server: &TestServer, token: &str, name: &str, slug: &str) -> String {
    let query = r#"mutation CreateCollection($input: CreateCollectionInput!) {
        createCollection(input: $input) { id }
    }"#;
    let vars = json!({"input": {"name": name, "slug": slug, "definition": json!({"fields": [{"name": "title", "type": "text"}]})}});
    let body = gql_with_vars(server, token, query, vars).await;
    body["data"]["createCollection"]["id"].as_str().unwrap().to_string()
}

async fn create_entry(server: &TestServer, token: &str, collection_id: &str, slug: &str, data: Value) -> Value {
    let query = r#"mutation CreateEntry($input: CreateEntryInput!) {
        createEntry(input: $input) { id slug status collectionId }
    }"#;
    let vars = json!({"input": {"collectionId": collection_id, "slug": slug, "data": data}});
    gql_with_vars(server, token, query, vars).await
}

#[tokio::test]
async fn test_entries_query() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "entry-1", json!({"title": "First"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    gql(&server, &token, &format!(r#"mutation {{ publishEntry(id: "{}") {{ id }} }}"#, entry_id)).await;

    let body = gql(&server, &token, "{ entries { id slug status } }").await;
    assert!(body["errors"].is_null(), "errors: {:?}", body["errors"]);
    let entries = body["data"]["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_entries_with_status_filter() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    create_entry(&server, &token, &col_id, "draft-entry", json!({"title": "Draft"})).await;

    let body = gql(&server, &token, r#"{ entries(status: "draft") { id slug status } }"#).await;
    assert!(body["errors"].is_null());
    let entries = body["data"]["entries"].as_array().unwrap();
    assert!(entries.iter().all(|e| e["status"].as_str().unwrap() == "draft"));
}

#[tokio::test]
async fn test_entries_with_collection_id_filter() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    create_entry(&server, &token, &col_id, "my-post", json!({"title": "My Post"})).await;

    let query = format!(r#"{{ entries(collectionId: "{}") {{ id slug collectionId }} }}"#, col_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    let entries = body["data"]["entries"].as_array().unwrap();
    assert!(entries.iter().all(|e| e["collectionId"].as_str().unwrap() == col_id));
}

#[tokio::test]
async fn test_entries_with_search() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "searchable", json!({"title": "Unique Title"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    gql(&server, &token, &format!(r#"mutation {{ publishEntry(id: "{}") {{ id }} }}"#, entry_id)).await;

    let body = gql(&server, &token, r#"{ entries(search: "Unique") { id slug } }"#).await;
    assert!(body["errors"].is_null(), "errors: {:?}", body["errors"]);
    let entries = body["data"]["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
}

#[tokio::test]
async fn test_entries_with_pagination() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    for i in 0..5 {
        create_entry(&server, &token, &col_id, &format!("entry-{}", i), json!({"title": format!("Post {}", i)})).await;
    }

    let body = gql(&server, &token, "{ entries(page: 1, perPage: 2) { id } }").await;
    assert!(body["errors"].is_null());
    let entries = body["data"]["entries"].as_array().unwrap();
    assert!(entries.len() <= 2);
}

#[tokio::test]
async fn test_entry_by_id() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "get-me", json!({"title": "Get Me"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    let query = format!(r#"{{ entry(id: "{}") {{ id slug data }} }}"#, entry_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["entry"]["slug"].as_str().unwrap(), "get-me");
}

#[tokio::test]
async fn test_entry_not_found() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let body = gql(&server, &token, r#"{ entry(id: "nonexistent") { id } }"#).await;
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(msg.contains("not found"));
}

#[tokio::test]
async fn test_create_entry_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let body = create_entry(&server, &token, &col_id, "new-entry", json!({"title": "New"})).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["createEntry"]["slug"].as_str().unwrap(), "new-entry");
    assert_eq!(body["data"]["createEntry"]["status"].as_str().unwrap(), "draft");
}

#[tokio::test]
async fn test_create_entry_nonexistent_collection() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let body = create_entry(&server, &token, "nonexistent", "fail", json!({"title": "Fail"})).await;
    assert!(body["errors"].is_array());
}

#[tokio::test]
async fn test_update_entry_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "to-update", json!({"title": "Old"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    let query = r#"mutation UpdateEntry($id: String!, $input: UpdateEntryInput!) {
        updateEntry(id: $id, input: $input) { id data }
    }"#;
    let vars = json!({"id": entry_id, "input": {"data": json!({"title": "Updated"})}});
    let body = gql_with_vars(&server, &token, query, vars).await;
    assert!(body["errors"].is_null());
}

#[tokio::test]
async fn test_delete_entry_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "to-delete", json!({"title": "Bye"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    let query = format!(r#"mutation {{ deleteEntry(id: "{}") }}"#, entry_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["deleteEntry"].as_bool().unwrap(), true);
}

#[tokio::test]
async fn test_publish_entry_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "to-publish", json!({"title": "Draft"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    let query = format!(r#"mutation {{ publishEntry(id: "{}") {{ id status }} }}"#, entry_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["publishEntry"]["status"].as_str().unwrap(), "published");
}

#[tokio::test]
async fn test_unpublish_entry_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "to-unpublish", json!({"title": "Published"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    gql(&server, &token, &format!(r#"mutation {{ publishEntry(id: "{}") {{ status }} }}"#, entry_id)).await;

    let query = format!(r#"mutation {{ unpublishEntry(id: "{}") {{ id status }} }}"#, entry_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["unpublishEntry"]["status"].as_str().unwrap(), "draft");
}

#[tokio::test]
async fn test_restore_revision_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "revisioned", json!({"title": "V1"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    let update_query = r#"mutation UpdateEntry($id: String!, $input: UpdateEntryInput!) {
        updateEntry(id: $id, input: $input) { id }
    }"#;
    let vars = json!({"id": entry_id, "input": {"data": json!({"title": "V2"})}});
    let body = gql_with_vars(&server, &token, update_query, vars).await;
    assert!(body["errors"].is_null(), "update failed: {:?}", body["errors"]);
    assert_eq!(body["data"]["updateEntry"]["id"].as_str().unwrap(), entry_id);

    let restore_query = format!(r#"mutation {{ restoreRevision(entryId: "{}", revisionNumber: 1) {{ id data }} }}"#, entry_id);
    let body = gql(&server, &token, &restore_query).await;
    assert!(body["errors"].is_null());
}

#[tokio::test]
async fn test_entry_revisions_query() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "rev-test", json!({"title": "V1"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    let update_query = r#"mutation { updateEntry(id: "%ID%", input: {data: {title: "V2"}}) { id } }"#;
    let update_query = update_query.replace("%ID%", entry_id);
    gql(&server, &token, &update_query).await;

    let query = format!(r#"{{ entryRevisions(entryId: "{}") {{ items {{ revisionNumber data }} total }} }}"#, entry_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null(), "errors: {:?}", body["errors"]);
    let items = body["data"]["entryRevisions"]["items"].as_array().unwrap();
    assert!(items.len() >= 2, "expected >= 2 revisions, got {}", items.len());
}

#[tokio::test]
async fn test_entry_revision_query() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "rev-single", json!({"title": "V1"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    let query = format!(r#"{{ entryRevision(entryId: "{}", revisionNumber: 1) {{ revisionNumber data }} }}"#, entry_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["entryRevision"]["revisionNumber"].as_i64().unwrap(), 1);
}

#[tokio::test]
async fn test_entry_revision_with_diff() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let col_id = create_collection(&server, &token, "Posts", "posts").await;

    let created = create_entry(&server, &token, &col_id, "diff-test", json!({"title": "V1"})).await;
    let entry_id = created["data"]["createEntry"]["id"].as_str().unwrap();

    let update_query = r#"mutation { updateEntry(id: "%ID%", input: {data: {title: "V2"}}) { id } }"#;
    let update_query = update_query.replace("%ID%", entry_id);
    gql(&server, &token, &update_query).await;

    let query = format!(r#"{{ entryRevision(entryId: "{}", revisionNumber: 2, diff: true) {{ revisionNumber diffFromPrevious }} }}"#, entry_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null(), "errors: {:?}", body["errors"]);
    assert_eq!(body["data"]["entryRevision"]["revisionNumber"].as_i64().unwrap(), 2);
}

#[tokio::test]
async fn test_create_entry_with_invalid_collection_id() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let query = r#"mutation CreateEntry($input: CreateEntryInput!) {
        createEntry(input: $input) { id }
    }"#;
    let vars = json!({"input": {"collectionId": "nonexistent", "slug": "orphan", "data": json!({"title": "Hello"})}});
    let body = gql_with_vars(&server, &token, query, vars).await;

    assert!(body["errors"].is_array());
}
