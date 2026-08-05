//! Higher-level fixture builders layered on the auth/client helpers.

use serde_json::json;

use super::auth::{auth_header, extract_cookies};
use super::client::http_client;
use super::server::TestServer;

/// Log in as the seeded admin and create a filesystem-backed "Test Site".
/// Returns `(token, csrf, site_id)` — the common starting point for dashboard tests.
pub async fn setup(server: &TestServer) -> (String, String, String) {
    let client = http_client();
    let resp = server.login_user(&client, "admin@cms.local", "admin").await;
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

/// Test-only scope presets for scoped machine site keys.
pub fn site_key_scopes(access: &str) -> Vec<&'static str> {
    let mut scopes = vec![
        "site.read",
        "site.settings.read",
        "content.read",
        "files.read",
        "schema.read",
        "webhooks.read",
        "deployments.read",
        "mcp.use",
    ];
    if access == "write" {
        scopes.extend([
            "site.settings.write",
            "content.write",
            "files.write",
            "schema.write",
            "webhooks.write",
            "webhooks.trigger",
            "deployments.write",
            "deployments.trigger",
        ]);
    }
    scopes
}

/// Log in as admin, create a site, and mint a scoped machine site key.
/// Returns `(site_id, token)`.
pub async fn create_site_and_token(server: &TestServer, access: &str) -> (String, String) {
    let client = http_client();
    let resp = server.login_user(&client, "admin@cms.local", "admin").await;
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
        .json(&json!({"name": "Test Token", "scopes": site_key_scopes(access)}))
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
