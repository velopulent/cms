use serde_json::{Value, json};

use crate::common::{
    TestServer,
    auth::{auth_header, extract_cookies},
};

fn create_payload(name: &str, bucket: &str) -> Value {
    json!({
        "name": name,
        "endpoint": "https://s3.example.com",
        "region": "auto",
        "bucket": bucket,
        "public_url": null,
        "access_key_id": "test-access-key",
        "secret_access_key": "test-secret-key"
    })
}

fn update_payload(name: &str, bucket: &str) -> Value {
    json!({
        "name": name,
        "endpoint": "https://s3.example.com",
        "region": "auto",
        "bucket": bucket,
        "public_url": null,
        "enabled": true,
        "access_key_id": null,
        "secret_access_key": null
    })
}

async fn admin_client(server: &TestServer) -> (reqwest::Client, reqwest::header::HeaderMap) {
    let client = reqwest::Client::new();
    let login = server.login_user(&client, "admin@cms.local", "admin").await;
    let (token, csrf) = extract_cookies(&login);
    (client, auth_header(&token, &csrf))
}

#[tokio::test]
async fn create_does_not_rebuild_unrelated_registry_entries() {
    let server = TestServer::start().await;
    let (client, headers) = admin_client(&server).await;
    let response = client
        .post(format!("{}/api/dashboard/instance/storage-profiles", server.base_url))
        .headers(headers.clone())
        .json(&create_payload("Created Profile", "created-profile-bucket"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let duplicate = client
        .post(format!("{}/api/dashboard/instance/storage-profiles", server.base_url))
        .headers(headers)
        .json(&create_payload("Created Profile", "another-profile-bucket"))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), 409);
    let body: Value = duplicate.json().await.unwrap();
    assert_eq!(body["error"], "A storage profile with this name already exists");
}

#[tokio::test]
async fn update_does_not_remove_active_provider_before_replacement() {
    let server = TestServer::start().await;
    let (client, headers) = admin_client(&server).await;
    let collection_url = format!("{}/api/dashboard/instance/storage-profiles", server.base_url);

    let create = client
        .post(&collection_url)
        .headers(headers.clone())
        .json(&create_payload("Original Profile", "original-profile-bucket"))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let profile: Value = create.json().await.unwrap();
    let profile_id = profile["id"].as_str().unwrap();

    let response = client
        .put(format!("{collection_url}/{profile_id}"))
        .headers(headers.clone())
        .json(&update_payload("Updated Profile", "updated-profile-bucket"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let profiles: Vec<Value> = client
        .get(&collection_url)
        .headers(headers)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let updated = profiles
        .iter()
        .find(|candidate| candidate["id"] == profile_id)
        .expect("updated profile disappeared");
    assert_eq!(updated["name"], "Updated Profile");
    assert_eq!(updated["bucket"], "updated-profile-bucket");
}

#[tokio::test]
async fn site_creation_assigns_profile_atomically() {
    let server = TestServer::start().await;
    let (client, headers) = admin_client(&server).await;
    let sites_url = format!("{}/api/dashboard/sites", server.base_url);

    let before: Vec<Value> = client
        .get(&sites_url)
        .headers(headers.clone())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let rejected = client
        .post(&sites_url)
        .headers(headers.clone())
        .json(&json!({
            "name": "Must Roll Back",
            "storage_provider": "filesystem",
            "storage_profile_id": "missing-profile"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 400);
    let after: Vec<Value> = client
        .get(&sites_url)
        .headers(headers.clone())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after.len(), before.len(), "failed profile assignment inserted a site");

    let profile_response = client
        .post(format!("{}/api/dashboard/instance/storage-profiles", server.base_url))
        .headers(headers.clone())
        .json(&create_payload("Site Profile", "site-profile-bucket"))
        .send()
        .await
        .unwrap();
    assert_eq!(profile_response.status(), 201);
    let profile: Value = profile_response.json().await.unwrap();
    let profile_id = profile["id"].as_str().unwrap();

    let created = client
        .post(&sites_url)
        .headers(headers)
        .json(&json!({
            "name": "Profile-backed Site",
            "storage_provider": "filesystem",
            "storage_profile_id": profile_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let site: Value = created.json().await.unwrap();
    assert_eq!(site["storage_provider"], "s3");
    assert_eq!(site["storage_profile_id"], profile_id);
}

#[tokio::test]
async fn live_site_prevents_profile_deletion() {
    let server = TestServer::start().await;
    let (client, headers) = admin_client(&server).await;
    let profiles_url = format!("{}/api/dashboard/instance/storage-profiles", server.base_url);
    let created = client
        .post(&profiles_url)
        .headers(headers.clone())
        .json(&create_payload("Live Site Profile", "live-site-profile-bucket"))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let profile: Value = created.json().await.unwrap();
    let profile_id = profile["id"].as_str().unwrap();

    let site = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .headers(headers.clone())
        .json(&json!({
            "name": "Profile-backed site",
            "storage_profile_id": profile_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(site.status(), 201);

    let response = client
        .delete(format!("{profiles_url}/{profile_id}"))
        .headers(headers)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"], "Storage profile is in use");
}
