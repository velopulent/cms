use cms::grpc::cms::v1::collection_service_client::CollectionServiceClient;
use cms::grpc::cms::v1::singleton_service_client::SingletonServiceClient;
use cms::grpc::cms::v1::{CreateCollectionRequest, GetSingletonRequest, UpdateSingletonRequest};

use crate::common::{GrpcTestContext, auth_interceptor};

async fn setup() -> (GrpcTestContext, String, String) {
    let ctx = GrpcTestContext::start().await;
    let (site_id, token) = ctx.setup_site_and_token().await;

    let channel = ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));

    client
        .create_collection(tonic::Request::new(CreateCollectionRequest {
            name: "Settings".into(),
            slug: "settings".into(),
            definition: r#"{"fields":[{"name":"site_name","type":"string"}]}"#.into(),
            is_singleton: true,
        }))
        .await
        .unwrap();

    (ctx, site_id, token)
}

#[tokio::test]
async fn test_get_singleton() {
    let (ctx, _site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = SingletonServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let resp = client
        .get_singleton(tonic::Request::new(GetSingletonRequest {
            slug: "settings".into(),
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
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.slug, "settings");
    assert!(resp.data.is_some());
    let data = resp.data.unwrap();
    assert!(data.contains("My Site"));
}
