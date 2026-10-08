use cms::grpc::cms::v1::collection_service_client::CollectionServiceClient;
use cms::grpc::cms::v1::{GetCollectionRequest, ListCollectionsRequest};

use crate::common::{GrpcTestContext, grpc::auth_interceptor};

#[tokio::test]
async fn schema_discovery_is_read_only_and_site_scoped() {
    let ctx = GrpcTestContext::start().await;
    let (site_id, token) = ctx.setup_site_and_token().await;
    let created = ctx
        .create_collection(
            &site_id,
            "Posts",
            "posts",
            serde_json::json!({"fields":[{"name":"title","type":"text"}]}),
            false,
        )
        .await;

    let channel = ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));
    let listed = client
        .list_collections(tonic::Request::new(ListCollectionsRequest {
            site_id: site_id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(listed.collections.len(), 1);
    assert_eq!(listed.collections[0].id, created["id"].as_str().unwrap());

    let fetched = client
        .get_collection(tonic::Request::new(GetCollectionRequest {
            site_id,
            slug: "posts".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(fetched.slug, "posts");
    assert!(fetched.definition_value.is_some());
}

#[tokio::test]
async fn schema_get_missing_collection_is_not_found() {
    let ctx = GrpcTestContext::start().await;
    let (site_id, token) = ctx.setup_site_and_token().await;
    let channel = ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));
    let error = client
        .get_collection(tonic::Request::new(GetCollectionRequest {
            site_id,
            slug: "missing".into(),
        }))
        .await
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::NotFound);
}
