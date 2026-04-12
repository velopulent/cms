use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::site::v1::singleton_service_server::SingletonService;
use crate::grpc::cms::site::v1::{GetSingletonRequest, Singleton as ProtoSingleton, UpdateSingletonRequest};
use crate::grpc::interceptor::get_auth_context;
use crate::models::collection::Collection;
use crate::repository::Repository;

#[derive(Clone)]
pub struct SingletonServiceImpl {
    repository: Arc<Repository>,
}

impl SingletonServiceImpl {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self { repository }
    }
}

#[tonic::async_trait]
impl SingletonService for SingletonServiceImpl {
    async fn get_singleton(&self, request: Request<GetSingletonRequest>) -> Result<Response<ProtoSingleton>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(crate::middleware::auth::SCOPE_CONTENT_READ, "singletons")?;
        let site_id = auth.require_site_id()?.to_string();
        let slug = request.into_inner().slug;

        let collection = self
            .repository
            .collection
            .get_by_slug(&site_id, &slug)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Singleton not found"))?;

        if !collection.is_singleton {
            return Err(Status::not_found("Collection is not a singleton"));
        }

        Ok(Response::new(ProtoSingleton::from(collection)))
    }

    async fn update_singleton(
        &self,
        request: Request<UpdateSingletonRequest>,
    ) -> Result<Response<ProtoSingleton>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(crate::middleware::auth::SCOPE_CONTENT_WRITE, "singletons")?;
        let site_id = auth.require_site_id()?.to_string();
        let req = request.into_inner();

        let collection = self
            .repository
            .collection
            .get_by_slug(&site_id, &req.slug)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Singleton not found"))?;

        if !collection.is_singleton {
            return Err(Status::not_found("Collection is not a singleton"));
        }

        let updated = self
            .repository
            .collection
            .update_singleton_data(&collection.id, &req.data)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoSingleton::from(updated)))
    }
}

impl From<Collection> for ProtoSingleton {
    fn from(c: Collection) -> Self {
        ProtoSingleton {
            id: c.id,
            site_id: c.site_id,
            name: c.name,
            slug: c.slug,
            definition: c.definition,
            data: c.singleton_data,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}
