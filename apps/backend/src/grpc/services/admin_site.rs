use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::site_service_server::SiteService;
use crate::grpc::cms::v1::{GetSiteRequest, Site as ProtoSite, UpdateSiteRequest};
use crate::grpc::interceptor::{GrpcAuthContext, get_auth_context};
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

fn ensure_same_site(auth: &GrpcAuthContext, site_id: &str) -> Result<(), Status> {
    if auth.require_site_id()? == site_id {
        Ok(())
    } else {
        Err(Status::permission_denied("Site token does not have access to this site"))
    }
}

#[tonic::async_trait]
impl SiteService for SiteServiceImpl {
    async fn get_site(&self, mut request: Request<GetSiteRequest>) -> Result<Response<ProtoSite>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_site_scope("site:read")?;
        let site_id = request.into_inner().site_id;
        ensure_same_site(&auth, &site_id)?;

        let site = self
            .app_site_service
            .get_site(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Site not found"))?;

        Ok(Response::new(ProtoSite::from(site)))
    }

    async fn update_site(&self, mut request: Request<UpdateSiteRequest>) -> Result<Response<ProtoSite>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_site_scope("sites:write")?;
        let req = request.into_inner();
        ensure_same_site(&auth, &req.site_id)?;

        let site = self
            .app_site_service
            .update_site(&req.site_id, req.name.as_deref())
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ProtoSite::from(site)))
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
