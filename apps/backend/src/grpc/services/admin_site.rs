use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::site_service_server::SiteService;
use crate::grpc::cms::v1::{GetSiteRequest, ListSitesRequest, ListSitesResponse, Site as ProtoSite};
use crate::grpc::interceptor::get_auth_context;
use crate::models::authorization::Action;
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
        if !auth.scopes.contains(&crate::models::access_token::TokenScope::SiteRead) {
            return Err(Status::permission_denied("Token scope does not permit this operation"));
        }
        let sites = self
            .app_site_service
            .list_sites_for_actor(&auth.actor)
            .await
            .map_err(crate::grpc::service_error)?
            .into_iter()
            .filter_map(|site| {
                Some(ProtoSite {
                    id: site.get("id")?.as_str()?.to_owned(),
                    name: site.get("name")?.as_str()?.to_owned(),
                    storage_provider: site
                        .get("storage_provider")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_owned(),
                    created_by: site
                        .get("created_by")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_owned(),
                    created_at: site.get("created_at")?.as_str()?.to_owned(),
                    updated_at: site.get("updated_at")?.as_str()?.to_owned(),
                    created_at_timestamp: site
                        .get("created_at")
                        .and_then(|value| value.as_str())
                        .and_then(crate::grpc::timestamp_from_text),
                    updated_at_timestamp: site
                        .get("updated_at")
                        .and_then(|value| value.as_str())
                        .and_then(crate::grpc::timestamp_from_text),
                })
            })
            .collect();
        Ok(Response::new(ListSitesResponse { sites }))
    }

    async fn get_site(&self, mut request: Request<GetSiteRequest>) -> Result<Response<ProtoSite>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let site_id = request.into_inner().site_id;
        let site_id = auth.resolve_site_id(&site_id)?;
        auth.require_action(&self.repository, &site_id, Action::SiteRead)
            .await?;

        let site = self
            .app_site_service
            .get_site(&site_id)
            .await
            .map_err(crate::grpc::service_error)?
            .ok_or_else(|| Status::not_found("Site not found"))?;

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
            created_at: site.created_at.clone(),
            updated_at: site.updated_at.clone(),
            created_at_timestamp: crate::grpc::timestamp_from_text(&site.created_at),
            updated_at_timestamp: crate::grpc::timestamp_from_text(&site.updated_at),
        }
    }
}
