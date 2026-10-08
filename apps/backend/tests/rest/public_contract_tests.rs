use crate::common::{TestServer, auth::auth_header, fixtures::setup};
use serde_json::{Value, json};

async fn token(server: &TestServer, site: &str, session: &str, csrf: &str, scopes: &[&str]) -> String {
    let response = reqwest::Client::new()
        .post(format!("{}/api/dashboard/sites/{site}/tokens", server.base_url))
        .headers(auth_header(session, csrf))
        .json(&json!({"name":"Audit", "scopes": scopes}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    response.json::<Value>().await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn public_entries_enforce_preview_publication_and_atomic_versions() {
    let server = TestServer::start().await;
    let (session, csrf, site) = setup(&server).await;
    let client = reqwest::Client::new();
    let collection = client
        .post(format!("{}/api/dashboard/sites/{site}/collections", server.base_url))
        .headers(auth_header(&session, &csrf))
        .json(&json!({"name":"Posts","slug":"posts","definition":{"fields":[{"name":"title","type":"text"}]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(collection.status(), 201);
    let writer = token(
        &server,
        &site,
        &session,
        &csrf,
        &["content.read", "content.write", "content.preview.read"],
    )
    .await;
    let reader = token(&server, &site, &session, &csrf, &["content.read"]).await;
    let base = format!("{}/api/v1/sites/{site}", server.base_url);
    let response = client
        .post(format!("{base}/collections/posts/entries"))
        .bearer_auth(&writer)
        .json(&json!({"slug":"draft","data":{"title":"Private"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let entry: Value = response.json().await.unwrap();
    let id = entry["id"].as_str().unwrap();
    for suffix in [
        format!("entries/{id}"),
        format!("entries/{id}/revisions"),
        format!("entries/{id}/revisions/1"),
    ] {
        let response = client
            .get(format!("{base}/{suffix}"))
            .bearer_auth(&reader)
            .send()
            .await
            .unwrap();
        assert!(
            matches!(response.status().as_u16(), 403 | 404),
            "{suffix}: {}",
            response.status()
        );
    }
    let response = client
        .patch(format!("{base}/entries/{id}"))
        .bearer_auth(&writer)
        .json(&json!({"status":"published"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let response = client
        .get(format!(
            "{base}/collections/posts/entries?include_drafts=true&per_page=1&include_total=true"
        ))
        .bearer_auth(&writer)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let page: Value = response.json().await.unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["page_info"]["has_next_page"], false);
    let version = format!("\"{}\"", entry["version"].as_str().unwrap());
    let update = |title: &'static str| {
        client
            .patch(format!("{base}/entries/{id}"))
            .bearer_auth(&writer)
            .header("If-Match", &version)
            .json(&json!({"data":{"title":title}}))
            .send()
    };
    let (first, second) = tokio::join!(update("First"), update("Second"));
    let mut statuses = [first.unwrap().status().as_u16(), second.unwrap().status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 412]);
    let current = client
        .get(format!("{base}/entries/{id}?include_drafts=true"))
        .bearer_auth(&writer)
        .send()
        .await
        .unwrap();
    assert_eq!(current.status(), 200);
    assert_ne!(current.headers()["etag"], version);
    let body: Value = current.json().await.unwrap();
    assert!(matches!(body["data"]["title"].as_str(), Some("First" | "Second")));
}

#[tokio::test]
async fn openapi_matches_public_entry_routes() {
    let server = TestServer::start().await;
    let doc: Value = reqwest::get(format!("{}/api/v1/openapi.json", server.base_url))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(doc["paths"]["/api/v1/sites/{site_id}/entries/{id}"]["patch"].is_object());
    assert!(doc["paths"]["/api/v1/sites/{site_id}/entries/{id}"]["delete"].is_object());
    assert!(
        doc["paths"]
            .as_object()
            .unwrap()
            .keys()
            .all(|key| !key.starts_with("/api/v1/entries"))
    );
}

#[tokio::test]
async fn public_errors_have_problem_type_and_request_correlation() {
    let server = TestServer::start().await;
    let response = reqwest::get(format!("{}/api/v1/sites", server.base_url)).await.unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(response.headers()["content-type"], "application/problem+json");
    let request_id = response.headers()["x-request-id"].to_str().unwrap().to_owned();
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], 401);
    assert_eq!(body["code"], "unauthorized");
    assert_eq!(body["request_id"], request_id);
    assert!(body["detail"].is_string());
}

#[tokio::test]
async fn active_uploaded_documents_cannot_execute_in_dashboard_origin() {
    let server = TestServer::start().await;
    let (session, csrf, site) = setup(&server).await;
    let writer = token(&server, &site, &session, &csrf, &["files.write"]).await;
    let client = reqwest::Client::new();
    for (mime, filename, content) in [
        ("text/html", "test.html", "<script>document.cookie</script>"),
        (
            "image/svg+xml",
            "test.svg",
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><script>document.cookie</script></svg>",
        ),
    ] {
        let response = client
            .post(format!("{}/api/v1/sites/{site}/files", server.base_url))
            .bearer_auth(&writer)
            .multipart(
                reqwest::multipart::Form::new().part(
                    "file",
                    reqwest::multipart::Part::text(content)
                        .file_name(filename)
                        .mime_str(mime)
                        .unwrap(),
                ),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        let file: Value = response.json().await.unwrap();
        let response = client
            .get(format!(
                "{}/api/files/{}",
                server.base_url,
                file["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert!(
            response.headers()["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("sandbox")
        );
        assert_eq!(response.text().await.unwrap(), content);
    }
}

#[tokio::test]
async fn malformed_trailing_multipart_does_not_commit_a_file() {
    let server = TestServer::start().await;
    let (session, csrf, site) = setup(&server).await;
    let writer = token(&server, &site, &session, &csrf, &["files.write", "files.read"]).await;
    let client = reqwest::Client::new();
    let body = "--audit\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--audit\r\nMalformed header\r\n\r\nignored\r\n--audit--\r\n";
    let response = client
        .post(format!("{}/api/v1/sites/{site}/files", server.base_url))
        .bearer_auth(&writer)
        .header("content-type", "multipart/form-data; boundary=audit")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let files: Value = client
        .get(format!("{}/api/v1/sites/{site}/files", server.base_url))
        .bearer_auth(&writer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        files["items"].as_array().unwrap().len(),
        0,
        "Malformed upload left persisted metadata"
    );
}

#[tokio::test]
async fn singleton_preview_and_versions_are_enforced() {
    let server = TestServer::start().await;
    let (session, csrf, site) = setup(&server).await;
    let client = reqwest::Client::new();
    let response = client.post(format!("{}/api/dashboard/sites/{site}/collections",server.base_url))
        .headers(auth_header(&session,&csrf)).json(&json!({"name":"Settings","slug":"settings","is_singleton":true,"definition":{"fields":[{"name":"title","type":"text"}]}}))
        .send().await.unwrap();
    assert_eq!(response.status(), 201);
    let writer = token(
        &server,
        &site,
        &session,
        &csrf,
        &["content.write", "content.preview.read", "content.read"],
    )
    .await;
    let reader = token(&server, &site, &session, &csrf, &["content.read"]).await;
    let url = format!("{}/api/v1/sites/{site}/singletons/settings", server.base_url);
    let response = client
        .patch(&url)
        .bearer_auth(&writer)
        .json(&json!({"data":{"title":"Private"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let version = response.headers()["etag"].to_str().unwrap().to_owned();
    let response = client.get(&url).bearer_auth(&reader).send().await.unwrap();
    let hidden: Value = response.json().await.unwrap();
    assert!(hidden["data"].is_null());
    assert!(hidden["entry_id"].is_null());
    assert!(hidden["version"].is_null());
    let response = client
        .get(format!("{url}?include_drafts=true"))
        .bearer_auth(&reader)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let response = client
        .get(format!("{url}?include_drafts=true"))
        .bearer_auth(&writer)
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers()["etag"], version);
    let visible: Value = response.json().await.unwrap();
    assert_eq!(visible["data"]["title"], "Private");
    let response = client
        .patch(&url)
        .bearer_auth(&writer)
        .header("If-Match", &version)
        .json(&json!({"data":{"title":"First"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let response = client
        .patch(&url)
        .bearer_auth(&writer)
        .header("If-Match", &version)
        .json(&json!({"data":{"title":"Stale"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 412);
    let response = client
        .patch(&url)
        .bearer_auth(&writer)
        .header("If-Match", "invalid-unquoted")
        .json(&json!({"data":{"title":"Malformed"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let current: Value = client
        .get(format!("{url}?include_drafts=true"))
        .bearer_auth(&writer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current["data"]["title"], "First");
}

#[tokio::test]
async fn signed_upload_cannot_be_replayed_after_permanent_file_deletion() {
    let server = TestServer::start().await;
    let (session, csrf, site) = setup(&server).await;
    let writer = token(&server, &site, &session, &csrf, &["files.write", "files.read"]).await;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/v1/sites/{site}/files/upload-url", server.base_url))
        .bearer_auth(&writer)
        .json(&json!({"filename":"once.txt","content_type":"text/plain"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let upload: Value = response.json().await.unwrap();
    let url = upload["upload_url"].as_str().unwrap();
    let response = client
        .put(url)
        .header("Content-Type", "text/plain")
        .body("original")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let file: Value = response.json().await.unwrap();
    let id = file["id"].as_str().unwrap();
    let response = client
        .delete(format!("{}/api/v1/sites/{site}/files/{id}", server.base_url))
        .bearer_auth(&writer)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    for suffix in [format!("files/{id}"), format!("files/{id}/references")] {
        let response = client
            .get(format!("{}/api/v1/sites/{site}/{suffix}", server.base_url))
            .bearer_auth(&writer)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404, "Public metadata exposed deleted file");
    }
    let dashboard = client
        .get(format!("{}/api/dashboard/sites/{site}/files/{id}", server.base_url))
        .headers(auth_header(&session, &csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(dashboard.status(), 200, "Dashboard trash metadata contract changed");
    let response = client
        .post(format!(
            "{}/api/dashboard/sites/{site}/files/batch-permanent-delete",
            server.base_url
        ))
        .headers(auth_header(&session, &csrf))
        .json(&json!({"ids":[id]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let response = client
        .put(url)
        .header("Content-Type", "text/plain")
        .body("replayed")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        409,
        "File deletion erased signed URL replay protection"
    );
    let files: Value = client
        .get(format!("{}/api/v1/sites/{site}/files", server.base_url))
        .bearer_auth(&writer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(files["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn field_renames_preserve_references_and_failed_schema_updates_are_atomic() {
    let server = TestServer::start().await;
    let (session, csrf, site) = setup(&server).await;
    let client = reqwest::Client::new();
    let writer = token(
        &server,
        &site,
        &session,
        &csrf,
        &["content.write", "content.preview.read", "files.write", "files.read"],
    )
    .await;
    for (name, slug) in [("Media", "media"), ("Conflict", "conflict")] {
        let response = client
            .post(format!("{}/api/dashboard/sites/{site}/collections", server.base_url))
            .headers(auth_header(&session, &csrf))
            .json(&json!({"name":name,"slug":slug,"definition":{"fields":[{"name":"hero","type":"file"}]}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
    }
    let response = client
        .post(format!("{}/api/v1/sites/{site}/files", server.base_url))
        .bearer_auth(&writer)
        .multipart(
            reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::text("content")
                    .file_name("file.txt")
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let file: Value = response.json().await.unwrap();
    let id = file["id"].as_str().unwrap();
    let url = format!("/api/files/{id}");
    let response = client
        .post(format!(
            "{}/api/v1/sites/{site}/collections/media/entries",
            server.base_url
        ))
        .bearer_auth(&writer)
        .json(&json!({"slug":"entry","data":{"hero":url}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let entry: Value = response.json().await.unwrap();
    let entry_id = entry["id"].as_str().unwrap();
    let schema_url = format!("{}/api/dashboard/sites/{site}/collections/media", server.base_url);
    let response = client
        .put(&schema_url)
        .headers(auth_header(&session, &csrf))
        .json(&json!({"slug":"conflict","definition":{"fields":[{"name":"cover","type":"file"}]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let unchanged: Value = client
        .get(format!(
            "{}/api/v1/sites/{site}/entries/{entry_id}?include_drafts=true",
            server.base_url
        ))
        .bearer_auth(&writer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(unchanged["data"]["hero"], url);
    assert_eq!(unchanged["version"], entry["version"]);
    let response = client
        .put(&schema_url)
        .headers(auth_header(&session, &csrf))
        .json(&json!({"definition":{"fields":[{"name":"cover","type":"file"}]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let current: Value = client
        .get(format!(
            "{}/api/v1/sites/{site}/entries/{entry_id}?include_drafts=true",
            server.base_url
        ))
        .bearer_auth(&writer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current["data"]["cover"], url);
    assert!(current["data"].get("hero").is_none());
    assert_ne!(current["version"], entry["version"]);
    let references: Value = client
        .get(format!("{}/api/v1/sites/{site}/files/{id}/references", server.base_url))
        .bearer_auth(&writer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(references.as_array().unwrap().len(), 1);
    assert_eq!(references[0]["field_name"], "cover");
}

#[tokio::test]
async fn filename_limits_and_unicode_delivery_headers_are_consistent() {
    let server = TestServer::start().await;
    let (session, csrf, site) = setup(&server).await;
    let writer = token(&server, &site, &session, &csrf, &["files.write"]).await;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/v1/sites/{site}/files/upload-url", server.base_url))
        .bearer_auth(&writer)
        .json(&json!({"filename":"x".repeat(256),"content_type":"text/plain"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    for name in ["résumé.txt".to_owned(), format!("a.{}", "x".repeat(253))] {
        let response = client
            .post(format!("{}/api/v1/sites/{site}/files", server.base_url))
            .bearer_auth(&writer)
            .multipart(
                reqwest::multipart::Form::new().part(
                    "file",
                    reqwest::multipart::Part::text("bytes")
                        .file_name(name.clone())
                        .mime_str("text/plain")
                        .unwrap(),
                ),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201, "Valid filename could not be stored: {name}");
        let file: Value = response.json().await.unwrap();
        let response = client
            .get(format!(
                "{}/api/files/{}",
                server.base_url,
                file["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let disposition = response.headers()["content-disposition"].to_str().unwrap();
        assert!(disposition.contains("filename*=UTF-8''"));
        if name.starts_with("résumé") {
            assert!(disposition.contains("r%C3%A9sum%C3%A9.txt"));
        }
        assert_eq!(response.text().await.unwrap(), "bytes");
    }
}

#[tokio::test]
async fn expiring_tokens_and_bearer_scheme_case_work_across_http_protocols() {
    let server = TestServer::start().await;
    let (session, csrf, site) = setup(&server).await;
    let client = reqwest::Client::new();
    let response = client.post(format!("{}/api/dashboard/sites/{site}/tokens",server.base_url)).headers(auth_header(&session,&csrf))
        .json(&json!({"name":"Expires later","scopes":["site.read","mcp.use"],"expires_at":(chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339()}))
        .send().await.unwrap();
    assert_eq!(response.status(), 201);
    let credential: Value = response.json().await.unwrap();
    let authorization = format!("bEaReR {}", credential["token"].as_str().unwrap());
    let response = client
        .get(format!("{}/api/v1/sites/{site}", server.base_url))
        .header("Authorization", &authorization)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Future token rejected by REST");
    let response = client
        .post(format!("{}/api/graphql", server.base_url))
        .header("Authorization", &authorization)
        .json(&json!({"query":"{ currentSite { id } }"}))
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    assert!(body["errors"].is_null(), "Future token rejected by GraphQL: {body}");
    assert_eq!(body["data"]["currentSite"]["id"], site);
    let response = client
        .post(format!("{}/mcp", server.base_url))
        .header("Authorization", &authorization)
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "server/discover")
        .json(
            &json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{
            "io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "Future token rejected by MCP");
    let body: Value = response.json().await.unwrap();
    assert!(
        body["result"]["supportedVersions"]
            .as_array()
            .unwrap()
            .contains(&json!("2026-07-28"))
    );
}

#[tokio::test]
async fn public_lists_reject_unknown_collections_and_bad_cursors_with_actionable_errors() {
    let server = TestServer::start().await;
    let (session, csrf, site) = setup(&server).await;
    let client = reqwest::Client::new();
    let created = client
        .post(format!("{}/api/dashboard/sites/{site}/collections", server.base_url))
        .headers(auth_header(&session, &csrf))
        .json(&json!({"name":"Posts","slug":"posts","definition":{"fields":[{"name":"title","type":"text"}]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let reader = token(&server, &site, &session, &csrf, &["content.read", "files.read"]).await;
    let base = format!("{}/api/v1/sites/{site}", server.base_url);
    let get = |path: String| client.get(format!("{base}{path}")).bearer_auth(&reader).send();

    assert_eq!(get("/collections/missing/entries".into()).await.unwrap().status(), 404);
    for path in ["/collections/posts/entries?cursor=garbage", "/files?cursor=garbage"] {
        let response = get(path.into()).await.unwrap();
        assert_eq!(response.status(), 400, "{path}");
        let body: Value = response.json().await.unwrap();
        assert!(body["detail"].as_str().unwrap().contains("Cursor"), "{path}: {body}");
    }

    let response = get("/collections/posts/entries?include_drafts=true".into())
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let body: Value = response.json().await.unwrap();
    assert!(
        body["detail"].as_str().unwrap().contains("content.preview.read"),
        "403 must name the missing scope: {body}"
    );
}
