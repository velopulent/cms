use serde_json::{Value, json};

use crate::common::{TestServer, auth::auth_header};

// ── helpers ──

async fn login(server: &TestServer, name: &str, password: &str) -> (String, String) {
    let client = reqwest::Client::builder().build().unwrap();
    // Login is by email; map the logical name to the email these helpers assign.
    let email = if name == "admin" {
        "admin@cms.local".to_string()
    } else {
        format!("{name}@example.com")
    };
    let resp = server.login_user(&client, &email, password).await;
    let mut token = String::new();
    let mut csrf = String::new();
    for cookie in resp.headers().get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if let Some(v) = val.split(';').next().and_then(|c| c.strip_prefix("token=")) {
                token = v.to_string();
            }
            if let Some(v) = val.split(';').next().and_then(|c| c.strip_prefix("csrf=")) {
                csrf = v.to_string();
            }
        }
    }
    (token, csrf)
}

async fn create_site(server: &TestServer, token: &str, csrf: &str) -> String {
    create_site_named(server, token, csrf, "Backup Site").await
}

async fn create_site_named(server: &TestServer, token: &str, csrf: &str, name: &str) -> String {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .headers(auth_header(token, csrf))
        .json(&json!({"name": name, "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: Value = resp.json().await.unwrap();
    site["id"].as_str().unwrap().to_string()
}

async fn create_collection(server: &TestServer, token: &str, csrf: &str, site_id: &str) -> String {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(token, csrf))
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

async fn create_entry(server: &TestServer, token: &str, csrf: &str, site_id: &str, collection_id: &str) -> String {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(token, csrf))
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

async fn get_entry_status(server: &TestServer, token: &str, csrf: &str, site_id: &str, entry_id: &str) -> u16 {
    let client = reqwest::Client::new();
    client
        .get(format!(
            "{}/api/dashboard/sites/{}/entries/{}",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(token, csrf))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

async fn delete_entry(server: &TestServer, token: &str, csrf: &str, site_id: &str, entry_id: &str) {
    let client = reqwest::Client::new();
    client
        .delete(format!(
            "{}/api/dashboard/sites/{}/entries/{}",
            server.base_url, site_id, entry_id
        ))
        .headers(auth_header(token, csrf))
        .send()
        .await
        .unwrap();
}

// ── tests ──

#[tokio::test]
async fn site_backup_and_restore_round_trip() {
    let server = TestServer::start().await;
    let (token, csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &token, &csrf).await;
    let col_id = create_collection(&server, &token, &csrf, &site_id).await;
    let entry_id = create_entry(&server, &token, &csrf, &site_id, &col_id).await;

    // Create a site backup.
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/backups", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"encrypt": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "create backup");
    let backup: Value = resp.json().await.unwrap();
    let backup_id = backup["id"].as_str().unwrap().to_string();
    assert_eq!(backup["status"], "success");

    // Delete the entry, confirm it's gone.
    delete_entry(&server, &token, &csrf, &site_id, &entry_id).await;
    assert_eq!(get_entry_status(&server, &token, &csrf, &site_id, &entry_id).await, 404);

    // Restore the site backup in place.
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/restore", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"backup_id": backup_id, "confirm": "RESTORE"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204, "restore site");

    // The entry is back.
    assert_eq!(get_entry_status(&server, &token, &csrf, &site_id, &entry_id).await, 200);
}

#[tokio::test]
async fn restore_requires_confirmation() {
    let server = TestServer::start().await;
    let (token, csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &token, &csrf).await;
    let col_id = create_collection(&server, &token, &csrf, &site_id).await;
    let _ = create_entry(&server, &token, &csrf, &site_id, &col_id).await;

    let client = reqwest::Client::new();
    let backup: Value = client
        .post(format!("{}/api/dashboard/sites/{}/backups", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
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
        .headers(auth_header(&token, &csrf))
        .json(&json!({"backup_id": backup_id}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400, "restore without confirm must be rejected");
}

#[tokio::test]
async fn encrypted_backup_round_trips() {
    let server = TestServer::start().await;
    let (token, csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &token, &csrf).await;
    let col_id = create_collection(&server, &token, &csrf, &site_id).await;
    let entry_id = create_entry(&server, &token, &csrf, &site_id, &col_id).await;

    let client = reqwest::Client::new();
    let backup: Value = client
        .post(format!("{}/api/dashboard/sites/{}/backups", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"encrypt": true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(backup["encrypted"], true);
    let backup_id = backup["id"].as_str().unwrap();

    delete_entry(&server, &token, &csrf, &site_id, &entry_id).await;

    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/restore", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"backup_id": backup_id, "confirm": "RESTORE"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    assert_eq!(get_entry_status(&server, &token, &csrf, &site_id, &entry_id).await, 200);
}

async fn create_user(server: &TestServer, token: &str, csrf: &str, name: &str, instance_role: Option<&str>) {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/dashboard/instance/users", server.base_url))
        .headers(auth_header(token, csrf))
        .json(&json!({
            "name": name,
            "email": format!("{name}@example.com"),
            "temporary_password": "password123",
            "instance_role": instance_role,
        }))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "create user {name}: {}", resp.status());
}

async fn invite_member(server: &TestServer, token: &str, csrf: &str, site_id: &str, name: &str, role: &str) {
    let client = reqwest::Client::new();
    // Members are invited by email; map the logical name to its assigned email.
    let email = format!("{name}@example.com");
    let resp = client
        .post(format!("{}/api/dashboard/sites/{}/members", server.base_url, site_id))
        .headers(auth_header(token, csrf))
        .json(&json!({"email": email, "role": role}))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "invite member: {}", resp.status());
}

async fn post_status(server: &TestServer, token: &str, csrf: &str, path: &str) -> u16 {
    let client = reqwest::Client::new();
    client
        .post(format!("{}{}", server.base_url, path))
        .headers(auth_header(token, csrf))
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
    let (token, csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &token, &csrf).await;
    let client = reqwest::Client::new();

    // A schedule that keeps only the most recent 1 backup.
    let sched: Value = client
        .post(format!(
            "{}/api/dashboard/sites/{}/backup-schedules",
            server.base_url, site_id
        ))
        .headers(auth_header(&token, &csrf))
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
            .headers(auth_header(&token, &csrf))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 201, "run schedule");
    }

    let backups: Vec<Value> = client
        .get(format!("{}/api/dashboard/sites/{}/backups", server.base_url, site_id))
        .headers(auth_header(&token, &csrf))
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
    let (owner_token, owner_csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &owner_token, &owner_csrf).await;

    // An instance admin and a plain user who becomes a site editor.
    create_user(&server, &owner_token, &owner_csrf, "adminuser", Some("instance_admin")).await;
    create_user(&server, &owner_token, &owner_csrf, "editoruser", None).await;
    invite_member(&server, &owner_token, &owner_csrf, &site_id, "editoruser", "editor").await;

    // instance_admin: denied instance backup (owner-only), allowed site backup.
    let (admin_token, admin_csrf) = login(&server, "adminuser", "password123").await;
    assert_eq!(
        post_status(&server, &admin_token, &admin_csrf, "/api/dashboard/instance/backups").await,
        403,
        "admin must not create instance backups"
    );
    assert_eq!(
        post_status(
            &server,
            &admin_token,
            &admin_csrf,
            &format!("/api/dashboard/sites/{site_id}/backups")
        )
        .await,
        201,
        "admin may create site backups"
    );

    // editor: denied both site and instance backups.
    let (ed_token, ed_csrf) = login(&server, "editoruser", "password123").await;
    assert_eq!(
        post_status(
            &server,
            &ed_token,
            &ed_csrf,
            &format!("/api/dashboard/sites/{site_id}/backups")
        )
        .await,
        403,
        "editor must not create site backups"
    );
    assert_eq!(
        post_status(&server, &ed_token, &ed_csrf, "/api/dashboard/instance/backups").await,
        403,
        "editor must not create instance backups"
    );
}

#[tokio::test]
async fn inspect_lists_sites_and_multi_site_restore_round_trips() {
    let server = TestServer::start().await;
    let (token, csrf) = login(&server, "admin", "admin").await;
    let client = reqwest::Client::new();

    // Two sites, each with an entry.
    let site_a = create_site_named(&server, &token, &csrf, "Alpha").await;
    let col_a = create_collection(&server, &token, &csrf, &site_a).await;
    let entry_a = create_entry(&server, &token, &csrf, &site_a, &col_a).await;

    let site_b = create_site_named(&server, &token, &csrf, "Bravo").await;
    let col_b = create_collection(&server, &token, &csrf, &site_b).await;
    let entry_b = create_entry(&server, &token, &csrf, &site_b, &col_b).await;

    // One instance backup that captures both sites.
    let backup: Value = client
        .post(format!("{}/api/dashboard/instance/backups", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let backup_id = backup["id"].as_str().unwrap().to_string();

    // Inspect lists both sites (with names).
    let inspected: Value = client
        .post(format!("{}/api/dashboard/instance/restore/inspect", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"backup_id": backup_id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(inspected["scope"], "instance");
    let sites = inspected["sites"].as_array().unwrap();
    let ids: Vec<&str> = sites.iter().filter_map(|s| s["id"].as_str()).collect();
    assert!(
        ids.contains(&site_a.as_str()) && ids.contains(&site_b.as_str()),
        "both sites listed"
    );
    let names: Vec<&str> = sites.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(
        names.contains(&"Alpha") && names.contains(&"Bravo"),
        "site names surfaced"
    );

    // Wipe both entries.
    delete_entry(&server, &token, &csrf, &site_a, &entry_a).await;
    delete_entry(&server, &token, &csrf, &site_b, &entry_b).await;
    assert_eq!(get_entry_status(&server, &token, &csrf, &site_a, &entry_a).await, 404);
    assert_eq!(get_entry_status(&server, &token, &csrf, &site_b, &entry_b).await, 404);

    // Restore both selected sites in one call.
    let resp = client
        .post(format!("{}/api/dashboard/instance/restore", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({
            "backup_id": backup_id,
            "mode": "site",
            "site_ids": [site_a, site_b],
            "confirm": "RESTORE",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204, "multi-site restore");

    assert_eq!(get_entry_status(&server, &token, &csrf, &site_a, &entry_a).await, 200);
    assert_eq!(get_entry_status(&server, &token, &csrf, &site_b, &entry_b).await, 200);
}

#[tokio::test]
async fn multi_site_restore_with_bad_id_is_atomic() {
    let server = TestServer::start().await;
    let (token, csrf) = login(&server, "admin", "admin").await;
    let client = reqwest::Client::new();

    let site_id = create_site_named(&server, &token, &csrf, "Alpha").await;
    let col_id = create_collection(&server, &token, &csrf, &site_id).await;
    let entry_id = create_entry(&server, &token, &csrf, &site_id, &col_id).await;

    let backup: Value = client
        .post(format!("{}/api/dashboard/instance/backups", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let backup_id = backup["id"].as_str().unwrap().to_string();

    delete_entry(&server, &token, &csrf, &site_id, &entry_id).await;

    // A valid id plus a bogus one: the whole restore must fail, nothing written.
    let resp = client
        .post(format!("{}/api/dashboard/instance/restore", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({
            "backup_id": backup_id,
            "mode": "site",
            "site_ids": [site_id, "00000000-0000-0000-0000-000000000000"],
            "confirm": "RESTORE",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400, "bad site id rejects the whole restore");
    // The valid site was NOT partially restored.
    assert_eq!(get_entry_status(&server, &token, &csrf, &site_id, &entry_id).await, 404);
}

#[tokio::test]
async fn instance_backup_and_restore_round_trip() {
    let server = TestServer::start().await;
    let (token, csrf) = login(&server, "admin", "admin").await;
    let site_id = create_site(&server, &token, &csrf).await;
    let col_id = create_collection(&server, &token, &csrf, &site_id).await;
    let entry_id = create_entry(&server, &token, &csrf, &site_id, &col_id).await;

    let client = reqwest::Client::new();
    let backup: Value = client
        .post(format!("{}/api/dashboard/instance/backups", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let backup_id = backup["id"].as_str().unwrap().to_string();
    assert_eq!(backup["scope"], "instance");

    delete_entry(&server, &token, &csrf, &site_id, &entry_id).await;

    let resp = client
        .post(format!("{}/api/dashboard/instance/restore", server.base_url))
        .headers(auth_header(&token, &csrf))
        .json(&json!({"backup_id": backup_id, "mode": "instance", "confirm": "RESTORE"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204, "restore instance");

    // Instance restore wipes sessions; re-login, then verify the data is back.
    let (token2, csrf2) = login(&server, "admin", "admin").await;
    assert_eq!(
        get_entry_status(&server, &token2, &csrf2, &site_id, &entry_id).await,
        200
    );
}
