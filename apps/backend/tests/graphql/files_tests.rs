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
        .json(&json!({"name": "File Site", "storage_provider": "filesystem"}))
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

async fn upload_file_via_rest(server: &TestServer, token: &str) -> String {
    let client = reqwest::Client::builder().build().unwrap();
    let part = reqwest::multipart::Part::bytes(b"test content".to_vec())
        .file_name("test.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let resp = client
        .post(format!("{}/files", server.base_url))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .unwrap();
    let val: Value = resp.json().await.unwrap();
    val["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_files_query() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    upload_file_via_rest(&server, &token).await;

    let body = gql(
        &server,
        &token,
        "{ files { id filename originalName mimeType size url } }",
    )
    .await;
    assert!(body["errors"].is_null());
    let files = body["data"]["files"].as_array().unwrap();
    assert!(!files.is_empty());
}

#[tokio::test]
async fn test_file_by_id() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let file_id = upload_file_via_rest(&server, &token).await;

    let query = format!(r#"{{ file(id: "{}") {{ id filename url thumbnailUrl }} }}"#, file_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["file"]["id"].as_str().unwrap(), file_id);
    assert!(body["data"]["file"]["url"].as_str().unwrap().contains("/api/files/"));
}

#[tokio::test]
async fn test_file_not_found() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let body = gql(&server, &token, r#"{ file(id: "nonexistent") { id } }"#).await;
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(msg.contains("not found"));
}

#[tokio::test]
async fn test_file_references_query() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let file_id = upload_file_via_rest(&server, &token).await;

    let query = format!(
        r#"{{ fileReferences(fileId: "{}") {{ entryId collectionName fieldName }} }}"#,
        file_id
    );
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    let _ = body["data"]["fileReferences"].as_array().unwrap();
    // TODO: we should create some entries that reference the file and verify they are returned here, but for now just check the structure of the response
    // let refs = body["data"]["fileReferences"].as_array().unwrap();
    // assert!(!refs.is_empty(), "expected file references");
}

#[tokio::test]
async fn test_delete_file_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let file_id = upload_file_via_rest(&server, &token).await;

    let query = format!(r#"mutation {{ deleteFile(id: "{}") }}"#, file_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert!(body["data"]["deleteFile"].as_bool().unwrap());
}

#[tokio::test]
async fn test_restore_file_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let file_id = upload_file_via_rest(&server, &token).await;

    let del_body = gql(
        &server,
        &token,
        &format!(r#"mutation {{ deleteFile(id: "{}") }}"#, file_id),
    )
    .await;
    assert!(
        del_body["errors"].is_null(),
        "deleteFile should succeed: {:?}",
        del_body["errors"]
    );

    let query = format!(r#"mutation {{ restoreFile(id: "{}") }}"#, file_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert!(body["data"]["restoreFile"].as_bool().unwrap());
}

#[tokio::test]
async fn test_batch_delete_files_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let mut ids = Vec::new();
    for _ in 0..3 {
        ids.push(upload_file_via_rest(&server, &token).await);
    }

    let query = r#"mutation BatchDelete($ids: [String!]!) { batchDeleteFiles(ids: $ids) }"#;
    let vars = json!({"ids": ids});
    let body = gql_with_vars(&server, &token, query, vars).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["batchDeleteFiles"].as_i64().unwrap(), 3);
}

#[tokio::test]
async fn test_batch_restore_files_mutation() {
    let server = TestServer::start().await;
    let (_, token) = setup(&server).await;

    let mut ids = Vec::new();
    for _ in 0..2 {
        ids.push(upload_file_via_rest(&server, &token).await);
    }

    for id in &ids {
        let del_body = gql(&server, &token, &format!(r#"mutation {{ deleteFile(id: "{}") }}"#, id)).await;
        assert!(
            del_body["errors"].is_null(),
            "deleteFile should succeed: {:?}",
            del_body["errors"]
        );
    }

    let query = r#"mutation BatchRestore($ids: [String!]!) { batchRestoreFiles(ids: $ids) }"#;
    let vars = json!({"ids": ids});
    let body = gql_with_vars(&server, &token, query, vars).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["batchRestoreFiles"].as_i64().unwrap(), 2);
}
