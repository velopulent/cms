use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::collection_service_server::CollectionService;
use crate::grpc::cms::v1::{
    Collection as ProtoCollection, GetCollectionRequest, ListCollectionsRequest, ListCollectionsResponse,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::authorization::Action;
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
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::SchemaRead)
            .await?;

        let collections = self
            .app_collection_service
            .list_collections(&site_id)
            .await
            .map_err(crate::grpc::service_error)?;

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
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::SchemaRead)
            .await?;
        let slug = &req.slug;

        let collection = self
            .app_collection_service
            .get_collection(&site_id, slug)
            .await
            .map_err(crate::grpc::service_error)?
            .ok_or_else(|| Status::not_found("Collection not found"))?;

        Ok(Response::new(CollectionServiceImpl::collection_to_proto(collection)))
    }
}

impl CollectionServiceImpl {
    fn collection_to_proto(c: Collection) -> ProtoCollection {
        ProtoCollection {
            definition: crate::grpc::json_text_to_struct(&c.definition),
            created_at: crate::grpc::timestamp_from_text(&c.created_at),
            updated_at: crate::grpc::timestamp_from_text(&c.updated_at),
            id: c.id,
            site_id: c.site_id,
            name: c.name,
            slug: c.slug,
            is_singleton: c.is_singleton,
        }
    }
}
