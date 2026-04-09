use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::collection_service_server::CollectionService;
use crate::grpc::cms::v1::{
    DeleteResponse, Collection as ProtoCollection,
    ListCollectionsRequest, ListCollectionsResponse, GetCollectionRequest,
    CreateCollectionRequest, UpdateCollectionRequest, DeleteCollectionRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::collection::Collection;
use crate::repository::Repository;
use uuid::Uuid;

#[derive(Clone)]
pub struct CollectionServiceImpl {
    repository: Arc<Repository>,
}

impl CollectionServiceImpl {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self { repository }
    }
}

#[tonic::async_trait]
impl CollectionService for CollectionServiceImpl {
    async fn list_collections(
        &self,
        request: Request<ListCollectionsRequest>,
    ) -> Result<Response<ListCollectionsResponse>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;

        let collections = self
            .repository
            .collection
            .list(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let response = ListCollectionsResponse {
            collections: collections.into_iter().map(ProtoCollection::from).collect(),
        };

        Ok(Response::new(response))
    }

    async fn get_collection(
        &self,
        request: Request<GetCollectionRequest>,
    ) -> Result<Response<ProtoCollection>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let slug = &request.into_inner().slug;

        let collection = self
            .repository
            .collection
            .get_by_slug(&site_id, slug)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Collection not found"))?;

        Ok(Response::new(ProtoCollection::from(collection)))
    }

    async fn create_collection(
        &self,
        request: Request<CreateCollectionRequest>,
    ) -> Result<Response<ProtoCollection>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;

        let req = request.into_inner();

        let id = Uuid::now_v7().to_string();
        let definition = if req.definition.is_empty() {
            "{}".to_string()
        } else {
            req.definition
        };

        let collection = self
            .repository
            .collection
            .create(&id, &site_id, &req.name, &req.slug, &definition, req.is_singleton)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoCollection::from(collection)))
    }

    async fn update_collection(
        &self,
        request: Request<UpdateCollectionRequest>,
    ) -> Result<Response<ProtoCollection>, Status> {
        let _auth = get_auth_context(&request)?;

        let req = request.into_inner();

        let existing = self
            .repository
            .collection
            .get_by_id(&req.id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Collection not found"))?;

        let collection = self
            .repository
            .collection
            .update(
                &req.id,
                req.name.as_ref().unwrap_or(&existing.name),
                req.slug.as_ref().unwrap_or(&existing.slug),
                req.definition.as_ref().unwrap_or(&existing.definition),
            )
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoCollection::from(collection)))
    }

    async fn delete_collection(
        &self,
        request: Request<DeleteCollectionRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let req = request.into_inner();

        let deleted = self
            .repository
            .collection
            .delete(&site_id, &req.slug)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

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

impl From<Collection> for ProtoCollection {
    fn from(c: Collection) -> Self {
        ProtoCollection {
            id: c.id,
            site_id: c.site_id,
            name: c.name,
            slug: c.slug,
            definition: c.definition,
            is_singleton: c.is_singleton,
            singleton_data: c.singleton_data,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}
