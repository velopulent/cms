use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::entry_service_server::EntryService;
use crate::grpc::cms::v1::{
    DeleteResponse, Entry as ProtoEntry,
    ListEntriesRequest, ListEntriesResponse, GetEntryRequest,
    CreateEntryRequest, UpdateEntryRequest, DeleteEntryRequest,
    PublishEntryRequest, UnpublishEntryRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::entry::Entry;
use crate::repository::traits::ListEntriesParams;
use crate::repository::Repository;
use uuid::Uuid;

#[derive(Clone)]
pub struct EntryServiceImpl {
    repository: Arc<Repository>,
}

impl EntryServiceImpl {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self { repository }
    }
}

#[tonic::async_trait]
impl EntryService for EntryServiceImpl {
    async fn list_entries(
        &self,
        request: Request<ListEntriesRequest>,
    ) -> Result<Response<ListEntriesResponse>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;

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
            .repository
            .entry
            .list(params)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let response = ListEntriesResponse {
            items: result.items.into_iter().map(ProtoEntry::from).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        };

        Ok(Response::new(response))
    }

    async fn get_entry(
        &self,
        request: Request<GetEntryRequest>,
    ) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let id = request.into_inner().id;

        let entry = self
            .repository
            .entry
            .get_by_id(&id, &site_id, false)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Entry not found"))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn create_entry(
        &self,
        request: Request<CreateEntryRequest>,
    ) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;

        let req = request.into_inner();
        let id = Uuid::now_v7().to_string();

        let entry = self
            .repository
            .entry
            .create(&id, &site_id, &req.collection_id, &req.data, &req.slug)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn update_entry(
        &self,
        request: Request<UpdateEntryRequest>,
    ) -> Result<Response<ProtoEntry>, Status> {
        let _auth = get_auth_context(&request)?;

        let req = request.into_inner();

        let existing = self
            .repository
            .entry
            .get_by_id_any_site(&req.id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Entry not found"))?;

        let entry = self
            .repository
            .entry
            .update(
                &req.id,
                req.data.as_ref().unwrap_or(&existing.data),
                req.slug.as_ref().unwrap_or(&existing.slug),
                req.status.as_ref().unwrap_or(&existing.status),
            )
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn delete_entry(
        &self,
        request: Request<DeleteEntryRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let id = request.into_inner().id;

        let deleted = self
            .repository
            .entry
            .delete(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "Entry deleted".to_string()
            } else {
                "Entry not found".to_string()
            },
        }))
    }

    async fn publish_entry(
        &self,
        request: Request<PublishEntryRequest>,
    ) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let id = request.into_inner().id;

        let entry = self
            .repository
            .entry
            .publish(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn unpublish_entry(
        &self,
        request: Request<UnpublishEntryRequest>,
    ) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let id = request.into_inner().id;

        let entry = self
            .repository
            .entry
            .unpublish(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

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
