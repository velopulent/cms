use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::collection_service_server::CollectionService;
use crate::grpc::cms::v1::{
    Collection as ProtoCollection, CreateCollectionRequest, DeleteCollectionRequest, DeleteResponse,
    GetCollectionRequest, ListCollectionsRequest, ListCollectionsResponse, UpdateCollectionRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::access_token::TokenScope;
use crate::models::collection::Collection;
use crate::repository::Repository;
use crate::services::collection::CollectionService as AppCollectionService;

#[derive(Clone)]
pub struct CollectionServiceImpl {
    app_collection_service: Arc<AppCollectionService>,
    repository: Arc<Repository>,
}

impl CollectionServiceImpl {
    pub fn new(collection_service: Arc<AppCollectionService>, repository: Arc<Repository>) -> Self {
        Self {
            app_collection_service: collection_service,
            repository,
        }
    }
}

#[tonic::async_trait]
impl CollectionService for CollectionServiceImpl {
    async fn list_collections(
        &self,
        mut request: Request<ListCollectionsRequest>,
    ) -> Result<Response<ListCollectionsResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::SchemaRead)?;
        let site_id = auth.require_site_id()?.to_string();

        let collections = self
            .app_collection_service
            .list_collections(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        let response = ListCollectionsResponse {
            collections: collections
                .into_iter()
                .map(CollectionServiceImpl::collection_to_proto)
                .collect(),
        };

        Ok(Response::new(response))
    }

    async fn get_collection(
        &self,
        mut request: Request<GetCollectionRequest>,
    ) -> Result<Response<ProtoCollection>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::SchemaRead)?;
        let site_id = auth.require_site_id()?.to_string();
        let slug = &request.into_inner().slug;

        let collection = self
            .app_collection_service
            .get_collection(&site_id, slug)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Collection not found"))?;

        Ok(Response::new(CollectionServiceImpl::collection_to_proto(collection)))
    }

    async fn create_collection(
        &self,
        mut request: Request<CreateCollectionRequest>,
    ) -> Result<Response<ProtoCollection>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::SchemaWrite)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();

        let definition = if req.definition.is_empty() {
            "{}".to_string()
        } else {
            req.definition
        };

        let collection = self
            .app_collection_service
            .create_collection(&site_id, &req.name, &req.slug, &definition, req.is_singleton)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(CollectionServiceImpl::collection_to_proto(collection)))
    }

    async fn update_collection(
        &self,
        mut request: Request<UpdateCollectionRequest>,
    ) -> Result<Response<ProtoCollection>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::SchemaWrite)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();

        let existing = self
            .app_collection_service
            .get_by_id(&req.id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Collection not found"))?;

        let collection = self
            .app_collection_service
            .update_collection(
                &site_id,
                &existing.slug,
                req.name.as_deref(),
                req.slug.as_deref(),
                req.definition.as_deref(),
            )
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(CollectionServiceImpl::collection_to_proto(collection)))
    }

    async fn delete_collection(
        &self,
        mut request: Request<DeleteCollectionRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::SchemaWrite)?;
        let site_id = auth.require_site_id()?.to_string();
        let req = request.into_inner();

        let deleted = self
            .app_collection_service
            .delete_collection(&site_id, &req.slug)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "Collection deleted".to_string()
            } else {
                "Collection not found".to_string()
            },
        }))
    }
}

impl CollectionServiceImpl {
    fn collection_to_proto(c: Collection) -> ProtoCollection {
        ProtoCollection {
            id: c.id,
            site_id: c.site_id,
            name: c.name,
            slug: c.slug,
            definition: c.definition,
            is_singleton: c.is_singleton,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}
