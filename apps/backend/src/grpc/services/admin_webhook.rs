use std::collections::HashMap;
use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::webhook_service_server::WebhookService;
use crate::grpc::cms::v1::{
    CreateWebhookRequest, DeleteWebhookRequest, GetWebhookRequest, ListWebhookDeliveriesRequest,
    ListWebhookDeliveriesResponse, ListWebhooksRequest, ListWebhooksResponse, SiteWebhook as ProtoSiteWebhook,
    TriggerWebhookRequest, UpdateWebhookRequest, WebhookDelivery as ProtoWebhookDelivery,
};
use crate::grpc::interceptor::{GrpcAuthContext, get_auth_context};
use crate::models::access_token::TokenScope;
use crate::models::webhook::WebhookDelivery;
use crate::repository::Repository;
use crate::services::webhook::WebhookService as AppWebhookService;

#[derive(Clone)]
pub struct WebhookServiceImpl {
    app_webhook_service: Arc<AppWebhookService>,
    repository: Arc<Repository>,
}

impl WebhookServiceImpl {
    pub fn new(webhook_service: Arc<AppWebhookService>, repository: Arc<Repository>) -> Self {
        Self {
            app_webhook_service: webhook_service,
            repository,
        }
    }
}

fn ensure_same_site(auth: &GrpcAuthContext, site_id: &str) -> Result<(), Status> {
    if auth.require_site_id()? == site_id {
        Ok(())
    } else {
        Err(Status::permission_denied(
            "Site token does not have access to this site",
        ))
    }
}

#[tonic::async_trait]
impl WebhookService for WebhookServiceImpl {
    async fn list_webhooks(
        &self,
        mut request: Request<ListWebhooksRequest>,
    ) -> Result<Response<ListWebhooksResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::WebhooksRead)?;
        let site_id = request.into_inner().site_id;
        ensure_same_site(&auth, &site_id)?;

        let webhooks = self
            .app_webhook_service
            .list_webhooks(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ListWebhooksResponse {
            webhooks: webhooks.into_iter().map(ProtoSiteWebhook::from).collect(),
        }))
    }

    async fn get_webhook(&self, mut request: Request<GetWebhookRequest>) -> Result<Response<ProtoSiteWebhook>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::WebhooksRead)?;
        let req = request.into_inner();
        ensure_same_site(&auth, &req.site_id)?;

        let webhook = self
            .app_webhook_service
            .get_webhook(&req.webhook_id, &req.site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Webhook not found"))?;

        let headers = self.app_webhook_service.decrypt_webhook_headers(&webhook);
        Ok(Response::new(ProtoSiteWebhook::from_model(webhook, headers)))
    }

    async fn create_webhook(
        &self,
        mut request: Request<CreateWebhookRequest>,
    ) -> Result<Response<ProtoSiteWebhook>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::WebhooksWrite)?;
        let req = request.into_inner();
        ensure_same_site(&auth, &req.site_id)?;

        let created_by = None;
        let headers: HashMap<String, String> = req.headers.into_iter().collect();

        let webhook = self
            .app_webhook_service
            .create_webhook(&req.site_id, &req.label, &req.url, &headers, created_by)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoSiteWebhook::from_model(webhook, headers)))
    }

    async fn update_webhook(
        &self,
        mut request: Request<UpdateWebhookRequest>,
    ) -> Result<Response<ProtoSiteWebhook>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::WebhooksWrite)?;
        let req = request.into_inner();
        ensure_same_site(&auth, &req.site_id)?;

        let headers: Option<HashMap<String, String>> = if req.headers.is_empty() {
            None
        } else {
            Some(req.headers.into_iter().collect())
        };

        let webhook = self
            .app_webhook_service
            .update_webhook(
                &req.webhook_id,
                &req.site_id,
                req.label.as_deref(),
                req.url.as_deref(),
                headers.as_ref(),
            )
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        let decrypted_headers = self.app_webhook_service.decrypt_webhook_headers(&webhook);
        Ok(Response::new(ProtoSiteWebhook::from_model(webhook, decrypted_headers)))
    }

    async fn delete_webhook(
        &self,
        mut request: Request<DeleteWebhookRequest>,
    ) -> Result<Response<crate::grpc::cms::v1::DeleteResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::WebhooksWrite)?;
        let req = request.into_inner();
        ensure_same_site(&auth, &req.site_id)?;

        let deleted = self
            .app_webhook_service
            .delete_webhook(&req.webhook_id, &req.site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(crate::grpc::cms::v1::DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "Webhook deleted".to_string()
            } else {
                "Webhook not found".to_string()
            },
        }))
    }

    async fn trigger_webhook(
        &self,
        mut request: Request<TriggerWebhookRequest>,
    ) -> Result<Response<ProtoWebhookDelivery>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::WebhooksTrigger)?;
        let req = request.into_inner();
        ensure_same_site(&auth, &req.site_id)?;

        let triggered_by = None;

        let delivery = self
            .app_webhook_service
            .trigger_webhook(&req.webhook_id, &req.site_id, triggered_by)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoWebhookDelivery::from(delivery)))
    }

    async fn list_webhook_deliveries(
        &self,
        mut request: Request<ListWebhookDeliveriesRequest>,
    ) -> Result<Response<ListWebhookDeliveriesResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::WebhooksRead)?;
        let req = request.into_inner();
        ensure_same_site(&auth, &req.site_id)?;

        let page = req.page.max(1);
        let per_page = req.per_page.clamp(1, 100);

        let (deliveries, total) = self
            .app_webhook_service
            .list_deliveries(&req.webhook_id, &req.site_id, page, per_page)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ListWebhookDeliveriesResponse {
            items: deliveries.into_iter().map(ProtoWebhookDelivery::from).collect(),
            total,
            page,
            per_page,
        }))
    }
}

impl ProtoSiteWebhook {
    fn from_model(model: crate::models::webhook::SiteWebhook, headers: HashMap<String, String>) -> Self {
        Self {
            id: model.id,
            site_id: model.site_id,
            label: model.label,
            url: model.url,
            headers,
            created_by: model.created_by,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl From<crate::models::webhook::SiteWebhook> for ProtoSiteWebhook {
    fn from(model: crate::models::webhook::SiteWebhook) -> Self {
        Self {
            id: model.id,
            site_id: model.site_id,
            label: model.label,
            url: model.url,
            headers: HashMap::new(),
            created_by: model.created_by,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl From<WebhookDelivery> for ProtoWebhookDelivery {
    fn from(delivery: WebhookDelivery) -> Self {
        Self {
            id: delivery.id,
            webhook_id: delivery.webhook_id,
            status: delivery.status,
            status_code: delivery.status_code,
            response_body: delivery.response_body,
            duration_ms: delivery.duration_ms,
            triggered_by: delivery.triggered_by,
            triggered_at: delivery.triggered_at,
        }
    }
}
