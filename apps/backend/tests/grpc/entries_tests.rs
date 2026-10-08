use cms::grpc::cms::v1::entry_service_client::EntryServiceClient;
use cms::grpc::cms::v1::{
    CreateEntryRequest, DeleteEntryRequest, GetEntryRequest, GetEntryRevisionRequest, ListEntriesRequest,
    ListEntriesResponse, ListEntryRevisionsRequest, PublishEntryRequest, RestoreEntryRevisionRequest,
    UnpublishEntryRequest, UpdateEntryRequest,
};

use crate::common::{GrpcTestContext, grpc::auth_interceptor, grpc::content, grpc::content_json};

async fn setup() -> (GrpcTestContext, String, String, String) {
    let ctx = GrpcTestContext::start().await;
    let (site_id, token) = ctx.setup_site_and_token().await;

    let collection = ctx
        .create_collection(
            &site_id,
            "Posts",
            "posts",
            serde_json::json!({"fields":[{"name":"title","type":"text"}]}),
            false,
        )
        .await;

    (ctx, site_id, token, collection["id"].as_str().unwrap().to_string())
}

async fn wait_for_search<I>(
    client: &mut EntryServiceClient<tonic::service::interceptor::InterceptedService<tonic::transport::Channel, I>>,
    site_id: &str,
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
                site_id: site_id.to_owned(),
                collection_id: Some(collection_id.to_owned()),
                status: None,
                search: Some(search.to_owned()),
                page_size: 10,
                ..Default::default()
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
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let resp = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            data: content(r#"{"title":"Hello World"}"#),
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
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            data: content(r#"{"title":"Test"}"#),
            slug: "test-entry".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let fetched = client
        .get_entry(tonic::Request::new(GetEntryRequest {
            site_id: site_id.clone(),
            id: created.id.clone(),
            include_drafts: true,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.slug, "test-entry");
}

#[tokio::test]
async fn test_list_entries() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    for i in 0..3 {
        let _created = client
            .create_entry(tonic::Request::new(CreateEntryRequest {
                site_id: site_id.clone(),
                collection_id: collection_id.clone(),
                data: content(&format!(r#"{{"title":"Entry {}"}}"#, i)),
                slug: format!("entry-{}", i),
            }))
            .await
            .unwrap();
    }

    let resp = client
        .list_entries(tonic::Request::new(ListEntriesRequest {
            site_id: site_id.clone(),
            collection_id: Some(collection_id),
            status: None,
            search: None,
            page_size: 10,
            include_drafts: true,
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.items.len(), 3);
    assert_eq!(resp.total_size, 3);
}

#[tokio::test]
async fn test_list_entries_with_search() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            data: content(r#"{"title":"Unique Title"}"#),
            slug: "searchable".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _published = client
        .publish_entry(tonic::Request::new(PublishEntryRequest {
            site_id: site_id.clone(),
            id: created.id.clone(),
        }))
        .await
        .unwrap();

    // Search indexing is asynchronous in production. Poll briefly until the
    // queue consumer publishes the committed document to the reader.
    let resp = wait_for_search(&mut client, &site_id, &collection_id, "Unique", "searchable").await;

    let slugs: Vec<&str> = resp.items.iter().map(|i| i.slug.as_str()).collect();
    assert!(
        slugs.contains(&"searchable"),
        "expected slug 'searchable' in search results, got: {:?}",
        slugs
    );
}

#[tokio::test]
async fn test_search_indexes_are_isolated_across_concurrent_contexts() {
    let ((ctx_a, site_a, token_a, collection_a), (ctx_b, site_b, token_b, collection_b)) =
        tokio::join!(setup(), setup());

    let channel_a = ctx_a.connect().await;
    let channel_b = ctx_b.connect().await;
    let mut client_a = EntryServiceClient::with_interceptor(channel_a, auth_interceptor(&token_a));
    let mut client_b = EntryServiceClient::with_interceptor(channel_b, auth_interceptor(&token_b));

    for (client, site_id, collection_id, slug) in [
        (&mut client_a, site_a.clone(), collection_a.clone(), "context-a"),
        (&mut client_b, site_b.clone(), collection_b.clone(), "context-b"),
    ] {
        let created = client
            .create_entry(tonic::Request::new(CreateEntryRequest {
                site_id: site_id.clone(),
                collection_id,
                data: content(r#"{"title":"Running"}"#),
                slug: slug.into(),
            }))
            .await
            .unwrap()
            .into_inner();
        client
            .publish_entry(tonic::Request::new(PublishEntryRequest {
                site_id: site_id.clone(),
                id: created.id,
            }))
            .await
            .unwrap();
    }

    for (client, site_id, collection_id, expected_slug, other_slug) in [
        (&mut client_a, site_a, collection_a, "context-a", "context-b"),
        (&mut client_b, site_b, collection_b, "context-b", "context-a"),
    ] {
        // Exercise Tantivy's stemming rather than an exact stored-value match.
        let response = wait_for_search(client, &site_id, &collection_id, "run", expected_slug).await;
        let slugs: Vec<&str> = response.items.iter().map(|item| item.slug.as_str()).collect();
        assert!(
            !slugs.contains(&other_slug),
            "search index leaked across contexts: {slugs:?}"
        );
    }
}

#[tokio::test]
async fn test_update_entry() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            data: content(r#"{"title":"Old"}"#),
            slug: "update-me".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let updated = client
        .update_entry(tonic::Request::new(UpdateEntryRequest {
            site_id: site_id.clone(),
            id: created.id,
            data: content(r#"{"title":"New"}"#),
            slug: None,
            status: None,
            change_summary: Some("Updated title".into()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(
        content_json(&updated.data),
        serde_json::from_str::<serde_json::Value>(r#"{"title":"New"}"#).unwrap()
    );
}

#[tokio::test]
async fn test_delete_entry() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            data: content(r#"{"title":"Delete Me"}"#),
            slug: "delete-me".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let resp = client
        .delete_entry(tonic::Request::new(DeleteEntryRequest {
            site_id: site_id.clone(),
            id: created.id,
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.deleted);

    let result = client
        .get_entry(tonic::Request::new(GetEntryRequest {
            site_id: site_id.clone(),
            id: "nonexistent".into(),
            ..Default::default()
        }))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_publish_entry() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            data: content(r#"{"title":"Publish Me"}"#),
            slug: "publish-me".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(created.status, "draft");

    let published = client
        .publish_entry(tonic::Request::new(PublishEntryRequest {
            site_id: site_id.clone(),
            id: created.id,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(published.status, "published");
    assert!(published.published_at.is_some());
}

#[tokio::test]
async fn test_unpublish_entry() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            data: content(r#"{"title":"Unpublish Me"}"#),
            slug: "unpublish-me".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _published = client
        .publish_entry(tonic::Request::new(PublishEntryRequest {
            site_id: site_id.clone(),
            id: created.id.clone(),
        }))
        .await
        .unwrap();

    let unpublished = client
        .unpublish_entry(tonic::Request::new(UnpublishEntryRequest {
            site_id: site_id.clone(),
            id: created.id,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(unpublished.status, "draft");
    assert!(unpublished.published_at.is_none());
}

#[tokio::test]
async fn test_list_entry_revisions() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            data: content(r#"{"title":"V1"}"#),
            slug: "revision-test".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _updated = client
        .update_entry(tonic::Request::new(UpdateEntryRequest {
            site_id: site_id.clone(),
            id: created.id.clone(),
            data: content(r#"{"title":"V2"}"#),
            slug: None,
            status: None,
            change_summary: Some("Updated to V2".into()),
            ..Default::default()
        }))
        .await
        .unwrap();

    let resp = client
        .list_entry_revisions(tonic::Request::new(ListEntryRevisionsRequest {
            site_id: site_id.clone(),
            entry_id: created.id,
            page_size: 10,
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.items.len() >= 2);
    assert!(resp.total_size >= 2);
}

#[tokio::test]
async fn test_get_entry_revision() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            data: content(r#"{"title":"V1"}"#),
            slug: "get-revision".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _updated = client
        .update_entry(tonic::Request::new(UpdateEntryRequest {
            site_id: site_id.clone(),
            id: created.id.clone(),
            data: content(r#"{"title":"V2"}"#),
            slug: None,
            status: None,
            change_summary: None,
            ..Default::default()
        }))
        .await
        .unwrap();

    let revision = client
        .get_entry_revision(tonic::Request::new(GetEntryRevisionRequest {
            site_id: site_id.clone(),
            entry_id: created.id,
            revision_number: 1,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(revision.revision_number, 1);
    assert!(content_json(&revision.data).to_string().contains("V1"));
}

#[tokio::test]
async fn test_restore_entry_revision() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let channel = ctx.connect().await;
    let mut client = EntryServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_entry(tonic::Request::new(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            data: content(r#"{"title":"Original"}"#),
            slug: "restore-revision".into(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _updated = client
        .update_entry(tonic::Request::new(UpdateEntryRequest {
            site_id: site_id.clone(),
            id: created.id.clone(),
            data: content(r#"{"title":"Changed"}"#),
            slug: None,
            status: None,
            change_summary: None,
            ..Default::default()
        }))
        .await
        .unwrap();

    let restored = client
        .restore_entry_revision(tonic::Request::new(RestoreEntryRevisionRequest {
            site_id: site_id.clone(),
            entry_id: created.id,
            revision_number: 1,
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(content_json(&restored.data).to_string().contains("Original"));
}

#[tokio::test]
async fn typed_values_and_versions_fail_without_mutating_entries() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let mut client = EntryServiceClient::with_interceptor(ctx.connect().await, auth_interceptor(&token));
    for kind in [
        None,
        Some(prost_types::value::Kind::NumberValue(f64::NAN)),
        Some(prost_types::value::Kind::NullValue(7)),
    ] {
        let error = client
            .create_entry(CreateEntryRequest {
                site_id: site_id.clone(),
                collection_id: collection_id.clone(),
                slug: "invalid".into(),
                data: Some(prost_types::Struct {
                    fields: [("title".into(), prost_types::Value { kind })].into_iter().collect(),
                }),
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), tonic::Code::InvalidArgument);
    }
    let created = client
        .create_entry(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id: collection_id.clone(),
            slug: "versioned".into(),
            data: content(r#"{"title":"Original"}"#),
        })
        .await
        .unwrap()
        .into_inner();
    let error = client
        .update_entry(UpdateEntryRequest {
            site_id: site_id.clone(),
            id: created.id.clone(),
            data: content(r#"{"title":"Lost"}"#),
            expected_version: "stale".into(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
    let fetched = client
        .get_entry(GetEntryRequest {
            site_id: site_id.clone(),
            id: created.id,
            include_drafts: true,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        content_json(&fetched.data),
        serde_json::from_str::<serde_json::Value>(r#"{"title":"Original"}"#).unwrap()
    );
    let page = client
        .list_entries(ListEntriesRequest {
            site_id,
            collection_id: Some(collection_id),
            page_size: 1,
            include_drafts: true,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(page.total_size, 1);
    assert!(
        page.next_page_token.is_empty(),
        "exactly full final page must terminate"
    );
}

#[tokio::test]
async fn update_status_requires_publish_scope() {
    let (ctx, site_id, token, collection_id) = setup().await;
    let http = crate::common::client::http_client();
    let response = http
        .post(format!("{}/api/auth/login", ctx.rest_base_url))
        .json(&serde_json::json!({"email":"admin@cms.local","password":"admin"}))
        .send()
        .await
        .unwrap();
    let (session, csrf) = crate::common::auth::extract_cookies(&response);
    let response = http
        .post(format!("{}/api/dashboard/sites/{site_id}/tokens", ctx.rest_base_url))
        .headers(crate::common::auth::auth_header(&session, &csrf))
        .json(&serde_json::json!({"name":"writer","scopes":["content.write"]}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let credential: serde_json::Value = response.json().await.unwrap();
    let mut writer = EntryServiceClient::with_interceptor(
        ctx.connect().await,
        auth_interceptor(credential["token"].as_str().unwrap()),
    );
    let created = writer
        .create_entry(CreateEntryRequest {
            site_id: site_id.clone(),
            collection_id,
            slug: "draft".into(),
            data: content(r#"{"title":"Draft"}"#),
        })
        .await
        .unwrap()
        .into_inner();
    let error = writer
        .update_entry(UpdateEntryRequest {
            site_id: site_id.clone(),
            id: created.id.clone(),
            status: Some("published".into()),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::PermissionDenied);
    let mut reader = EntryServiceClient::with_interceptor(ctx.connect().await, auth_interceptor(&token));
    let fetched = reader
        .get_entry(GetEntryRequest {
            site_id,
            id: created.id,
            include_drafts: true,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(fetched.status, "draft");
}

#[tokio::test]
async fn site_scoped_requests_require_an_explicit_site_id() {
    let (ctx, _site_id, token, collection_id) = setup().await;
    let mut client = EntryServiceClient::with_interceptor(ctx.connect().await, auth_interceptor(&token));
    let error = client
        .list_entries(ListEntriesRequest {
            collection_id: Some(collection_id),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::InvalidArgument);
}
