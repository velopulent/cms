use std::sync::Arc;

use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::grpc::cms::admin::v1::site_service_server::SiteService;
use crate::grpc::cms::admin::v1::{
    CreateSiteRequest, DeleteResponse, DeleteSiteRequest, GetSiteRequest, ListSitesRequest, ListSitesResponse,
    Site as ProtoSite, UpdateSiteRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::middleware::auth::{SCOPE_SITES_DELETE, SCOPE_SITES_READ, SCOPE_SITES_WRITE};
use crate::models::site::Site;
use crate::repository::Repository;

#[derive(Clone)]
pub struct AdminSiteServiceImpl {
    repository: Arc<Repository>,
}

impl AdminSiteServiceImpl {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self { repository }
    }
}

#[tonic::async_trait]
impl SiteService for AdminSiteServiceImpl {
    async fn list_sites(&self, request: Request<ListSitesRequest>) -> Result<Response<ListSitesResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_SITES_READ, "sites")?;

        let sites = self
            .repository
            .site
            .list_all()
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ListSitesResponse {
            sites: sites.into_iter().map(ProtoSite::from).collect(),
        }))
    }

    async fn get_site(&self, request: Request<GetSiteRequest>) -> Result<Response<ProtoSite>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_SITES_READ, "sites")?;
        let site_id = request.into_inner().site_id;

        let site = self
            .repository
            .site
            .get_by_id(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Site not found"))?;

        Ok(Response::new(ProtoSite::from(site)))
    }

    async fn create_site(&self, request: Request<CreateSiteRequest>) -> Result<Response<ProtoSite>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_SITES_WRITE, "sites")?;
        let req = request.into_inner();

        if req.name.trim().is_empty() {
            return Err(Status::invalid_argument("Name is required"));
        }

        let storage_provider = req.default_storage_provider.as_deref().unwrap_or("filesystem");
        if storage_provider != "filesystem" && storage_provider != "s3" {
            return Err(Status::invalid_argument(
                "Invalid storage provider. Must be 'filesystem' or 's3'",
            ));
        }

        let site = self
            .repository
            .site
            .create(&Uuid::now_v7().to_string(), &req.name, storage_provider, "system")
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoSite::from(site)))
    }

    async fn update_site(&self, request: Request<UpdateSiteRequest>) -> Result<Response<ProtoSite>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_SITES_WRITE, "sites")?;
        let req = request.into_inner();

        let existing = self
            .repository
            .site
            .get_by_id(&req.site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Site not found"))?;

        let name = req.name.as_deref().unwrap_or(&existing.name);
        let storage_provider = req
            .default_storage_provider
            .as_deref()
            .unwrap_or(&existing.default_storage_provider);

        if storage_provider != "filesystem" && storage_provider != "s3" {
            return Err(Status::invalid_argument(
                "Invalid storage provider. Must be 'filesystem' or 's3'",
            ));
        }

        let site = self
            .repository
            .site
            .update(&req.site_id, name, storage_provider)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ProtoSite::from(site)))
    }

    async fn delete_site(&self, request: Request<DeleteSiteRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_SITES_DELETE, "sites")?;
        let site_id = request.into_inner().site_id;

        let deleted = self
            .repository
            .site
            .delete(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

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
            default_storage_provider: site.default_storage_provider,
            created_by: site.created_by,
            created_at: site.created_at,
            updated_at: site.updated_at,
        }
    }
}
