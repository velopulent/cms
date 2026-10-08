use serde_json::{Value, json};

use crate::common::TestServer;

async fn setup(server: &TestServer) -> (String, String) {
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server.login_user(&client, "admin@cms.local", "admin").await;
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
        .json(&json!({"name": "Token", "scopes": crate::common::fixtures::site_key_scopes("write")}))
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

async fn create_collection(server: &TestServer, site_id: &str, name: &str, slug: &str) {
    let client = reqwest::Client::new();
    let login = server.login_user(&client, "admin@cms.local", "admin").await;
    let (session, csrf) = crate::common::auth::extract_cookies(&login);
    let response = client
        .post(format!("{}/api/dashboard/sites/{site_id}/collections", server.base_url))
        .headers(crate::common::auth::auth_header(&session, &csrf))
        .json(&json!({"name":name,"slug":slug,"definition":{"fields":[{"name":"title","type":"text"}]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
}

#[tokio::test]
async fn test_collections_query() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    create_collection(&server, &site_id, "Posts", "posts").await;

    let body = gql(&server, &token, "{ collections { id name slug } }").await;
    assert!(body["errors"].is_null());
    let cols = body["data"]["collections"].as_array().unwrap();
    assert!(!cols.is_empty());
}

#[tokio::test]
async fn test_collection_by_slug() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    create_collection(&server, &site_id, "Pages", "pages").await;

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
async fn management_mutations_are_absent_from_public_schema() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;
    let body = gql(&server, &token, "{ __schema { mutationType { fields { name } } } }").await;
    assert!(body["errors"].is_null(), "{body}");
    let names = body["data"]["__schema"]["mutationType"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    for name in [
        "createCollection",
        "updateCollection",
        "deleteCollection",
        "createWebhook",
        "updateWebhook",
        "deleteWebhook",
        "batchDeleteFiles",
        "batchRestoreFiles",
    ] {
        assert!(!names.contains(&name), "Public management mutation exposed: {name}");
    }
    assert!(names.contains(&"createEntry"));
}
