use cms::grpc::cms::v1::site_service_client::SiteServiceClient;
use cms::grpc::cms::v1::{GetSiteRequest, UpdateSiteRequest};

use crate::common::{GrpcTestContext, auth_interceptor, seed_access_token, seed_site};

async fn setup() -> (GrpcTestContext, String, String) {
    let ctx = GrpcTestContext::start().await;
    let site_id = seed_site(&ctx.repository, "Test Site", &ctx.admin_user_id).await;
    let token = seed_access_token(&ctx.repository, &site_id, "write").await;
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
    assert_eq!(resp.storage_provider, "filesystem");
}

#[tokio::test]
async fn test_update_site() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = SiteServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let resp = client
        .update_site(tonic::Request::new(UpdateSiteRequest {
            site_id: site_id.clone(),
            name: Some("Updated Site".into()),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.id, site_id);
    assert_eq!(resp.name, "Updated Site");
}
