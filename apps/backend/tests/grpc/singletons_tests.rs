use cms::grpc::cms::v1::singleton_service_client::SingletonServiceClient;
use cms::grpc::cms::v1::{GetSingletonRequest, UpdateSingletonRequest};

use crate::common::{GrpcTestContext, grpc::auth_interceptor};

async fn setup() -> (GrpcTestContext, String, String) {
    let ctx = GrpcTestContext::start().await;
    let (site_id, token) = ctx.setup_site_and_token().await;

    ctx.create_collection(
        &site_id,
        "Settings",
        "settings",
        serde_json::json!({"fields":[{"name":"site_name","type":"text"}]}),
        true,
    )
    .await;

    (ctx, site_id, token)
}

#[tokio::test]
async fn test_get_singleton() {
    let (ctx, _site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = SingletonServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let resp = client
        .get_singleton(tonic::Request::new(GetSingletonRequest {
            include_drafts: true,
            slug: "settings".into(),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.slug, "settings");
    assert_eq!(resp.name, "Settings");
    assert!(!resp.id.is_empty());
}

#[tokio::test]
async fn test_update_singleton() {
    let (ctx, _site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = SingletonServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let resp = client
        .update_singleton(tonic::Request::new(UpdateSingletonRequest {
            slug: "settings".into(),
            data: r#"{"site_name":"My Site"}"#.into(),
            change_summary: None,
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.slug, "settings");
    assert!(resp.data.is_some());
    let data = resp.data.unwrap();
    assert!(data.contains("My Site"));
}
