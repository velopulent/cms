use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::entry_service_server::EntryService;
use crate::grpc::cms::v1::{
    CreateEntryRequest, DeleteEntryRequest, DeleteResponse, Entry as ProtoEntry, EntryRevision as ProtoEntryRevision,
    GetEntryRequest, GetEntryRevisionRequest, ListEntriesRequest, ListEntriesResponse,
    ListEntryRevisionsRequest, ListEntryRevisionsResponse, PublishEntryRequest, RestoreEntryRevisionRequest,
    UnpublishEntryRequest, UpdateEntryRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::entry::{Entry, EntryRevision};
use crate::repository::Repository;
use crate::repository::traits::ListEntriesParams;
use crate::services::entry::EntryService as AppEntryService;

#[derive(Clone)]
pub struct EntryServiceImpl {
    app_entry_service: Arc<AppEntryService>,
    repository: Arc<Repository>,
}

impl EntryServiceImpl {
    pub fn new(entry_service: Arc<AppEntryService>, repository: Arc<Repository>) -> Self {
        Self {
            app_entry_service: entry_service,
            repository,
        }
    }
}

#[tonic::async_trait]
impl EntryService for EntryServiceImpl {
    async fn list_entries(
        &self,
        mut request: Request<ListEntriesRequest>,
    ) -> Result<Response<ListEntriesResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
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

    async fn get_entry(&self, mut request: Request<GetEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
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

    async fn create_entry(&self, mut request: Request<CreateEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();
        let data: serde_json::Value = serde_json::from_str(&req.data).unwrap_or_default();

        let entry = self
            .app_entry_service
            .create_entry(&site_id, &req.collection_id, &data, &req.slug, None)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn update_entry(&self, mut request: Request<UpdateEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
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
                None,
                req.change_summary.as_deref(),
            )
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn delete_entry(&self, mut request: Request<DeleteEntryRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
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

    async fn publish_entry(&self, mut request: Request<PublishEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
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

    async fn unpublish_entry(&self, mut request: Request<UnpublishEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
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

    async fn list_entry_revisions(
        &self,
        mut request: Request<ListEntryRevisionsRequest>,
    ) -> Result<Response<ListEntryRevisionsResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_READ)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();

        // Verify entry exists and belongs to site
        self.app_entry_service
            .get_entry(&req.entry_id, &site_id, false)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Entry not found"))?;

        let page_val = req.page.max(1);
        let per_page_val = if req.per_page <= 0 { 50 } else { req.per_page.clamp(1, 200) };

        let result = self
            .app_entry_service
            .list_revisions(&req.entry_id, &site_id, page_val, per_page_val)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        let response = ListEntryRevisionsResponse {
            items: result.items.into_iter().map(ProtoEntryRevision::from).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        };

        Ok(Response::new(response))
    }

    async fn get_entry_revision(
        &self,
        mut request: Request<GetEntryRevisionRequest>,
    ) -> Result<Response<ProtoEntryRevision>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_READ)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();

        // Verify entry exists and belongs to site
        self.app_entry_service
            .get_entry(&req.entry_id, &site_id, false)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Entry not found"))?;

        let revision = self
            .app_entry_service
            .get_revision(&req.entry_id, &site_id, req.revision_number)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Revision not found"))?;

        Ok(Response::new(ProtoEntryRevision::from(revision)))
    }

    async fn restore_entry_revision(
        &self,
        mut request: Request<RestoreEntryRevisionRequest>,
    ) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_CONTENT_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();

        self.app_entry_service
            .get_entry(&req.entry_id, &site_id, false)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Entry not found"))?;

        let entry = self
            .app_entry_service
            .restore_revision(&req.entry_id, &site_id, req.revision_number, None)
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

impl From<EntryRevision> for ProtoEntryRevision {
    fn from(r: EntryRevision) -> Self {
        ProtoEntryRevision {
            id: r.id,
            entry_id: r.entry_id,
            revision_number: r.revision_number,
            data: serde_json::to_string(&r.data.0).unwrap_or_default(),
            created_by: r.created_by,
            created_at: r.created_at,
            change_summary: r.change_summary,
        }
    }
}
