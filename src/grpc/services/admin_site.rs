use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::site_service_server::SiteService;
use crate::grpc::cms::v1::{
    CreateSiteRequest, DeleteResponse, DeleteSiteRequest, GetSiteRequest, ListSitesRequest, ListSitesResponse,
    Site as ProtoSite, UpdateSiteRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::middleware::auth::{SCOPE_SITES_DELETE, SCOPE_SITES_READ, SCOPE_SITES_WRITE};
use crate::models::site::Site;
use crate::repository::Repository;
use crate::services::site::SiteService as AppSiteService;

#[derive(Clone)]
pub struct SiteServiceImpl {
    app_site_service: Arc<AppSiteService>,
    repository: Arc<Repository>,
}

impl SiteServiceImpl {
    pub fn new(site_service: Arc<AppSiteService>, repository: Arc<Repository>) -> Self {
        Self {
            app_site_service: site_service,
            repository,
        }
    }
}

#[tonic::async_trait]
impl SiteService for SiteServiceImpl {
    async fn list_sites(&self, mut request: Request<ListSitesRequest>) -> Result<Response<ListSitesResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_instance_scope(SCOPE_SITES_READ)?;

        let sites = self
            .app_site_service
            .list_sites_instance()
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ListSitesResponse {
            sites: sites
                .into_iter()
                .filter_map(|s| {
                    let obj = s.as_object()?;
                    Some(ProtoSite {
                        id: obj["id"].as_str()?.to_string(),
                        name: obj["name"].as_str()?.to_string(),
                        storage_provider: obj["storage_provider"].as_str()?.to_string(),
                        created_by: obj["created_by"].as_str()?.to_string(),
                        created_at: obj["created_at"].as_str()?.to_string(),
                        updated_at: obj["updated_at"].as_str()?.to_string(),
                    })
                })
                .collect(),
        }))
    }

    async fn get_site(&self, mut request: Request<GetSiteRequest>) -> Result<Response<ProtoSite>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_instance_scope(SCOPE_SITES_READ)?;
        let site_id = request.into_inner().site_id;

        let site = self
            .app_site_service
            .get_site(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Site not found"))?;

        Ok(Response::new(ProtoSite::from(site)))
    }

    async fn create_site(&self, mut request: Request<CreateSiteRequest>) -> Result<Response<ProtoSite>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_instance_scope(SCOPE_SITES_WRITE)?;
        let req = request.into_inner();

        if req.name.trim().is_empty() {
            return Err(Status::invalid_argument("Name is required"));
        }

        let storage_provider = req.storage_provider.as_deref().unwrap_or("filesystem");
        if storage_provider != "filesystem" && storage_provider != "s3" {
            return Err(Status::invalid_argument(
                "Invalid storage provider. Must be 'filesystem' or 's3'",
            ));
        }

        let site = self
            .app_site_service
            .create_site(&req.name, Some(storage_provider), "system")
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoSite::from(site)))
    }

    async fn update_site(&self, mut request: Request<UpdateSiteRequest>) -> Result<Response<ProtoSite>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_instance_scope(SCOPE_SITES_WRITE)?;
        let req = request.into_inner();

        let site = self
            .app_site_service
            .update_site(&req.site_id, req.name.as_deref())
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoSite::from(site)))
    }

    async fn delete_site(&self, mut request: Request<DeleteSiteRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_instance_scope(SCOPE_SITES_DELETE)?;
        let site_id = request.into_inner().site_id;

        let deleted = self
            .app_site_service
            .delete_site(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "Site deleted".to_string()
            } else {
                "Site not found".to_string()
            },
        }))
    }
}

impl From<Site> for ProtoSite {
    fn from(site: Site) -> Self {
        Self {
            id: site.id,
            name: site.name,
            storage_provider: site.storage_provider,
            created_by: site.created_by,
            created_at: site.created_at,
            updated_at: site.updated_at,
        }
    }
}
