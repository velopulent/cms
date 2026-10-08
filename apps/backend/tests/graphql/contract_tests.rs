use serde_json::{Value, json};

use crate::common::{TestServer, auth::extract_cookies};

async fn setup(server: &TestServer) -> (String, String, String, String) {
    let client = reqwest::Client::new();
    let login = server.login_user(&client, "admin@cms.local", "admin").await;
    let (session, csrf) = extract_cookies(&login);
    let site_response = client
        .post(format!("{}/api/dashboard/sites", server.base_url))
        .header("Cookie", format!("token={session}; csrf={csrf}"))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "GraphQL Contract Site", "storage_provider": "filesystem"}))
        .send()
        .await
        .unwrap();
    let site: Value = site_response.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap().to_owned();
    let token_response = client
        .post(format!("{}/api/dashboard/sites/{site_id}/tokens", server.base_url))
        .header("Cookie", format!("token={session}; csrf={csrf}"))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({"name": "GraphQL Contract", "scopes": crate::common::fixtures::site_key_scopes("write")}))
        .send()
        .await
        .unwrap();
    let token = token_response.json::<Value>().await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let collection = client
        .post(format!("{}/api/dashboard/sites/{site_id}/collections", server.base_url))
        .header("Cookie", format!("token={session}; csrf={csrf}"))
        .header("X-CSRF-Token", &csrf)
        .json(&json!({
            "name": "Posts",
            "slug": "posts",
            "definition": {"fields": [{"name": "title", "type": "text"}]}
        }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    (site_id, token, collection["id"].as_str().unwrap().to_owned(), session)
}

async fn gql(server: &TestServer, token: &str, query: &str, variables: Value) -> Value {
    reqwest::Client::new()
        .post(format!("{}/api/graphql", server.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({"query": query, "variables": variables}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn site_namespace_and_explicit_entry_mutation_work() {
    let server = TestServer::start().await;
    let (site_id, token, collection_id, _) = setup(&server).await;
    let body = gql(
        &server,
        &token,
        "mutation Create($siteId: String!, $input: CreateEntryInput!) { createEntry(siteId: $siteId, input: $input) { id data version } }",
        json!({"siteId": site_id, "input": {"collectionId": collection_id, "slug": "hello", "data": {"title": "Hello"}}}),
    )
    .await;
    assert!(body["errors"].is_null(), "GraphQL errors: {}", body["errors"]);
    if body["data"]["createEntry"]["data"]["title"] != "Hello" {
        panic!("body: {body}");
    }
    assert!(!body["data"]["createEntry"]["version"].as_str().unwrap().is_empty());

    let body = gql(
        &server,
        &token,
        &format!("{{ site(id: \"{site_id}\") {{ id entries(includeDrafts: true) {{ nodes {{ slug }} pageInfo {{ hasNextPage }} totalCount }} }} }}"),
        json!({}),
    )
    .await;
    assert!(body["errors"].is_null(), "GraphQL errors: {}", body["errors"]);
    assert_eq!(body["data"]["site"]["entries"]["nodes"][0]["slug"], "hello");
}

#[tokio::test]
async fn introspection_requires_authentication() {
    let server = TestServer::start().await;
    let response = reqwest::Client::new()
        .post(format!("{}/api/graphql", server.base_url))
        .json(&json!({"query": "{ __schema { queryType { name } } }"}))
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    assert!(
        body["data"]["__schema"].is_null(),
        "Unauthenticated schema exposed: {body}"
    );
}

#[tokio::test]
async fn draft_status_cannot_override_visibility_and_writers_cannot_publish() {
    let server = TestServer::start().await;
    let (site, writer, collection, _) = setup(&server).await;
    let created = gql(
        &server,
        &writer,
        "mutation($site: String!, $input: CreateEntryInput!) { createEntry(siteId:$site,input:$input) { id } }",
        json!({"site":site,"input":{"collectionId":collection,"slug":"private","data":{"title":"Secret"}}}),
    )
    .await;
    assert!(created["errors"].is_null(), "{created}");
    let id = created["data"]["createEntry"]["id"].as_str().unwrap();
    let client = reqwest::Client::new();
    let login = server.login_user(&client, "admin@cms.local", "admin").await;
    let (session, csrf) = extract_cookies(&login);
    let response = client
        .post(format!("{}/api/dashboard/sites/{site}/tokens", server.base_url))
        .headers(crate::common::auth::auth_header(&session, &csrf))
        .json(&json!({"name":"Limited", "scopes":["site.read","schema.read","content.read","content.write"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let limited = response.json::<Value>().await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned();
    for query in [format!(
        "{{ site(id:\"{site}\") {{ entries(status:\"draft\",includeDrafts:false) {{ nodes {{ id data }} }} }} }}"
    )] {
        let body = gql(&server, &limited, &query, json!({})).await;
        assert!(body["errors"].is_null(), "{body}");
        assert!(!body.to_string().contains("Secret"), "draft leaked: {body}");
        assert!(!body.to_string().contains(id), "draft ID leaked: {body}");
    }
    let body = gql(
        &server,
        &limited,
        &format!("mutation {{ updateEntry(siteId:\"{site}\",id:\"{id}\",input:{{status:\"published\"}}) {{ id }} }}"),
        json!({}),
    )
    .await;
    assert!(body["errors"].is_array(), "publication scope bypass: {body}");
    let body = gql(
        &server,
        &writer,
        &format!("{{ site(id:\"{site}\") {{ entry(id:\"{id}\",includeDrafts:true) {{ status }} }} }}"),
        json!({}),
    )
    .await;
    assert_eq!(body["data"]["site"]["entry"]["status"], "draft");
}

#[tokio::test]
async fn namespaced_nested_collections_use_explicit_pat_site() {
    let server = TestServer::start().await;
    let (site, writer, collection, _) = setup(&server).await;
    let created = gql(
        &server,
        &writer,
        "mutation($site: String!, $input: CreateEntryInput!) { createEntry(siteId:$site,input:$input) { id } }",
        json!({"site":site,"input":{"collectionId":collection,"slug":"private","data":{"title":"Secret"}}}),
    )
    .await;
    assert!(created["errors"].is_null(), "{created}");
    let client = reqwest::Client::new();
    let login = server.login_user(&client, "admin@cms.local", "admin").await;
    let (session, csrf) = extract_cookies(&login);
    let response = client
        .post(format!("{}/api/dashboard/account/tokens", server.base_url))
        .headers(crate::common::auth::auth_header(&session, &csrf))
        .json(&json!({"name":"PAT","scopes":["site.read","schema.read","content.preview.read"],"expires_at":null}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let pat = response.json::<Value>().await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let body = gql(
        &server,
        &pat,
        &format!("{{ site(id:\"{site}\") {{ collections {{ entries(status:\"draft\") {{ slug }} }} }} }}"),
        json!({}),
    )
    .await;
    assert!(
        body["errors"].is_null(),
        "Nested resolver ignored explicit site: {body}"
    );
    assert_eq!(body["data"]["site"]["collections"][0]["entries"][0]["slug"], "private");
}

#[tokio::test]
async fn excessive_aliases_are_rejected_before_resolver_work() {
    let server = TestServer::start().await;
    let query = format!(
        "{{ {} }}",
        (0..257)
            .map(|index| format!("field{index}:__typename"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let body = gql(&server, "invalid", &query, json!({})).await;
    assert!(body["errors"].is_array(), "{body}");
    assert!(
        body["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("256-alias limit")
    );
    assert!(body["data"].is_null());
}

#[tokio::test]
async fn content_only_personal_tokens_read_with_explicit_site_context() {
    let server = TestServer::start().await;
    let (site, writer, collection, _) = setup(&server).await;
    let created = gql(
        &server,
        &writer,
        "mutation($site:String!,$input:CreateEntryInput!){createEntry(siteId:$site,input:$input){id}}",
        json!({"site":site,"input":{"collectionId":collection,"slug":"published","data":{"title":"Public"}}}),
    )
    .await;
    assert!(created["errors"].is_null(), "{created}");
    let id = created["data"]["createEntry"]["id"].as_str().unwrap();
    let published = gql(
        &server,
        &writer,
        &format!("mutation {{ setEntryPublication(siteId:\"{site}\",id:\"{id}\",published:true) {{ id }} }}"),
        json!({}),
    )
    .await;
    assert!(published["errors"].is_null(), "{published}");
    let client = reqwest::Client::new();
    let login = server.login_user(&client, "admin@cms.local", "admin").await;
    let (session, csrf) = extract_cookies(&login);
    let response = client
        .post(format!("{}/api/dashboard/account/tokens", server.base_url))
        .headers(crate::common::auth::auth_header(&session, &csrf))
        .json(&json!({"name":"Content only","scopes":["content.read"],"expires_at":null}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let pat = response.json::<Value>().await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let response = client
        .post(format!("{}/api/graphql", server.base_url))
        .bearer_auth(&pat)
        .json(&json!({"query":format!("{{ site(id:\"{site}\") {{ entry(id:\"{id}\") {{ id data }} }} }}")}))
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    assert!(
        body["errors"].is_null(),
        "Context selection required unrelated site.read scope: {body}"
    );
    assert_eq!(body["data"]["site"]["entry"]["data"]["title"], "Public");
    let other_site = gql(
        &server,
        &pat,
        &format!("{{ site(id:\"not-a-member\") {{ entry(id:\"{id}\") {{ id }} }} }}"),
        json!({}),
    )
    .await;
    assert!(other_site["errors"].is_array(), "PAT read a site it does not belong to");
}
