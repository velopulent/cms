use cms::grpc::cms::v1::site_service_client::SiteServiceClient;
use cms::grpc::cms::v1::{GetSiteRequest, ListSitesRequest};

use crate::common::{GrpcTestContext, grpc::auth_interceptor};

async fn setup() -> (GrpcTestContext, String, String) {
    let ctx = GrpcTestContext::start().await;
    let (site_id, token) = ctx.setup_site_and_token().await;
    (ctx, site_id, token)
}

#[tokio::test]
async fn test_get_site() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = SiteServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let resp = client
        .get_site(tonic::Request::new(GetSiteRequest {
            site_id: site_id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.id, site_id);
    assert_eq!(resp.name, "Test Site");
}

#[tokio::test]
async fn test_list_sites() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = SiteServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let resp = client
        .list_sites(tonic::Request::new(ListSitesRequest {}))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.sites.iter().any(|site| site.id == site_id));
}

#[tokio::test]
async fn future_expiry_and_typed_timestamps_work_with_both_database_backends() {
    use serde_json::{Value, json};
    let ctx = crate::common::GrpcTestContext::start().await;
    let (site, _) = ctx.setup_site_and_token().await;
    let http = reqwest::Client::new();
    let response = http
        .post(format!("{}/api/auth/login", ctx.rest_base_url))
        .json(&json!({"email":"admin@cms.local","password":"admin"}))
        .send()
        .await
        .unwrap();
    let (session, csrf) = crate::common::auth::extract_cookies(&response);
    let response = http.post(format!("{}/api/dashboard/sites/{site}/tokens",ctx.rest_base_url))
        .headers(crate::common::auth::auth_header(&session,&csrf))
        .json(&json!({"name":"Future","scopes":["site.read"],"expires_at":(chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339()})).send().await.unwrap();
    assert_eq!(response.status(), 201);
    let credential: Value = response.json().await.unwrap();
    let authorization = format!("bearer {}", credential["token"].as_str().unwrap());
    let mut client = cms::grpc::cms::v1::site_service_client::SiteServiceClient::new(ctx.connect().await);
    let mut request = tonic::Request::new(cms::grpc::cms::v1::GetSiteRequest { site_id: site.clone() });
    request
        .metadata_mut()
        .insert("authorization", authorization.parse().unwrap());
    let response = client.get_site(request).await.unwrap().into_inner();
    assert_eq!(response.id, site);
    let created = response.created_at.expect("Database timestamp was omitted");
    assert!((chrono::Utc::now().timestamp() - created.seconds).abs() < 60);
    assert!(response.updated_at.is_some());
}
