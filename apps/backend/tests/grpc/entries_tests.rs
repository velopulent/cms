use cms::grpc::cms::v1::collection_service_client::CollectionServiceClient;
use cms::grpc::cms::v1::entry_service_client::EntryServiceClient;
use cms::grpc::cms::v1::{
    CreateCollectionRequest, CreateEntryRequest, DeleteEntryRequest, GetEntryRequest, GetEntryRevisionRequest,
    ListEntriesRequest, ListEntriesResponse, ListEntryRevisionsRequest, PublishEntryRequest,
    RestoreEntryRevisionRequest, UnpublishEntryRequest, UpdateEntryRequest,
};

use crate::common::{GrpcTestContext, grpc::auth_interceptor};

async fn setup() -> (GrpcTestContext, String, String, String) {
    let ctx = GrpcTestContext::start().await;
    let (site_id, token) = ctx.setup_site_and_token().await;

    let channel = ctx.connect().await;
    let mut client = CollectionServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let collection = client
        .create_collection(tonic::Request::new(CreateCollectionRequest {
            name: "Posts".into(),
            slug: "posts".into(),
            definition: r#"{"fields":[{"name":"title","type":"text"}]}"#.into(),
            is_singleton: false,
        }))
        .await
        .unwrap()
        .into_inner();

    (ctx, site_id, token, collection.id)
}

async fn wait_for_search<I>(
    client: &mut EntryServiceClient<tonic::service::interceptor::InterceptedService<tonic::transport::Channel, I>>,
    collection_id: &str,
    search: &str,
    expected_slug: &str,
) -> ListEntriesResponse
where
    I: tonic::service::Interceptor + Clone,
{
    for _ in 0..50 {
        let response = client
            .list_entries(tonic::Request::new(ListEntriesRequest {
                collection_id: Some(collection_id.to_owned()),
                status: None,
                search: Some(search.to_owned()),
                page: 1,
                per_page: 10,
            }))
            .await
            .unwrap()
            .into_inner();
        if response.items.iter().any(|item| item.slug == expected_slug) {
            return response;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    panic!("{expected_slug} never became searchable");
}

#[tokio::test]
async fn test_create_entry() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let resp = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            collection_id: collection_id.clone(),
            data: r#"{"title":"Hello World"}"#.into(),
            slug: "hello-world".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.collection_id, collection_id);
    assert_eq!(resp.slug, "hello-world");
    assert_eq!(resp.status, "draft");
    assert!(!resp.id.is_empty());
}

#[tokio::test]
async fn test_get_entry() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            collection_id: collection_id.clone(),
            data: r#"{"title":"Test"}"#.into(),
            slug: "test-entry".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let fetched = client
        .get_entry(tonic::Request::new(GetEntryRequest { id: created.id.clone() }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.slug, "test-entry");
}

#[tokio::test]
async fn test_list_entries() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    for i in 0..3 {
        let _created = client
            .create_entry(tonic::Request::new(CreateEntryRequest {
                collection_id: collection_id.clone(),
                data: format!(r#"{{"title":"Entry {}"}}"#, i),
                slug: format!("entry-{}", i),
            }))
            .await
            .unwrap();
    }

    let resp = client
        .list_entries(tonic::Request::new(ListEntriesRequest {
            collection_id: Some(collection_id),
            status: None,
            search: None,
            page: 1,
            per_page: 10,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.items.len(), 3);
    assert_eq!(resp.total, 3);
}

#[tokio::test]
async fn test_list_entries_with_search() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            collection_id: collection_id.clone(),
            data: r#"{"title":"Unique Title"}"#.into(),
            slug: "searchable".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _published = client
        .publish_entry(tonic::Request::new(PublishEntryRequest { id: created.id.clone() }))
        .await
        .unwrap();

    // Search indexing is asynchronous in production. Poll briefly until the
    // queue consumer publishes the committed document to the reader.
    let resp = wait_for_search(&mut client, &collection_id, "Unique", "searchable").await;

    let slugs: Vec<&str> = resp.items.iter().map(|i| i.slug.as_str()).collect();
    assert!(
        slugs.contains(&"searchable"),
        "expected slug 'searchable' in search results, got: {:?}",
        slugs
    );
}

#[tokio::test]
async fn test_search_indexes_are_isolated_across_concurrent_contexts() {
    let ((ctx_a, _site_a, token_a, collection_a), (ctx_b, _site_b, token_b, collection_b)) =
        tokio::join!(setup(), setup());

    let channel_a = ctx_a.connect().await;
    let channel_b = ctx_b.connect().await;
    let mut client_a = EntryServiceClient::with_interceptor(channel_a, auth_interceptor(&token_a));
    let mut client_b = EntryServiceClient::with_interceptor(channel_b, auth_interceptor(&token_b));

    for (client, collection_id, slug) in [
        (&mut client_a, collection_a.clone(), "context-a"),
        (&mut client_b, collection_b.clone(), "context-b"),
    ] {
        let created = client
            .create_entry(tonic::Request::new(CreateEntryRequest {
                collection_id,
                data: r#"{"title":"Running"}"#.into(),
                slug: slug.into(),
            }))
            .await
            .unwrap()
            .into_inner();
        client
            .publish_entry(tonic::Request::new(PublishEntryRequest { id: created.id }))
            .await
            .unwrap();
    }

    for (client, collection_id, expected_slug, other_slug) in [
        (&mut client_a, collection_a, "context-a", "context-b"),
        (&mut client_b, collection_b, "context-b", "context-a"),
    ] {
        // Exercise Tantivy's stemming rather than an exact stored-value match.
        let response = wait_for_search(client, &collection_id, "run", expected_slug).await;
        let slugs: Vec<&str> = response.items.iter().map(|item| item.slug.as_str()).collect();
        assert!(
            !slugs.contains(&other_slug),
            "search index leaked across contexts: {slugs:?}"
        );
    }
}

#[tokio::test]
async fn test_update_entry() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            collection_id: collection_id.clone(),
            data: r#"{"title":"Old"}"#.into(),
            slug: "update-me".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let updated = client
        .update_entry(tonic::Request::new(UpdateEntryRequest {
            id: created.id,
            data: Some(r#"{"title":"New"}"#.into()),
            slug: None,
            status: None,
            change_summary: Some("Updated title".into()),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(updated.data, r#"{"title":"New"}"#);
}

#[tokio::test]
async fn test_delete_entry() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            collection_id: collection_id.clone(),
            data: r#"{"title":"Delete Me"}"#.into(),
            slug: "delete-me".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let resp = client
        .delete_entry(tonic::Request::new(DeleteEntryRequest { id: created.id }))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.success);

    let result = client
        .get_entry(tonic::Request::new(GetEntryRequest {
            id: "nonexistent".into(),
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_publish_entry() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            collection_id: collection_id.clone(),
            data: r#"{"title":"Publish Me"}"#.into(),
            slug: "publish-me".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(created.status, "draft");

    let published = client
        .publish_entry(tonic::Request::new(PublishEntryRequest { id: created.id }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(published.status, "published");
    assert!(published.published_at.is_some());
}

#[tokio::test]
async fn test_unpublish_entry() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            collection_id: collection_id.clone(),
            data: r#"{"title":"Unpublish Me"}"#.into(),
            slug: "unpublish-me".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _published = client
        .publish_entry(tonic::Request::new(PublishEntryRequest { id: created.id.clone() }))
        .await
        .unwrap();

    let unpublished = client
        .unpublish_entry(tonic::Request::new(UnpublishEntryRequest { id: created.id }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(unpublished.status, "draft");
    assert!(unpublished.published_at.is_none());
}

#[tokio::test]
async fn test_list_entry_revisions() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            collection_id: collection_id.clone(),
            data: r#"{"title":"V1"}"#.into(),
            slug: "revision-test".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _updated = client
        .update_entry(tonic::Request::new(UpdateEntryRequest {
            id: created.id.clone(),
            data: Some(r#"{"title":"V2"}"#.into()),
            slug: None,
            status: None,
            change_summary: Some("Updated to V2".into()),
        }))
        .await
        .unwrap();

    let resp = client
        .list_entry_revisions(tonic::Request::new(ListEntryRevisionsRequest {
            entry_id: created.id,
            page: 1,
            per_page: 10,
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.items.len() >= 2);
    assert!(resp.total >= 2);
}

#[tokio::test]
async fn test_get_entry_revision() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            collection_id: collection_id.clone(),
            data: r#"{"title":"V1"}"#.into(),
            slug: "get-revision".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _updated = client
        .update_entry(tonic::Request::new(UpdateEntryRequest {
            id: created.id.clone(),
            data: Some(r#"{"title":"V2"}"#.into()),
            slug: None,
            status: None,
            change_summary: None,
        }))
        .await
        .unwrap();

    let revision = client
        .get_entry_revision(tonic::Request::new(GetEntryRevisionRequest {
            entry_id: created.id,
            revision_number: 1,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(revision.revision_number, 1);
    assert!(revision.data.contains("V1"));
}

#[tokio::test]
async fn test_restore_entry_revision() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            collection_id: collection_id.clone(),
            data: r#"{"title":"Original"}"#.into(),
            slug: "restore-revision".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _updated = client
        .update_entry(tonic::Request::new(UpdateEntryRequest {
            id: created.id.clone(),
            data: Some(r#"{"title":"Changed"}"#.into()),
            slug: None,
            status: None,
            change_summary: None,
        }))
        .await
        .unwrap();

    let restored = client
        .restore_entry_revision(tonic::Request::new(RestoreEntryRevisionRequest {
            entry_id: created.id,
            revision_number: 1,
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(restored.data.contains("Original"));
}
