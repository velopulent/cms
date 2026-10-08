use std::sync::Arc;

use crate::config::Config;
use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::entry_service_server::EntryService;
use crate::grpc::cms::v1::{
    CreateEntryRequest, DeleteEntryRequest, DeleteResponse, Entry as ProtoEntry, EntryRevision as ProtoEntryRevision,
    GetEntryRequest, GetEntryRevisionRequest, ListEntriesRequest, ListEntriesResponse, ListEntryRevisionsRequest,
    ListEntryRevisionsResponse, PublishEntryRequest, RestoreEntryRevisionRequest, UnpublishEntryRequest,
    UpdateEntryRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::authorization::Action;
use crate::models::entry::{Entry, EntryRevision};
use crate::repository::Repository;
use crate::repository::traits::ListEntriesParams;
use crate::services::entry::EntryService as AppEntryService;
use crate::services::entry::UpdateEntryInput;

#[derive(Clone)]
pub struct EntryServiceImpl {
    app_entry_service: Arc<AppEntryService>,
    repository: Arc<Repository>,
    config: Arc<Config>,
}

impl EntryServiceImpl {
    pub fn new(entry_service: Arc<AppEntryService>, repository: Arc<Repository>, config: Arc<Config>) -> Self {
        Self {
            app_entry_service: entry_service,
            repository,
            config,
        }
    }
}

#[tonic::async_trait]
impl EntryService for EntryServiceImpl {
    async fn list_entries(
        &self,
        mut request: Request<ListEntriesRequest>,
    ) -> Result<Response<ListEntriesResponse>, Status> {
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

        let per_page = crate::grpc::page_size(req.page_size);
        let fingerprint = crate::utils::cursor::fingerprint(&(
            &site_id,
            &req.collection_id,
            &req.status,
            &req.search,
            req.include_drafts,
            per_page,
        ));
        let page = crate::grpc::resolve_page(&req.page_token, &fingerprint, &self.config.token_index_key)?;
        let params = ListEntriesParams {
            site_id: &site_id,
            collection_slug: None,
            collection_id: req.collection_id.as_deref(),
            status: req.status.as_deref(),
            search: req.search.as_deref(),
            published_only: !req.include_drafts,
            page,
            per_page,
        };

        let result = self
            .app_entry_service
            .list_entries(params)
            .await
            .map_err(crate::grpc::service_error)?;

        let response = ListEntriesResponse {
            next_page_token: crate::grpc::next_page_token(
                result.page,
                result.per_page,
                result.total,
                fingerprint,
                &self.config.token_index_key,
            ),
            total_size: result.total,
            items: result.items.into_iter().map(ProtoEntry::from).collect(),
        };

        Ok(Response::new(response))
    }

    async fn get_entry(&self, mut request: Request<GetEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
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
        let id = req.id;

        let entry = self
            .app_entry_service
            .get_entry(&id, &site_id, !req.include_drafts)
            .await
            .map_err(crate::grpc::service_error)?
            .ok_or_else(|| Status::not_found("Entry not found"))?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn create_entry(&self, mut request: Request<CreateEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::ContentWrite)
            .await?;
        let data = crate::grpc::struct_to_json(
            req.data
                .as_ref()
                .ok_or_else(|| Status::invalid_argument("data is required"))?,
        )?;

        let entry = self
            .app_entry_service
            .create_entry(&site_id, &req.collection_id, &data, &req.slug, auth.actor.user_id())
            .await
            .map_err(crate::grpc::service_error)?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn update_entry(&self, mut request: Request<UpdateEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::ContentWrite)
            .await?;
        if req.status.is_some() {
            auth.require_action(&self.repository, &site_id, Action::ContentPublish)
                .await?;
        }
        let data = req.data.as_ref().map(crate::grpc::struct_to_json).transpose()?;

        let entry = self
            .app_entry_service
            .update_entry(UpdateEntryInput {
                id: &req.id,
                site_id: &site_id,
                data: data.as_ref(),
                slug: req.slug.as_deref(),
                status: req.status.as_deref(),
                created_by: auth.actor.user_id(),
                change_summary: req.change_summary.as_deref(),
                expected_version: if req.expected_version.is_empty() {
                    None
                } else {
                    Some(req.expected_version.as_str())
                },
            })
            .await
            .map_err(crate::grpc::service_error)?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn delete_entry(&self, mut request: Request<DeleteEntryRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::ContentWrite)
            .await?;
        let id = req.id;

        let deleted = self
            .app_entry_service
            .delete_entry(&id, &site_id)
            .await
            .map_err(crate::grpc::service_error)?;

        if deleted == 0 {
            return Err(Status::not_found("Entry not found"));
        }
        Ok(Response::new(DeleteResponse { deleted: true }))
    }

    async fn publish_entry(&self, mut request: Request<PublishEntryRequest>) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::ContentPublish)
            .await?;
        let id = req.id;

        let entry = self
            .app_entry_service
            .publish_entry(&id, &site_id)
            .await
            .map_err(crate::grpc::service_error)?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn unpublish_entry(
        &self,
        mut request: Request<UnpublishEntryRequest>,
    ) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::ContentPublish)
            .await?;
        let id = req.id;

        let entry = self
            .app_entry_service
            .unpublish_entry(&id, &site_id)
            .await
            .map_err(crate::grpc::service_error)?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }

    async fn list_entry_revisions(
        &self,
        mut request: Request<ListEntryRevisionsRequest>,
    ) -> Result<Response<ListEntryRevisionsResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::ContentPreviewRead)
            .await?;

        // Verify entry exists and belongs to site
        self.app_entry_service
            .get_entry(&req.entry_id, &site_id, false)
            .await
            .map_err(crate::grpc::service_error)?
            .ok_or_else(|| Status::not_found("Entry not found"))?;

        let per_page = crate::grpc::page_size(req.page_size);
        let fingerprint = crate::utils::cursor::fingerprint(&(&site_id, &req.entry_id, per_page));
        let page = crate::grpc::resolve_page(&req.page_token, &fingerprint, &self.config.token_index_key)?;
        let result = self
            .app_entry_service
            .list_revisions(&req.entry_id, &site_id, page, per_page)
            .await
            .map_err(crate::grpc::service_error)?;

        let response = ListEntryRevisionsResponse {
            next_page_token: crate::grpc::next_page_token(
                result.page,
                result.per_page,
                result.total,
                fingerprint,
                &self.config.token_index_key,
            ),
            total_size: result.total,
            items: result.items.into_iter().map(ProtoEntryRevision::from).collect(),
        };

        Ok(Response::new(response))
    }

    async fn get_entry_revision(
        &self,
        mut request: Request<GetEntryRevisionRequest>,
    ) -> Result<Response<ProtoEntryRevision>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::ContentPreviewRead)
            .await?;

        // Verify entry exists and belongs to site
        self.app_entry_service
            .get_entry(&req.entry_id, &site_id, false)
            .await
            .map_err(crate::grpc::service_error)?
            .ok_or_else(|| Status::not_found("Entry not found"))?;

        let revision = self
            .app_entry_service
            .get_revision(&req.entry_id, &site_id, req.revision_number)
            .await
            .map_err(crate::grpc::service_error)?
            .ok_or_else(|| Status::not_found("Revision not found"))?;

        Ok(Response::new(ProtoEntryRevision::from(revision)))
    }

    async fn restore_entry_revision(
        &self,
        mut request: Request<RestoreEntryRevisionRequest>,
    ) -> Result<Response<ProtoEntry>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::ContentWrite)
            .await?;

        self.app_entry_service
            .get_entry(&req.entry_id, &site_id, false)
            .await
            .map_err(crate::grpc::service_error)?
            .ok_or_else(|| Status::not_found("Entry not found"))?;

        let entry = self
            .app_entry_service
            .restore_revision(&req.entry_id, &site_id, req.revision_number, auth.actor.user_id())
            .await
            .map_err(crate::grpc::service_error)?;

        Ok(Response::new(ProtoEntry::from(entry)))
    }
}

impl From<Entry> for ProtoEntry {
    fn from(e: Entry) -> Self {
        ProtoEntry {
            data: crate::grpc::json_text_to_struct(&e.data),
            created_at: crate::grpc::timestamp_from_text(&e.created_at),
            updated_at: crate::grpc::timestamp_from_text(&e.updated_at),
            published_at: e.published_at.as_deref().and_then(crate::grpc::timestamp_from_text),
            id: e.id,
            site_id: e.site_id,
            collection_id: e.collection_id,
            slug: e.slug,
            status: e.status,
            singleton_collection_id: e.singleton_collection_id,
            version: e.version,
        }
    }
}

impl From<EntryRevision> for ProtoEntryRevision {
    fn from(r: EntryRevision) -> Self {
        ProtoEntryRevision {
            data: crate::grpc::json_to_struct(&r.data.0),
            created_at: crate::grpc::timestamp_from_text(&r.created_at),
            id: r.id,
            entry_id: r.entry_id,
            revision_number: r.revision_number,
            created_by: r.created_by,
            change_summary: r.change_summary,
        }
    }
}
