use serde_json::{Value, json};

use crate::common::{TestServer, auth::extract_cookies};

async fn gql(server: &TestServer, token: Option<&str>, query: &str) -> reqwest::Response {
    let client = reqwest::Client::builder().build().unwrap();
    let mut req = client
        .post(format!("{}/api/graphql", server.base_url))
        .json(&json!({"query": query}));

    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }

    req.send().await.unwrap()
}

async fn setup_site_token(server: &TestServer) -> (String, String) {
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
        .json(&json!({"name": "Test Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_id))
        .header("Cookie", format!("token={}; csrf={}", token, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "GraphQL Token", "scopes": crate::common::fixtures::site_key_scopes("write")}))
        .send()
        .await
        .unwrap();
    let token_val: Value = resp.json().await.unwrap();
    let api_key = token_val["token"].as_str().unwrap().to_string();

    (site_id.to_owned(), api_key)
}

async fn setup_read_token(server: &TestServer) -> String {
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
        .json(&json!({"name": "Read Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_id))
        .header("Cookie", format!("token={}; csrf={}", token, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Read Token", "scopes": crate::common::fixtures::site_key_scopes("read")}))
        .send()
        .await
        .unwrap();
    let token_val: Value = resp.json().await.unwrap();
    token_val["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_unauthenticated_query() {
    let server = TestServer::start().await;
    let resp = gql(&server, None, "{ currentSite { id name } }").await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert!(msg.contains("authentication") || msg.contains("token"));
}

#[tokio::test]
async fn test_invalid_token() {
    let server = TestServer::start().await;
    let resp = gql(&server, Some("cms_invalid_token"), "{ currentSite { id name } }").await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["errors"].is_array());
}

#[tokio::test]
async fn test_read_token_cannot_write() {
    let server = TestServer::start().await;
    let token = setup_read_token(&server).await;

    let resp = gql(
        &server,
        Some(&token),
        r#"mutation { createEntry(siteId: "unavailable", input: {collectionId: "unavailable", slug: "test", data: {title: "Test"}}) { id } }"#,
    )
    .await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert_eq!(body["errors"][0]["extensions"]["code"], "FORBIDDEN", "{msg}");
}

#[tokio::test]
async fn test_wrong_site_token() {
    let server = TestServer::start().await;
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
        .json(&json!({"name": "Site A", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site_a: Value = resp.json().await.unwrap();
    let site_a_id = site_a["id"].as_str().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .header("Cookie", format!("token={}; csrf={}", token, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Site B", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site_b: Value = resp.json().await.unwrap();
    let site_b_id = site_b["id"].as_str().unwrap();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_a_id))
        .header("Cookie", format!("token={}; csrf={}", token, csrf))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "Token A", "scopes": crate::common::fixtures::site_key_scopes("write")}))
        .send()
        .await
        .unwrap();
    let token_val: Value = resp.json().await.unwrap();
    let token_a = token_val["token"].as_str().unwrap();

    let query = format!(r#"{{ site(id: "{}") {{ id name }} }}"#, site_b_id);
    let resp = gql(&server, Some(token_a), &query).await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["errors"].is_array());
    let msg = body["errors"][0]["message"].as_str().unwrap();
    assert_eq!(body["errors"][0]["extensions"]["code"], "FORBIDDEN", "{msg}");
}

#[tokio::test]
async fn test_graphiql_served() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .get(format!("{}/api/graphql", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/html"));
}

#[tokio::test]
async fn test_introspection_query() {
    let server = TestServer::start().await;
    let (_, token) = setup_site_token(&server).await;

    let resp = gql(&server, Some(&token), "{ __schema { types { name } } }").await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["data"]["__schema"]["types"].is_array());
    let types = body["data"]["__schema"]["types"].as_array().unwrap();
    let type_names: Vec<&str> = types.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(type_names.contains(&"Site"));
    assert!(type_names.contains(&"Collection"));
    assert!(type_names.contains(&"Entry"));
}

#[tokio::test]
async fn test_valid_read_token_query() {
    let server = TestServer::start().await;
    let token = setup_read_token(&server).await;

    let resp = gql(&server, Some(&token), "{ currentSite { id name } }").await;
    let body: Value = resp.json().await.unwrap();
    assert!(body["data"].is_object());
    assert!(body["errors"].is_null() || body["errors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_valid_write_token_mutation() {
    let server = TestServer::start().await;
    let (site_id, token) = setup_site_token(&server).await;
    let client = reqwest::Client::new();
    let login = server.login_user(&client, "admin@cms.local", "admin").await;
    let (session, csrf) = extract_cookies(&login);
    let collection: Value = client
        .post(format!("{}/api/dashboard/sites/{site_id}/collections", server.base_url))
        .headers(crate::common::auth::auth_header(&session, &csrf))
        .json(&json!({"name":"Posts","slug":"posts","definition":{"fields":[{"name":"title","type":"text"}]}}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let query = format!(
        "mutation {{ createEntry(siteId:\"{site_id}\",input:{{collectionId:\"{}\",slug:\"test\",data:{{title:\"Test\"}}}}) {{ id data }} }}",
        collection["id"].as_str().unwrap()
    );
    let body: Value = gql(&server, Some(&token), &query).await.json().await.unwrap();
    assert!(body["errors"].is_null(), "{body}");
    assert_eq!(body["data"]["createEntry"]["data"]["title"], "Test");
}

#[tokio::test]
async fn test_viewer_personal_token_cannot_write() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let admin_login = server.login_user(&client, "admin@cms.local", "admin").await;
    let (admin_token, admin_csrf) = extract_cookies(&admin_login);
    let admin_headers = crate::common::auth::auth_header(&admin_token, &admin_csrf);

    let site_response = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .headers(admin_headers.clone())
        .json(&json!({"name": "Viewer PAT Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    assert_eq!(site_response.status(), 201);
    let site: Value = site_response.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap();

    let user_response = client
        .post(format!("{}/api/dashboard/instance/users", server.base_url))
        .headers(admin_headers.clone())
        .json(&json!({
            "name": "GraphQL Viewer",
            "email": "graphql-viewer@example.com",
            "temporary_password": "password123",
            "instance_role": null
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(user_response.status(), 201);

    let invite_response = client
        .post(format!("{}/api/dashboard/sites/{}/members", server.base_url, site_id))
        .headers(admin_headers)
        .json(&json!({"email": "graphql-viewer@example.com", "role": "viewer"}))
        .send()
        .await
        .unwrap();
    assert_eq!(invite_response.status(), 201);

    let viewer_login = server
        .login_user(&client, "graphql-viewer@example.com", "password123")
        .await;
    let (viewer_token, viewer_csrf) = extract_cookies(&viewer_login);
    let personal_token_response = client
        .post(format!("{}/api/dashboard/account/tokens", server.base_url))
        .headers(crate::common::auth::auth_header(&viewer_token, &viewer_csrf))
        .json(&json!({
            "name": "Viewer write-scoped token",
            "scopes": [
                "site.read",
                "content.read",
                "content.write"
            ],
            "expires_at": null
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(personal_token_response.status(), 201);
    let personal_token: Value = personal_token_response.json().await.unwrap();
    let personal_token = personal_token["token"].as_str().unwrap();

    let response = client
        .post(format!("{}/api/graphql", server.base_url))
        .header("Authorization", format!("Bearer {personal_token}"))
        .header("X-VCMS-Site", site_id)
        .json(&json!({
            "query": format!("mutation {{ createEntry(siteId:\"{site_id}\",input:{{collectionId:\"unavailable\",slug:\"forbidden\",data:{{title:\"Test\"}}}}) {{ id }} }}")
        }))
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();

    assert!(
        body["errors"].is_array(),
        "viewer PAT unexpectedly wrote through GraphQL: {body}"
    );
    assert_eq!(body["errors"][0]["extensions"]["code"], "FORBIDDEN");
}
