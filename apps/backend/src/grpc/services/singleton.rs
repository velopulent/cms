use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::singleton_service_server::SingletonService;
use crate::grpc::cms::v1::{GetSingletonRequest, Singleton as ProtoSingleton, UpdateSingletonRequest};
use crate::grpc::interceptor::get_auth_context;
use crate::repository::Repository;
use crate::services::singleton::SingletonService as AppSingletonService;
use crate::storage::{STORAGE_KIND_FILESYSTEM, StorageProvider, StorageRegistry};

#[derive(Clone)]
pub struct SingletonServiceImpl {
    app_singleton_service: Arc<AppSingletonService>,
    storage_registry: Arc<StorageRegistry>,
    repository: Arc<Repository>,
}

impl SingletonServiceImpl {
    pub fn new(
        singleton_service: Arc<AppSingletonService>,
        storage_registry: Arc<StorageRegistry>,
        repository: Arc<Repository>,
    ) -> Self {
        Self {
            app_singleton_service: singleton_service,
            storage_registry,
            repository,
        }
    }
}

#[tonic::async_trait]
impl SingletonService for SingletonServiceImpl {
    async fn get_singleton(
        &self,
        mut request: Request<GetSingletonRequest>,
    ) -> Result<Response<ProtoSingleton>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_read()?;
        let site_id = auth.require_site_id()?.to_string();
        let slug = request.into_inner().slug;

        let storage = self
            .storage_registry
            .get(STORAGE_KIND_FILESYSTEM)
            .map(|s| s as Arc<dyn StorageProvider>)
            .ok_or_else(|| Status::internal("Filesystem storage not configured"))?;

        let singleton = self
            .app_singleton_service
            .get_singleton(&site_id, &slug, storage)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoSingleton::from(singleton)))
    }

    async fn update_singleton(
        &self,
        mut request: Request<UpdateSingletonRequest>,
    ) -> Result<Response<ProtoSingleton>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_write()?;
        let site_id = auth.require_site_id()?.to_string();
        let req = request.into_inner();

        let data: serde_json::Value = serde_json::from_str(&req.data).unwrap_or_default();

        let singleton = self
            .app_singleton_service
            .update_singleton(&site_id, &req.slug, &data, None, req.change_summary.as_deref())
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoSingleton::from(singleton)))
    }
}

impl From<crate::models::collection::SingletonResponse> for ProtoSingleton {
    fn from(c: crate::models::collection::SingletonResponse) -> Self {
        ProtoSingleton {
            id: c.id,
            site_id: c.site_id,
            name: c.name,
            slug: c.slug,
            definition: c.definition.to_string(),
            data: c.data.map(|d| d.to_string()),
            entry_id: c.entry_id,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}
