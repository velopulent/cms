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

async fn upload_file_via_rest(server: &TestServer, token: &str, site_id: &str) -> String {
    let client = reqwest::Client::builder().build().unwrap();
    let part = reqwest::multipart::Part::bytes(b"test content".to_vec())
        .file_name("test.txt")
        .mime_str("text/plain")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);

    let resp = client
        .post(format!("{}/api/v1/sites/{site_id}/files", server.base_url))
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
    let (site_id, token) = setup(&server).await;

    upload_file_via_rest(&server, &token, &site_id).await;

    let body = gql(
        &server,
        &token,
        &format!(r#"{{ site(id: "{site_id}") {{ files {{ id filename originalName mimeType size url }} }} }}"#),
    )
    .await;
    assert!(body["errors"].is_null());
    let files = body["data"]["site"]["files"].as_array().unwrap();
    assert!(!files.is_empty());
}

#[tokio::test]
async fn test_file_by_id() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let file_id = upload_file_via_rest(&server, &token, &site_id).await;

    let query =
        format!(r#"{{ site(id: "{site_id}") {{ file(id: "{file_id}") {{ id filename url thumbnailUrl }} }} }}"#);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert_eq!(body["data"]["site"]["file"]["id"].as_str().unwrap(), file_id);
    assert!(
        body["data"]["site"]["file"]["url"]
            .as_str()
            .unwrap()
            .contains("/api/files/")
    );
}

#[tokio::test]
async fn test_file_not_found() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let query = format!(r#"{{ site(id: "{site_id}") {{ file(id: "nonexistent") {{ id }} }} }}"#);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(msg.contains("not found"));
}

#[tokio::test]
async fn test_file_references_query() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let file_id = upload_file_via_rest(&server, &token, &site_id).await;

    let client = reqwest::Client::new();
    let login = server.login_user(&client, "admin@cms.local", "admin").await;
    let (session, csrf) = crate::common::auth::extract_cookies(&login);
    let response = client.post(format!("{}/api/dashboard/sites/{site_id}/collections",server.base_url))
        .headers(crate::common::auth::auth_header(&session,&csrf))
        .json(&json!({"name":"Media","slug":"media","definition":{"fields":[{"name":"hero","type":"file"},{"name":"gallery","type":"file","multiple":true}]}}))
        .send().await.unwrap();
    assert_eq!(response.status(), 201);
    let collection: Value = response.json().await.unwrap();
    let response = client.post(format!("{}/api/v1/sites/{site_id}/collections/media/entries",server.base_url))
        .bearer_auth(&token).json(&json!({"slug":"media","data":{"hero":format!("/api/files/{file_id}"),"gallery":[format!("/api/files/{file_id}/thumbnail")]}}))
        .send().await.unwrap();
    assert_eq!(response.status(), 201, "duplicate file references must validate");
    let entry: Value = response.json().await.unwrap();
    let query = format!(
        r#"{{ site(id: "{site_id}") {{ fileReferences(fileId: "{file_id}") {{ entryId collectionName fieldName }} }} }}"#
    );
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    let refs = body["data"]["site"]["fileReferences"].as_array().unwrap();
    assert_eq!(refs.len(), 2);
    assert!(refs.iter().all(|reference| reference["entryId"] == entry["id"]));
    let mut fields = refs
        .iter()
        .map(|reference| reference["fieldName"].as_str().unwrap())
        .collect::<Vec<_>>();
    fields.sort();
    assert_eq!(fields, ["gallery[0]", "hero"]);
    assert_eq!(entry["collection_id"], collection["id"]);
}

#[tokio::test]
async fn test_delete_file_mutation() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let file_id = upload_file_via_rest(&server, &token, &site_id).await;

    let query = format!(r#"mutation {{ deleteFile(siteId: "{site_id}", id: "{}") }}"#, file_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert!(body["data"]["deleteFile"].as_bool().unwrap());
}

#[tokio::test]
async fn test_restore_file_mutation() {
    let server = TestServer::start().await;
    let (site_id, token) = setup(&server).await;

    let file_id = upload_file_via_rest(&server, &token, &site_id).await;

    let del_body = gql(
        &server,
        &token,
        &format!(r#"mutation {{ deleteFile(siteId: "{site_id}", id: "{}") }}"#, file_id),
    )
    .await;
    assert!(
        del_body["errors"].is_null(),
        "deleteFile should succeed: {:?}",
        del_body["errors"]
    );

    let query = format!(r#"mutation {{ restoreFile(siteId: "{site_id}", id: "{}") }}"#, file_id);
    let body = gql(&server, &token, &query).await;
    assert!(body["errors"].is_null());
    assert!(body["data"]["restoreFile"].as_bool().unwrap());
}
