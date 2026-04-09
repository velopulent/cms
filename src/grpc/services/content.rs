use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::content_service_server::ContentService;
use crate::grpc::cms::v1::{
    DeleteResponse, Content as ProtoContent,
    ListContentRequest, ListContentResponse, GetContentRequest,
    CreateContentRequest, UpdateContentRequest, DeleteContentRequest,
    PublishContentRequest, UnpublishContentRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::content::Content;
use crate::repository::traits::ListContentParams;
use crate::repository::Repository;
use uuid::Uuid;

#[derive(Clone)]
pub struct ContentServiceImpl {
    repository: Arc<Repository>,
}

impl ContentServiceImpl {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self { repository }
    }
}

#[tonic::async_trait]
impl ContentService for ContentServiceImpl {
    async fn list_content(
        &self,
        request: Request<ListContentRequest>,
    ) -> Result<Response<ListContentResponse>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;

        let req = request.into_inner();

        let params = ListContentParams {
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
            .content
            .list(params)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let response = ListContentResponse {
            items: result.items.into_iter().map(ProtoContent::from).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        };

        Ok(Response::new(response))
    }

    async fn get_content(
        &self,
        request: Request<GetContentRequest>,
    ) -> Result<Response<ProtoContent>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let id = request.into_inner().id;

        let content = self
            .repository
            .content
            .get_by_id(&id, &site_id, false)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Content not found"))?;

        Ok(Response::new(ProtoContent::from(content)))
    }

    async fn create_content(
        &self,
        request: Request<CreateContentRequest>,
    ) -> Result<Response<ProtoContent>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;

        let req = request.into_inner();
        let id = Uuid::now_v7().to_string();

        let content = self
            .repository
            .content
            .create(&id, &site_id, &req.collection_id, &req.data, &req.slug)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoContent::from(content)))
    }

    async fn update_content(
        &self,
        request: Request<UpdateContentRequest>,
    ) -> Result<Response<ProtoContent>, Status> {
        let _auth = get_auth_context(&request)?;

        let req = request.into_inner();

        let existing = self
            .repository
            .content
            .get_by_id_any_site(&req.id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Content not found"))?;

        let content = self
            .repository
            .content
            .update(
                &req.id,
                req.data.as_ref().unwrap_or(&existing.data),
                req.slug.as_ref().unwrap_or(&existing.slug),
                req.status.as_ref().unwrap_or(&existing.status),
            )
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoContent::from(content)))
    }

    async fn delete_content(
        &self,
        request: Request<DeleteContentRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let id = request.into_inner().id;

        let deleted = self
            .repository
            .content
            .delete(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "Content deleted".to_string()
            } else {
                "Content not found".to_string()
            },
        }))
    }

    async fn publish_content(
        &self,
        request: Request<PublishContentRequest>,
    ) -> Result<Response<ProtoContent>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let id = request.into_inner().id;

        let content = self
            .repository
            .content
            .publish(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoContent::from(content)))
    }

    async fn unpublish_content(
        &self,
        request: Request<UnpublishContentRequest>,
    ) -> Result<Response<ProtoContent>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let id = request.into_inner().id;

        let content = self
            .repository
            .content
            .unpublish(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoContent::from(content)))
    }
}

impl From<Content> for ProtoContent {
    fn from(c: Content) -> Self {
        ProtoContent {
            id: c.id,
            site_id: c.site_id,
            collection_id: c.collection_id,
            data: c.data,
            slug: c.slug,
            status: c.status,
            created_at: c.created_at,
            updated_at: c.updated_at,
            published_at: c.published_at,
        }
    }
}
