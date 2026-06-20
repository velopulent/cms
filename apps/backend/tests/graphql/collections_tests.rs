use serde_json::{Value, json};

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
        .json(&json!({"name": "Collection Site", "storage_provider": "filesystem"}))
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

async fn create_collection(server: &TestServer, token: &str, name: &str, slug: &str) -> Value {
    let query = r#"mutation CreateCollection($input: CreateCollectionInput!) {
        createCollection(input: $input) { id name slug }
    }"#;
    let vars = json!({"input": {"name": name, "slug": slug, "definition": json!({"fields": [{"name": "title", "type": "text"}]})}});
    gql_with_vars(server, token, query, vars).await
}

async fn create_singleton_collection(server: &TestServer, token: &str, name: &str, slug: &str) -> Value {
    let query = r#"mutation CreateCollection($input: CreateCollectionInput!) {
        createCollection(input: $input) { id name slug isSingleton }
    }"#;
    let vars = json!({"input": {"name": name, "slug": slug, "definition": json!({"fields": [{"name": "title", "type": "text"}]}), "isSingleton": true}});
    gql_with_vars(server, token, query, vars).await
}

#[tokio::test]
async fn test_collections_query() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    create_collection(&server, &token, "Posts", "posts").await;

    let body = gql(&server, &token, "{ collections { id name slug } }").await;
    assert!(body["errors"].is_null());
    let cols = body["data"]["collections"].as_array().unwrap();
    assert!(!cols.is_empty());
}

#[tokio::test]
async fn test_collection_by_slug() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    create_collection(&server, &token, "Pages", "pages").await;

    let body = gql(&server, &token, r#"{ collection(slug: "pages") { id name slug } }"#).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["collection"]["name"].as_str().unwrap(), "Pages");
}

#[tokio::test]
async fn test_collection_not_found() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let body = gql(&server, &token, r#"{ collection(slug: "nonexistent") { id } }"#).await;
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(msg.contains("not found"));
}

#[tokio::test]
async fn test_create_collection_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let body = create_collection(&server, &token, "New Col", "new-col").await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["createCollection"]["name"].as_str().unwrap(), "New Col");
    assert_eq!(body["data"]["createCollection"]["slug"].as_str().unwrap(), "new-col");
}

#[tokio::test]
async fn test_update_collection_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    create_collection(&server, &token, "Old Name", "old-name").await;

    let query = r#"mutation { updateCollection(slug: "old-name", input: {name: "New Name", slug: "new-name"}) { id name slug } }"#;
    let body = gql(&server, &token, query).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["updateCollection"]["name"].as_str().unwrap(), "New Name");
    assert_eq!(body["data"]["updateCollection"]["slug"].as_str().unwrap(), "new-name");
}

#[tokio::test]
async fn test_delete_collection_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    create_collection(&server, &token, "To Delete", "to-delete").await;

    let body = gql(&server, &token, r#"mutation { deleteCollection(slug: "to-delete") }"#).await;
    assert!(body["errors"].is_null());
    assert!(body["data"]["deleteCollection"].as_bool().unwrap());
}

#[tokio::test]
async fn test_create_singleton_collection() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let body = create_singleton_collection(&server, &token, "Settings", "settings").await;
    assert!(body["errors"].is_null(), "errors: {:?}", body["errors"]);
    assert_eq!(body["data"]["createCollection"]["name"].as_str().unwrap(), "Settings");
    assert_eq!(body["data"]["createCollection"]["slug"].as_str().unwrap(), "settings");
    assert!(body["data"]["createCollection"]["isSingleton"].as_bool().unwrap());
}

#[tokio::test]
async fn test_create_collection_invalid_field_type() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let query = r#"mutation CreateCollection($input: CreateCollectionInput!) {
        createCollection(input: $input) { id name }
    }"#;
    let vars = json!({"input": {"name": "Bad", "slug": "bad", "definition": json!({"fields": [{"name": "title", "type": "string"}]})}});
    let body = gql_with_vars(&server, &token, query, vars).await;

    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(
        msg.contains("Invalid definition") || msg.contains("invalid type"),
        "Expected validation error: {}",
        msg
    );
}

#[tokio::test]
async fn test_create_collection_duplicate_slug() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    create_collection(&server, &token, "Posts", "dup-slug").await;

    let query = r#"mutation CreateCollection($input: CreateCollectionInput!) {
        createCollection(input: $input) { id }
    }"#;
    let vars = json!({"input": {"name": "Posts Again", "slug": "dup-slug", "definition": json!({"fields": [{"name": "title", "type": "text"}]})}});
    let body = gql_with_vars(&server, &token, query, vars).await;

    assert!(body["errors"].is_array());
}

#[tokio::test]
async fn test_collection_not_singleton() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    create_collection(&server, &token, "Regular", "regular").await;

    let body = gql(&server, &token, r#"{ collection(slug: "regular") { id isSingleton } }"#).await;
    assert!(body["errors"].is_null(), "errors: {:?}", body["errors"]);
    assert!(!body["data"]["collection"]["isSingleton"].as_bool().unwrap());
}
