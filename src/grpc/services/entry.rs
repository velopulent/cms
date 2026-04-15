use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::entry_service_server::EntryService;
use crate::grpc::cms::v1::{
    CreateEntryRequest, DeleteEntryRequest, DeleteResponse, Entry as ProtoEntry, GetEntryRequest, ListEntriesRequest,
    ListEntriesResponse, PublishEntryRequest, UnpublishEntryRequest, UpdateEntryRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::entry::Entry;
use crate::repository::traits::ListEntriesParams;
use crate::services::entry::EntryService as AppEntryService;

#[derive(Clone)]
pub struct EntryServiceImpl {
    app_entry_service: Arc<AppEntryService>,
}

impl EntryServiceImpl {
    pub fn new(entry_service: Arc<AppEntryService>) -> Self {
        Self {
            app_entry_service: entry_service,
        }
    }
}

#[tonic::async_trait]
impl EntryService for EntryServiceImpl {
    async fn list_entries(
        &self,
        request: Request<ListEntriesRequest>,
    ) -> Result<Response<ListEntriesResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_READ)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();

        let params = ListEntriesParams {
            site_id: &site_id,
            collection_slug: None,
            collection_id: req.collection_id.as_deref(),
            status: req.status.as_deref(),
            search: req.search.as_deref(),
            published_only: false,
            page: req.page,
            per_page: req.per_page,
        };

        let result = self
            .app_entry_service
            .list_entries(params)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        let response = ListEntriesResponse {
            items: result.items.into_iter().map(ProtoEntry::from).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        };

        Ok(Response::new(response))
    }

    async fn get_entry(&self, request: Request<GetEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_READ)?;
        let site_id = auth.require_site_id()?.to_string();
        let id = request.into_inner().id;

        let entry = self
            .app_entry_service
            .get_entry(&id, &site_id, false)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Entry not found"))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn create_entry(&self, request: Request<CreateEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();
        let data: serde_json::Value = serde_json::from_str(&req.data).unwrap_or_default();

        let entry = self
            .app_entry_service
            .create_entry(&site_id, &req.collection_id, &data, &req.slug)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn update_entry(&self, request: Request<UpdateEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();
        let data: Option<serde_json::Value> = req.data.as_ref().map(|d| serde_json::from_str(d).unwrap_or_default());

        let entry = self
            .app_entry_service
            .update_entry(
                &req.id,
                &site_id,
                data.as_ref(),
                req.slug.as_deref(),
                req.status.as_deref(),
            )
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn delete_entry(&self, request: Request<DeleteEntryRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();
        let id = request.into_inner().id;

        let deleted = self
            .app_entry_service
            .delete_entry(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "Entry deleted".to_string()
            } else {
                "Entry not found".to_string()
            },
        }))
    }

    async fn publish_entry(&self, request: Request<PublishEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();
        let id = request.into_inner().id;

        let entry = self
            .app_entry_service
            .publish_entry(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn unpublish_entry(&self, request: Request<UnpublishEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();
        let id = request.into_inner().id;

        let entry = self
            .app_entry_service
            .unpublish_entry(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }
}

impl From<Entry> for ProtoEntry {
    fn from(e: Entry) -> Self {
        ProtoEntry {
            id: e.id,
            site_id: e.site_id,
            collection_id: e.collection_id,
            data: e.data,
            slug: e.slug,
            status: e.status,
            created_at: e.created_at,
            updated_at: e.updated_at,
            published_at: e.published_at,
        }
    }
}
