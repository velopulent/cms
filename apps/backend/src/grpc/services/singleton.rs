use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::singleton_service_server::SingletonService;
use crate::grpc::cms::v1::{
    GetSingletonRequest, ListSingletonsRequest, ListSingletonsResponse, Singleton as ProtoSingleton,
    UpdateSingletonRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::authorization::Action;
use crate::repository::Repository;
use crate::services::singleton::SingletonService as AppSingletonService;
use crate::storage::{StorageProvider, StorageRegistry};

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
    async fn list_singletons(
        &self,
        mut request: Request<ListSingletonsRequest>,
    ) -> Result<Response<ListSingletonsResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let site_id = auth.resolve_site_id(&request.get_ref().site_id)?;
        auth.require_action(
            &self.repository,
            &site_id,
            if request.get_ref().include_drafts {
                Action::ContentPreviewRead
            } else {
                Action::ContentRead
            },
        )
        .await?;
        let values = self
            .app_singleton_service
            .list_singletons_visible(&site_id, !request.get_ref().include_drafts)
            .await
            .map_err(crate::grpc::service_error)?;
        Ok(Response::new(ListSingletonsResponse {
            singletons: values.into_iter().map(ProtoSingleton::from).collect(),
        }))
    }

    async fn get_singleton(
        &self,
        mut request: Request<GetSingletonRequest>,
    ) -> Result<Response<ProtoSingleton>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(
            &self.repository,
            &site_id,
            if req.include_drafts {
                Action::ContentPreviewRead
            } else {
                Action::ContentRead
            },
        )
        .await?;
        let slug = req.slug;

        let storage_profile = self
            .repository
            .file
            .get_storage_provider(&site_id)
            .await
            .map_err(|_| Status::internal("Storage configuration unavailable"))?;
        let storage = self
            .storage_registry
            .get(&storage_profile)
            .map(|s| s as Arc<dyn StorageProvider>)
            .ok_or_else(|| Status::internal("Storage provider not configured"))?;

        let singleton = self
            .app_singleton_service
            .get_singleton_visible(&site_id, &slug, storage, !req.include_drafts)
            .await
            .map_err(crate::grpc::service_error)?;

        Ok(Response::new(ProtoSingleton::from(singleton)))
    }

    async fn update_singleton(
        &self,
        mut request: Request<UpdateSingletonRequest>,
    ) -> Result<Response<ProtoSingleton>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::ContentWrite)
            .await?;

        let data = match req.data_value.as_ref() {
            Some(value) => crate::grpc::struct_to_json(value)?,
            None => {
                serde_json::from_str(&req.data).map_err(|_| Status::invalid_argument("data must contain valid JSON"))?
            }
        };

        let singleton = self
            .app_singleton_service
            .update_singleton(
                &site_id,
                &req.slug,
                &data,
                auth.actor.user_id(),
                req.change_summary.as_deref(),
                (!req.expected_version.is_empty()).then_some(req.expected_version.as_str()),
            )
            .await
            .map_err(crate::grpc::service_error)?;

        Ok(Response::new(ProtoSingleton::from(singleton)))
    }
}

impl From<crate::models::collection::SingletonResponse> for ProtoSingleton {
    fn from(c: crate::models::collection::SingletonResponse) -> Self {
        let definition_value = crate::grpc::json_to_struct(&c.definition);
        let data_value = c.data.as_ref().and_then(crate::grpc::json_to_struct);
        ProtoSingleton {
            id: c.id,
            site_id: c.site_id,
            name: c.name,
            slug: c.slug,
            definition: c.definition.to_string(),
            data: c.data.map(|d| d.to_string()),
            entry_id: c.entry_id,
            created_at: c.created_at.clone(),
            updated_at: c.updated_at.clone(),
            definition_value,
            data_value,
            version: c.version,
            created_at_timestamp: crate::grpc::timestamp_from_text(&c.created_at),
            updated_at_timestamp: crate::grpc::timestamp_from_text(&c.updated_at),
        }
    }
}
