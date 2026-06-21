use cms::grpc::cms::v1::webhook_service_client::WebhookServiceClient;
use cms::grpc::cms::v1::{
    CreateWebhookRequest, DeleteWebhookRequest, GetWebhookRequest, ListWebhookDeliveriesRequest, ListWebhooksRequest,
    TriggerWebhookRequest, UpdateWebhookRequest,
};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::{GrpcTestContext, grpc::auth_interceptor};

async fn setup() -> (GrpcTestContext, String, String) {
    let ctx = GrpcTestContext::start().await;
    let (site_id, token) = ctx.setup_site_and_token().await;
    (ctx, site_id, token)
}

#[tokio::test]
async fn test_create_webhook() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = WebhookServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let resp = client
        .create_webhook(tonic::Request::new(CreateWebhookRequest {
            site_id: site_id.clone(),
            label: "Test Webhook".into(),
            url: "https://example.com/hook".into(),
            headers: [("X-Custom".into(), "value".into())].into(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.label, "Test Webhook");
    assert_eq!(resp.url, "https://example.com/hook");
    assert_eq!(resp.site_id, site_id);
    assert!(resp.headers.contains_key("X-Custom"));
    assert!(!resp.id.is_empty());
}

#[tokio::test]
async fn test_get_webhook() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = WebhookServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_webhook(tonic::Request::new(CreateWebhookRequest {
            site_id: site_id.clone(),
            label: "Get Webhook".into(),
            url: "https://example.com/get".into(),
            headers: Default::default(),
        }))
        .await
        .unwrap()
        .into_inner();

    let fetched = client
        .get_webhook(tonic::Request::new(GetWebhookRequest {
            site_id: site_id.clone(),
            webhook_id: created.id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.label, "Get Webhook");
}

#[tokio::test]
async fn test_list_webhooks() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = WebhookServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let _created = client
        .create_webhook(tonic::Request::new(CreateWebhookRequest {
            site_id: site_id.clone(),
            label: "Hook 1".into(),
            url: "https://example.com/h1".into(),
            headers: Default::default(),
        }))
        .await
        .unwrap();

    let _created = client
        .create_webhook(tonic::Request::new(CreateWebhookRequest {
            site_id: site_id.clone(),
            label: "Hook 2".into(),
            url: "https://example.com/h2".into(),
            headers: Default::default(),
        }))
        .await
        .unwrap();

    let resp = client
        .list_webhooks(tonic::Request::new(ListWebhooksRequest {
            site_id: site_id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.webhooks.len(), 2);
}

#[tokio::test]
async fn test_update_webhook() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = WebhookServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_webhook(tonic::Request::new(CreateWebhookRequest {
            site_id: site_id.clone(),
            label: "Old Hook".into(),
            url: "https://example.com/old".into(),
            headers: Default::default(),
        }))
        .await
        .unwrap()
        .into_inner();

    let mut new_headers = std::collections::HashMap::new();
    new_headers.insert("X-Updated".into(), "yes".into());

    let updated = client
        .update_webhook(tonic::Request::new(UpdateWebhookRequest {
            site_id: site_id.clone(),
            webhook_id: created.id,
            label: Some("New Hook".into()),
            url: Some("https://example.com/new".into()),
            headers: new_headers,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(updated.label, "New Hook");
    assert_eq!(updated.url, "https://example.com/new");
    assert!(updated.headers.contains_key("X-Updated"));
}

#[tokio::test]
async fn test_delete_webhook() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = WebhookServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_webhook(tonic::Request::new(CreateWebhookRequest {
            site_id: site_id.clone(),
            label: "Delete Me".into(),
            url: "https://example.com/delete".into(),
            headers: Default::default(),
        }))
        .await
        .unwrap()
        .into_inner();

    let resp = client
        .delete_webhook(tonic::Request::new(DeleteWebhookRequest {
            site_id: site_id.clone(),
            webhook_id: created.id,
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.success);

    let result = client
        .list_webhooks(tonic::Request::new(ListWebhooksRequest {
            site_id: site_id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(result.webhooks.is_empty());
}

#[tokio::test]
async fn test_trigger_webhook() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = WebhookServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_webhook(tonic::Request::new(CreateWebhookRequest {
            site_id: site_id.clone(),
            label: "Trigger Me".into(),
            url: mock_server.uri(),
            headers: Default::default(),
        }))
        .await
        .unwrap()
        .into_inner();

    let created_id = created.id.clone();
    let resp = client
        .trigger_webhook(tonic::Request::new(TriggerWebhookRequest {
            site_id: site_id.clone(),
            webhook_id: created.id,
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(!resp.id.is_empty());
    assert_eq!(resp.webhook_id, created_id);
    assert_eq!(resp.status, "success");
}

#[tokio::test]
async fn test_list_webhook_deliveries() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = WebhookServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let created = client
        .create_webhook(tonic::Request::new(CreateWebhookRequest {
            site_id: site_id.clone(),
            label: "Deliveries".into(),
            url: mock_server.uri(),
            headers: Default::default(),
        }))
        .await
        .unwrap()
        .into_inner();

    let _triggered = client
        .trigger_webhook(tonic::Request::new(TriggerWebhookRequest {
            site_id: site_id.clone(),
            webhook_id: created.id.clone(),
        }))
        .await
        .unwrap();

    let resp = client
        .list_webhook_deliveries(tonic::Request::new(ListWebhookDeliveriesRequest {
            site_id: site_id.clone(),
            webhook_id: created.id,
            page: 1,
            per_page: 10,
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(!resp.items.is_empty());
    assert!(resp.total >= 1);
}
