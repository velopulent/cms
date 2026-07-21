use serde_json::{Value, json};

use crate::common::{
    TestServer,
    auth::{auth_header, extract_cookies},
};

async fn owner_session(server: &TestServer, client: &reqwest::Client) -> (String, String) {
    let response = server.login_user(client, "admin@cms.local", "admin").await;
    assert_eq!(response.status(), 200);
    extract_cookies(&response)
}

#[tokio::test]
async fn owner_can_read_redacted_settings_and_admin_cannot() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (token, csrf) = owner_session(&server, &client).await;
    let response = client
        .get(format!("{}/api/dashboard/instance/settings", server.base_url))
        .headers(auth_header(&token, &csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["general"]["upload_limit_mb"], 50);
    assert!(body["storage_credentials"]["configured"].is_boolean());
    let serialized = body.to_string();
    assert!(!serialized.contains("secret_access_key"));
    assert!(!serialized.contains("master_key"));

    let create = client
        .post(format!("{}/api/dashboard/instance/users", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({
            "name": "settings admin", "email": "settings-admin@example.com",
            "temporary_password": "password123", "instance_role": "instance_admin"
        }))
        .send()
        .await
        .unwrap();
    assert!(create.status().is_success());
    let login = server
        .login_user(&client, "settings-admin@example.com", "password123")
        .await;
    let (admin_token, admin_csrf) = extract_cookies(&login);
    let forbidden = client
        .get(format!("{}/api/dashboard/instance/settings", server.base_url))
        .headers(auth_header(&admin_token, &admin_csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), 403);
}

#[tokio::test]
async fn general_update_applies_registration_immediately() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (token, csrf) = owner_session(&server, &client).await;
    let response = client
        .put(format!("{}/api/dashboard/instance/settings/general", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({
            "public_url": null,
            "public_registration": true,
            "session_lifetime_hours": 24,
            "upload_limit_mb": 50,
            "mcp_enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let register = client
        .post(format!("{}/api/auth/register", server.base_url))
        .json(&json!({"name": "New User", "email": "new@example.com", "password": "password123"}))
        .send()
        .await
        .unwrap();
    assert_eq!(register.status(), 201);
}

#[tokio::test]
async fn settings_updates_reject_unknown_fields_and_invalid_limits() {
    let server = TestServer::start().await;
    let client = reqwest::Client::new();
    let (token, csrf) = owner_session(&server, &client).await;
    let response = client
        .put(format!("{}/api/dashboard/instance/settings/general", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({
            "public_url": null, "public_registration": false,
            "session_lifetime_hours": 24, "upload_limit_mb": 0,
            "mcp_enabled": true, "mystery": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);

    let invalid = client
        .put(format!("{}/api/dashboard/instance/settings/general", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({
            "public_url": null, "public_registration": false,
            "session_lifetime_hours": 24, "upload_limit_mb": 0,
            "mcp_enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 400);
}
