use serde_json::{Value, json};

use crate::common::TestServer;

// ── helpers ──

fn auth_header(jwt: &str, csrf: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    let cookie_val = format!("token={}; csrf={}", jwt, csrf);
    headers.insert(
        reqwest::header::COOKIE,
        reqwest::header::HeaderValue::from_str(&cookie_val).unwrap(),
    );
    headers.insert("X-CSRF-Token", reqwest::header::HeaderValue::from_str(csrf).unwrap());
    headers
}

async fn login(server: &TestServer, username: &str, password: &str) -> (String, String) {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = server.login_user(&client, username, password).await;
    let mut jwt = String::new();
    let mut csrf = String::new();
    for cookie in resp.headers().get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if let Some(v) = val.split(';').next().and_then(|c| c.strip_prefix("token=")) {
                jwt = v.to_string();
            }
            if let Some(v) = val.split(';').next().and_then(|c| c.strip_prefix("csrf=")) {
                csrf = v.to_string();
            }
        }
    }
    (jwt, csrf)
}

async fn create_site(server: &TestServer, jwt: &str, csrf: &str) -> String {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .headers(auth_header(jwt, csrf))
        .json(&json!({"name": "Backup Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: Value = resp.json().await.unwrap();
    site["id"].as_str().unwrap().to_string()
}

async fn create_collection(server: &TestServer, jwt: &str, csrf: &str, site_id: &str) -> String {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(jwt, csrf))
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

async fn create_entry(server: &TestServer, jwt: &str, csrf: &str, site_id: &str, collection_id: &str) -> String {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(jwt, csrf))
        .json(&json!({
            "collection_id": collection_id,
            "slug": "hello",
            "data": {"title": "Hello World"},
        }))
        .send()
        .await
        .unwrap();
    let entry: Value = resp.json().await.unwrap();
    entry["id"].as_str().unwrap().to_string()
}

async fn get_entry_status(server: &TestServer, jwt: &str, csrf: &str, site_id: &str, entry_id: &str) -> u16 {
    let client = reqwest::Client::new();
    client
        .get(format!(
            "{}/api/dashboard/sites/{}/entries/{}",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(jwt, csrf))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

async fn delete_entry(server: &TestServer, jwt: &str, csrf: &str, site_id: &str, entry_id: &str) {
    let client = reqwest::Client::new();
    client
        .delete(format!(
            "{}/api/dashboard/sites/{}/entries/{}",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(jwt, csrf))
        .send()
        .await
        .unwrap();
}

// ── tests ──

#[tokio::test]
async fn site_backup_and_restore_round_trip() {
    let server = TestServer::start().await;
    let (jwt, csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &jwt, &csrf).await;
    let col_id = create_collection(&server, &jwt, &csrf, &site_id).await;
    let entry_id = create_entry(&server, &jwt, &csrf, &site_id, &col_id).await;

    // Create a site backup.
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/backups", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"encrypt": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "create backup");
    let backup: Value = resp.json().await.unwrap();
    let backup_id = backup["id"].as_str().unwrap().to_string();
    assert_eq!(backup["status"], "success");

    // Delete the entry, confirm it's gone.
    delete_entry(&server, &jwt, &csrf, &site_id, &entry_id).await;
    assert_eq!(get_entry_status(&server, &jwt, &csrf, &site_id, &entry_id).await, 404);

    // Restore the site backup in place.
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/restore", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"backup_id": backup_id, "confirm": "RESTORE"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204, "restore site");

    // The entry is back.
    assert_eq!(get_entry_status(&server, &jwt, &csrf, &site_id, &entry_id).await, 200);
}

#[tokio::test]
async fn restore_requires_confirmation() {
    let server = TestServer::start().await;
    let (jwt, csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &jwt, &csrf).await;
    let col_id = create_collection(&server, &jwt, &csrf, &site_id).await;
    let _ = create_entry(&server, &jwt, &csrf, &site_id, &col_id).await;

    let client = reqwest::Client::new();
    let backup: Value = client
        .post(format!("{}/api/dashboard/sites/{}/backups", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let backup_id = backup["id"].as_str().unwrap();

    // No confirmation → rejected.
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/restore", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"backup_id": backup_id}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400, "restore without confirm must be rejected");
}

#[tokio::test]
async fn encrypted_backup_round_trips() {
    let server = TestServer::start().await;
    let (jwt, csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &jwt, &csrf).await;
    let col_id = create_collection(&server, &jwt, &csrf, &site_id).await;
    let entry_id = create_entry(&server, &jwt, &csrf, &site_id, &col_id).await;

    let client = reqwest::Client::new();
    let backup: Value = client
        .post(format!("{}/api/dashboard/sites/{}/backups", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"encrypt": true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(backup["encrypted"], true);
    let backup_id = backup["id"].as_str().unwrap();

    delete_entry(&server, &jwt, &csrf, &site_id, &entry_id).await;

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/restore", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"backup_id": backup_id, "confirm": "RESTORE"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    assert_eq!(get_entry_status(&server, &jwt, &csrf, &site_id, &entry_id).await, 200);
}

async fn create_user(server: &TestServer, jwt: &str, csrf: &str, username: &str, instance_role: Option<&str>) {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/dashboard/instance/users", server.base_url))
        .headers(auth_header(jwt, csrf))
        .json(&json!({
            "username": username,
            "email": format!("{username}@example.com"),
            "temporary_password": "password123",
            "instance_role": instance_role,
        }))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "create user {username}: {}", resp.status());
}

async fn invite_member(server: &TestServer, jwt: &str, csrf: &str, site_id: &str, username: &str, role: &str) {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/members", server.base_url, site_id))
        .headers(auth_header(jwt, csrf))
        .json(&json!({"username": username, "role": role}))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "invite member: {}", resp.status());
}

async fn post_status(server: &TestServer, jwt: &str, csrf: &str, path: &str) -> u16 {
    let client = reqwest::Client::new();
    client
        .post(format!("{}{}", server.base_url, path))
        .headers(auth_header(jwt, csrf))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

#[tokio::test]
async fn schedule_retention_prunes_old_backups() {
    let server = TestServer::start().await;
    let (jwt, csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &jwt, &csrf).await;
    let client = reqwest::Client::new();

    // A schedule that keeps only the most recent 1 backup.
    let sched: Value = client
        .post(format!(
            "{}/api/dashboard/sites/{}/backup-schedules",
            server.base_url, site_id
        ))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({
            "cron": "0 0 * * *",
            "retention_n": 1,
            "include_files": false,
            "encrypt": false,
            "enabled": true,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let sid = sched["id"].as_str().unwrap();

    // Run it twice; retention should prune the older one.
    for _ in 0..2 {
        let resp = client
            .post(format!(
                "{}/api/dashboard/sites/{}/backup-schedules/{}/run",
                server.base_url, site_id, sid
            ))
            .headers(auth_header(&jwt, &csrf))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 201, "run schedule");
    }

    let backups: Vec<Value> = client
        .get(format!("{}/api/dashboard/sites/{}/backups", server.base_url, site_id))
        .headers(auth_header(&jwt, &csrf))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(backups.len(), 1, "retention should keep only the newest backup");
}

#[tokio::test]
async fn backup_rbac_matrix() {
    let server = TestServer::start().await;
    let (owner_jwt, owner_csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &owner_jwt, &owner_csrf).await;

    // An instance admin and a plain user who becomes a site editor.
    create_user(&server, &owner_jwt, &owner_csrf, "adminuser", Some("instance_admin")).await;
    create_user(&server, &owner_jwt, &owner_csrf, "editoruser", None).await;
    invite_member(&server, &owner_jwt, &owner_csrf, &site_id, "editoruser", "editor").await;

    // instance_admin: denied instance backup (owner-only), allowed site backup.
    let (admin_jwt, admin_csrf) = login(&server, "adminuser", "password123").await;
    assert_eq!(
        post_status(&server, &admin_jwt, &admin_csrf, "/api/dashboard/instance/backups").await,
        403,
        "admin must not create instance backups"
    );
    assert_eq!(
        post_status(
            &server,
            &admin_jwt,
            &admin_csrf,
            &format!("/api/dashboard/sites/{site_id}/backups")
        )
        .await,
        201,
        "admin may create site backups"
    );

    // editor: denied both site and instance backups.
    let (ed_jwt, ed_csrf) = login(&server, "editoruser", "password123").await;
    assert_eq!(
        post_status(
            &server,
            &ed_jwt,
            &ed_csrf,
            &format!("/api/dashboard/sites/{site_id}/backups")
        )
        .await,
        403,
        "editor must not create site backups"
    );
    assert_eq!(
        post_status(&server, &ed_jwt, &ed_csrf, "/api/dashboard/instance/backups").await,
        403,
        "editor must not create instance backups"
    );
}

#[tokio::test]
async fn instance_backup_and_restore_round_trip() {
    let server = TestServer::start().await;
    let (jwt, csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &jwt, &csrf).await;
    let col_id = create_collection(&server, &jwt, &csrf, &site_id).await;
    let entry_id = create_entry(&server, &jwt, &csrf, &site_id, &col_id).await;

    let client = reqwest::Client::new();
    let backup: Value = client
        .post(format!("{}/api/dashboard/instance/backups", server.base_url))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let backup_id = backup["id"].as_str().unwrap().to_string();
    assert_eq!(backup["scope"], "instance");

    delete_entry(&server, &jwt, &csrf, &site_id, &entry_id).await;

    let resp = client
        .post(format!("{}/api/dashboard/instance/restore", server.base_url))
        .headers(auth_header(&jwt, &csrf))
        .json(&json!({"backup_id": backup_id, "mode": "instance", "confirm": "RESTORE"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204, "restore instance");

    // Instance restore wipes sessions; re-login, then verify the data is back.
    let (jwt2, csrf2) = login(&server, "admin", "admin").await;
    assert_eq!(get_entry_status(&server, &jwt2, &csrf2, &site_id, &entry_id).await, 200);
}
