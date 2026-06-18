use serde_json::{Value, json};

use crate::common::{TestServer, auth::auth_header};

// ── helpers ──

async fn login(server: &TestServer, username: &str, password: &str) -> (String, String) {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = server.login_user(&client, username, password).await;
    assert_eq!(resp.status(), 200, "login failed for {username}");
    let mut jwt = String::new();
    let mut csrf = String::new();
    for cookie in resp.headers().get_all("set-cookie").iter() {
        if let Ok(val) = cookie.to_str() {
            if let Some(rest) = val.strip_prefix("token=") {
                jwt = rest.split(';').next().unwrap_or("").to_string();
            }
            if let Some(rest) = val.strip_prefix("csrf=") {
                csrf = rest.split(';').next().unwrap_or("").to_string();
            }
        }
    }
    (jwt, csrf)
}

async fn create_site(server: &TestServer, jwt: &str, csrf: &str, name: &str) -> String {
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .headers(auth_header(jwt, csrf))
        .json(&json!({"name": name, "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "create site failed: {}", resp.status());
    let site: Value = resp.json().await.unwrap();
    site["id"].as_str().unwrap().to_string()
}

/// Create a managed user. Returns the new user's id. `instance_role` is one of
/// `Some("instance_owner")`, `Some("instance_admin")`, or `None`.
async fn create_user(
    server: &TestServer,
    jwt: &str,
    csrf: &str,
    username: &str,
    instance_role: Option<&str>,
) -> reqwest::Response {
    let client = reqwest::Client::builder().build().unwrap();
    client
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
        .unwrap()
}

async fn invite_member(
    server: &TestServer,
    jwt: &str,
    csrf: &str,
    site_id: &str,
    username: &str,
    role: &str,
) -> reqwest::Response {
    let client = reqwest::Client::builder().build().unwrap();
    client
        .post(format!("{}/api/dashboard/sites/{}/members", server.base_url, site_id))
        .headers(auth_header(jwt, csrf))
        .json(&json!({"username": username, "role": role}))
        .send()
        .await
        .unwrap()
}

async fn update_member_role(
    server: &TestServer,
    jwt: &str,
    csrf: &str,
    site_id: &str,
    member_user_id: &str,
    role: &str,
) -> reqwest::Response {
    let client = reqwest::Client::builder().build().unwrap();
    client
        .put(format!(
            "{}/api/dashboard/sites/{}/members/{}",
            server.base_url, site_id, member_user_id
        ))
        .headers(auth_header(jwt, csrf))
        .json(&json!({"role": role}))
        .send()
        .await
        .unwrap()
}

async fn remove_member(
    server: &TestServer,
    jwt: &str,
    csrf: &str,
    site_id: &str,
    member_user_id: &str,
) -> reqwest::Response {
    let client = reqwest::Client::builder().build().unwrap();
    client
        .delete(format!(
            "{}/api/dashboard/sites/{}/members/{}",
            server.base_url, site_id, member_user_id
        ))
        .headers(auth_header(jwt, csrf))
        .send()
        .await
        .unwrap()
}

async fn create_collection(server: &TestServer, jwt: &str, csrf: &str, site_id: &str, slug: &str) -> reqwest::Response {
    let client = reqwest::Client::builder().build().unwrap();
    client
        .post(format!(
            "{}/api/dashboard/sites/{}/collections",
            server.base_url, site_id
        ))
        .headers(auth_header(jwt, csrf))
        .json(&json!({
            "name": slug,
            "slug": slug,
            "definition": {"fields": [{"name": "title", "type": "text", "required": true}]}
        }))
        .send()
        .await
        .unwrap()
}

async fn get_site(server: &TestServer, jwt: &str, csrf: &str, site_id: &str) -> reqwest::Response {
    let client = reqwest::Client::builder().build().unwrap();
    client
        .get(format!("{}/api/dashboard/sites/{}", server.base_url, site_id))
        .headers(auth_header(jwt, csrf))
        .send()
        .await
        .unwrap()
}

async fn create_entry(
    server: &TestServer,
    jwt: &str,
    csrf: &str,
    site_id: &str,
    collection_id: &str,
    slug: &str,
) -> reqwest::Response {
    let client = reqwest::Client::builder().build().unwrap();
    client
        .post(format!("{}/api/dashboard/sites/{}/entries", server.base_url, site_id))
        .headers(auth_header(jwt, csrf))
        .json(&json!({"collection_id": collection_id, "slug": slug, "data": {"title": "Hello"}}))
        .send()
        .await
        .unwrap()
}

// ── instance operator (admin) override: manages any site without membership ──

#[tokio::test]
async fn instance_admin_manages_site_without_membership() {
    let server = TestServer::start().await;
    let (owner_jwt, owner_csrf) = login(&server, "admin", "admin").await;

    // Owner creates an instance_admin operator and a site the operator is NOT a member of.
    let resp = create_user(&server, &owner_jwt, &owner_csrf, "operator", Some("instance_admin")).await;
    assert_eq!(resp.status(), 201);
    let site_id = create_site(&server, &owner_jwt, &owner_csrf, "Owner Site").await;

    let (op_jwt, op_csrf) = login(&server, "operator", "password123").await;

    // Override lets the operator read the site and manage its schema.
    assert_eq!(get_site(&server, &op_jwt, &op_csrf, &site_id).await.status(), 200);
    assert!(
        create_collection(&server, &op_jwt, &op_csrf, &site_id, "posts")
            .await
            .status()
            .is_success()
    );
}

// ── editor: scoped to its site, content only ──

#[tokio::test]
async fn editor_writes_content_but_not_schema_or_members() {
    let server = TestServer::start().await;
    let (admin_jwt, admin_csrf) = login(&server, "admin", "admin").await;

    let site_id = create_site(&server, &admin_jwt, &admin_csrf, "Editor Site").await;
    let other_site = create_site(&server, &admin_jwt, &admin_csrf, "Other Site").await;
    let col: Value = create_collection(&server, &admin_jwt, &admin_csrf, &site_id, "posts")
        .await
        .json()
        .await
        .unwrap();
    let col_id = col["id"].as_str().unwrap();

    assert_eq!(
        create_user(&server, &admin_jwt, &admin_csrf, "editor1", None)
            .await
            .status(),
        201
    );
    assert_eq!(
        invite_member(&server, &admin_jwt, &admin_csrf, &site_id, "editor1", "editor")
            .await
            .status(),
        201
    );

    let (ed_jwt, ed_csrf) = login(&server, "editor1", "password123").await;

    // Can read the site and write content.
    assert_eq!(get_site(&server, &ed_jwt, &ed_csrf, &site_id).await.status(), 200);
    assert!(
        create_entry(&server, &ed_jwt, &ed_csrf, &site_id, col_id, "first")
            .await
            .status()
            .is_success()
    );

    // Cannot manage schema or members on its own site.
    assert_eq!(
        create_collection(&server, &ed_jwt, &ed_csrf, &site_id, "extra")
            .await
            .status(),
        403
    );
    assert_eq!(
        invite_member(&server, &ed_jwt, &ed_csrf, &site_id, "admin", "viewer")
            .await
            .status(),
        403
    );

    // Cannot touch a site it is not a member of.
    assert_eq!(get_site(&server, &ed_jwt, &ed_csrf, &other_site).await.status(), 404);
}

// ── viewer: read-only ──

#[tokio::test]
async fn viewer_is_read_only() {
    let server = TestServer::start().await;
    let (admin_jwt, admin_csrf) = login(&server, "admin", "admin").await;

    let site_id = create_site(&server, &admin_jwt, &admin_csrf, "Viewer Site").await;
    let col: Value = create_collection(&server, &admin_jwt, &admin_csrf, &site_id, "posts")
        .await
        .json()
        .await
        .unwrap();
    let col_id = col["id"].as_str().unwrap();

    assert_eq!(
        create_user(&server, &admin_jwt, &admin_csrf, "viewer1", None)
            .await
            .status(),
        201
    );
    assert_eq!(
        invite_member(&server, &admin_jwt, &admin_csrf, &site_id, "viewer1", "viewer")
            .await
            .status(),
        201
    );

    let (v_jwt, v_csrf) = login(&server, "viewer1", "password123").await;
    assert_eq!(get_site(&server, &v_jwt, &v_csrf, &site_id).await.status(), 200);
    assert_eq!(
        create_entry(&server, &v_jwt, &v_csrf, &site_id, col_id, "nope")
            .await
            .status(),
        403
    );
}

// ── only an instance owner may grant the owner role ──

#[tokio::test]
async fn only_owner_grants_instance_owner() {
    let server = TestServer::start().await;
    let (owner_jwt, owner_csrf) = login(&server, "admin", "admin").await;

    assert_eq!(
        create_user(&server, &owner_jwt, &owner_csrf, "admin2", Some("instance_admin"))
            .await
            .status(),
        201
    );
    let (a2_jwt, a2_csrf) = login(&server, "admin2", "password123").await;

    // An instance_admin can create another admin...
    assert_eq!(
        create_user(&server, &a2_jwt, &a2_csrf, "admin3", Some("instance_admin"))
            .await
            .status(),
        201
    );
    // ...but cannot mint an instance_owner.
    assert_eq!(
        create_user(&server, &a2_jwt, &a2_csrf, "wouldbeowner", Some("instance_owner"))
            .await
            .status(),
        403
    );
}

// ── operator can update and remove a member (PUT/DELETE /members/{member_user_id}) ──

#[tokio::test]
async fn operator_updates_and_removes_member() {
    let server = TestServer::start().await;
    let (admin_jwt, admin_csrf) = login(&server, "admin", "admin").await;

    let site_id = create_site(&server, &admin_jwt, &admin_csrf, "Members Site").await;
    assert_eq!(
        create_user(&server, &admin_jwt, &admin_csrf, "member1", None)
            .await
            .status(),
        201
    );

    let member: Value = invite_member(&server, &admin_jwt, &admin_csrf, &site_id, "member1", "editor")
        .await
        .json()
        .await
        .unwrap();
    let member_user_id = member["user_id"].as_str().unwrap();

    // Update the member's role (the route path param is `member_user_id`).
    let resp = update_member_role(&server, &admin_jwt, &admin_csrf, &site_id, member_user_id, "viewer").await;
    assert_eq!(resp.status(), 200, "update member role failed");
    let updated: Value = resp.json().await.unwrap();
    assert_eq!(updated["role"].as_str(), Some("viewer"));

    // Remove the member.
    assert_eq!(
        remove_member(&server, &admin_jwt, &admin_csrf, &site_id, member_user_id)
            .await
            .status(),
        204
    );

    // Removing again now returns 404.
    assert_eq!(
        remove_member(&server, &admin_jwt, &admin_csrf, &site_id, member_user_id)
            .await
            .status(),
        404
    );
}

// ── instance operators cannot be added as site members ──

#[tokio::test]
async fn cannot_invite_operator_as_member() {
    let server = TestServer::start().await;
    let (admin_jwt, admin_csrf) = login(&server, "admin", "admin").await;

    let site_id = create_site(&server, &admin_jwt, &admin_csrf, "Guarded Site").await;
    assert_eq!(
        create_user(&server, &admin_jwt, &admin_csrf, "operator2", Some("instance_admin"))
            .await
            .status(),
        201
    );

    // An instance operator already has full access; inviting them as a member is rejected.
    assert_eq!(
        invite_member(&server, &admin_jwt, &admin_csrf, &site_id, "operator2", "editor")
            .await
            .status(),
        400
    );
}

// ── site deletion is operator-only ──

#[tokio::test]
async fn editor_cannot_delete_site() {
    let server = TestServer::start().await;
    let (admin_jwt, admin_csrf) = login(&server, "admin", "admin").await;

    let site_id = create_site(&server, &admin_jwt, &admin_csrf, "Doomed Site").await;
    assert_eq!(
        create_user(&server, &admin_jwt, &admin_csrf, "editor2", None)
            .await
            .status(),
        201
    );
    assert_eq!(
        invite_member(&server, &admin_jwt, &admin_csrf, &site_id, "editor2", "editor")
            .await
            .status(),
        201
    );

    let (ed_jwt, ed_csrf) = login(&server, "editor2", "password123").await;
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .delete(format!("{}/api/dashboard/sites/{}", server.base_url, site_id))
        .headers(auth_header(&ed_jwt, &ed_csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}
