//! Higher-level fixture builders layered on the auth/client helpers.

use serde_json::json;

use super::auth::{auth_header, extract_cookies};
use super::client::http_client;
use super::server::TestServer;

/// Log in as the seeded admin and create a filesystem-backed "Test Site".
/// Returns `(token, csrf, site_id)` — the common starting point for dashboard tests.
pub async fn setup(server: &TestServer) -> (String, String, String) {
    let client = http_client();
    let resp = server.login_user(&client, "admin", "admin").await;
    let (token, csrf) = extract_cookies(&resp);

    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"name": "Test Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: serde_json::Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap().to_string();

    (token, csrf, site_id)
}

/// Log in as admin, create a site, and mint a site access token with the given
/// `permission` (`"read"` or `"write"`). Returns `(site_id, token)`.
pub async fn create_site_and_token(server: &TestServer, permission: &str) -> (String, String) {
    let client = http_client();
    let resp = server.login_user(&client, "admin", "admin").await;
    let (token, csrf) = extract_cookies(&resp);

    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"name": "Test Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "Create site failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    let site: serde_json::Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap().to_string();

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/tokens", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"name": "Test Token", "permission": permission}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "Create token failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    let token_val: serde_json::Value = resp.json().await.unwrap();
    let token = token_val["token"].as_str().unwrap().to_string();

    (site_id, token)
}
