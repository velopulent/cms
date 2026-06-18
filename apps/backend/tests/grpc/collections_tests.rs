use cms::grpc::cms::v1::collection_service_client::CollectionServiceClient;
use cms::grpc::cms::v1::{
    CreateCollectionRequest, DeleteCollectionRequest, GetCollectionRequest, ListCollectionsRequest,
    UpdateCollectionRequest,
};

use crate::common::{GrpcTestContext, grpc::auth_interceptor};

async fn setup() -> (GrpcTestContext, String, String) {
    let ctx = GrpcTestContext::start().await;
    let (site_id, token) = ctx.setup_site_and_token().await;
    (ctx, site_id, token)
}

#[tokio::test]
async fn test_create_collection() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let resp = client
        .create_collection(tonic::Request::new(CreateCollectionRequest {
            name: "Posts".into(),
            slug: "posts".into(),
            definition: r#"{"fields":[{"name":"title","type":"text"}]}"#.into(),
            is_singleton: false,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.name, "Posts");
    assert_eq!(resp.slug, "posts");
    assert_eq!(resp.site_id, site_id);
    assert!(!resp.is_singleton);
    assert!(!resp.id.is_empty());
}

#[tokio::test]
async fn test_get_collection() {
    let (ctx, _site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_collection(tonic::Request::new(CreateCollectionRequest {
            name: "Pages".into(),
            slug: "pages".into(),
            definition: "{}".into(),
            is_singleton: false,
        }))
        .await
        .unwrap()
        .into_inner();

    let fetched = client
        .get_collection(tonic::Request::new(GetCollectionRequest { slug: "pages".into() }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "Pages");
    assert_eq!(fetched.slug, "pages");
}

#[tokio::test]
async fn test_list_collections() {
    let (ctx, _site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let _created = client
        .create_collection(tonic::Request::new(CreateCollectionRequest {
            name: "Posts".into(),
            slug: "posts".into(),
            definition: "{}".into(),
            is_singleton: false,
        }))
        .await
        .unwrap();

    let _created = client
        .create_collection(tonic::Request::new(CreateCollectionRequest {
            name: "Pages".into(),
            slug: "pages".into(),
            definition: "{}".into(),
            is_singleton: false,
        }))
        .await
        .unwrap();

    let resp = client
        .list_collections(tonic::Request::new(ListCollectionsRequest {}))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.collections.len(), 2);
    let names: Vec<&str> = resp.collections.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"Posts"));
    assert!(names.contains(&"Pages"));
}

#[tokio::test]
async fn test_update_collection() {
    let (ctx, _site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_collection(tonic::Request::new(CreateCollectionRequest {
            name: "Old Name".into(),
            slug: "old-name".into(),
            definition: "{}".into(),
            is_singleton: false,
        }))
        .await
        .unwrap()
        .into_inner();

    let updated = client
        .update_collection(tonic::Request::new(UpdateCollectionRequest {
            id: created.id,
            name: Some("New Name".into()),
            slug: Some("new-name".into()),
            definition: None,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.slug, "new-name");
}

#[tokio::test]
async fn test_delete_collection() {
    let (ctx, _site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_collection(tonic::Request::new(CreateCollectionRequest {
            name: "To Delete".into(),
            slug: "to-delete".into(),
            definition: "{}".into(),
            is_singleton: false,
        }))
        .await
        .unwrap()
        .into_inner();

    let resp = client
        .delete_collection(tonic::Request::new(DeleteCollectionRequest {
            id: created.id,
            slug: "to-delete".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.success);

    let result = client
        .get_collection(tonic::Request::new(GetCollectionRequest {
            slug: "to-delete".into(),
        }))
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_grpc_unauthenticated() {
    let ctx = GrpcTestContext::start().await;
    let channel = ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor("invalid-token-12345"));

    let result = client
        .list_collections(tonic::Request::new(ListCollectionsRequest {}))
        .await;

    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn test_grpc_create_collection_invalid_definition() {
    let (_ctx, _site_id, token) = setup().await;
    let channel = _ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let result = client
        .create_collection(tonic::Request::new(CreateCollectionRequest {
            name: "Bad".into(),
            slug: "bad".into(),
            definition: r#"{"fields":[{"name":"title","type":"string"}]}"#.into(),
            is_singleton: false,
        }))
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_grpc_get_collection_not_found() {
    let (_ctx, _site_id, token) = setup().await;
    let channel = _ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let result = client
        .get_collection(tonic::Request::new(GetCollectionRequest {
            slug: "nonexistent".into(),
        }))
        .await;

    assert!(result.is_err());
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::NotFound);
}
