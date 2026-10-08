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

        let per_page = if req.page_size <= 0 {
            if req.per_page <= 0 {
                50
            } else {
                req.per_page.clamp(1, 200)
            }
        } else {
            i64::from(req.page_size).clamp(1, 200)
        };
        let fingerprint = crate::utils::cursor::fingerprint(&(
            &site_id,
            &req.collection_id,
            &req.status,
            &req.search,
            req.include_drafts,
            per_page,
        ));
        let page = if req.page_token.is_empty() {
            req.page.max(1)
        } else {
            crate::utils::cursor::decode(&req.page_token, &self.config.token_index_key)
                .map_err(|_| Status::invalid_argument("Invalid page_token"))
                .and_then(|cursor| {
                    if cursor.fingerprint == fingerprint {
                        Ok(cursor.page)
                    } else {
                        Err(Status::invalid_argument("page_token does not match this query"))
                    }
                })?
        };
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

        let has_next_page = result.page.saturating_mul(result.per_page) < result.total;
        let next_page_token = if has_next_page {
            crate::utils::cursor::encode(
                &crate::utils::cursor::PageCursor {
                    version: 1,
                    page: result.page + 1,
                    fingerprint,
                },
                &self.config.token_index_key,
            )
        } else {
            String::new()
        };
        let response = ListEntriesResponse {
            items: result.items.into_iter().map(ProtoEntry::from).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
            next_page_token,
            total_size: result.total,
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
        let data = match req.data_value.as_ref() {
            Some(value) => crate::grpc::struct_to_json(value)?,
            None => serde_json::from_str(&req.data)
                .map_err(|_| Status::invalid_argument("data must contain a JSON object"))?,
        };

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
        let data = match req.data_value.as_ref() {
            Some(value) => Some(crate::grpc::struct_to_json(value)?),
            None => req
                .data
                .as_ref()
                .map(|d| serde_json::from_str(d).map_err(|_| Status::invalid_argument("data must contain valid JSON")))
                .transpose()?,
        };

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

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "Entry deleted".to_string()
            } else {
                "Entry not found".to_string()
            },
        }))
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

        let page_val = req.page.max(1);
        let per_page_val = if req.per_page <= 0 {
            50
        } else {
            req.per_page.clamp(1, 200)
        };

        let result = self
            .app_entry_service
            .list_revisions(&req.entry_id, &site_id, page_val, per_page_val)
            .await
            .map_err(crate::grpc::service_error)?;

        let response = ListEntryRevisionsResponse {
            items: result.items.into_iter().map(ProtoEntryRevision::from).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
            next_page_token: String::new(),
            total_size: result.total,
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
        let data_value = serde_json::from_str::<serde_json::Value>(&e.data)
            .ok()
            .and_then(|value| crate::grpc::json_to_struct(&value));
        ProtoEntry {
            id: e.id,
            site_id: e.site_id,
            collection_id: e.collection_id,
            data: e.data,
            slug: e.slug,
            status: e.status,
            singleton_collection_id: e.singleton_collection_id,
            created_at: e.created_at.clone(),
            updated_at: e.updated_at.clone(),
            published_at: e.published_at.clone(),
            data_value,
            version: e.version,
            created_at_timestamp: crate::grpc::timestamp_from_text(&e.created_at),
            updated_at_timestamp: crate::grpc::timestamp_from_text(&e.updated_at),
            published_at_timestamp: e.published_at.as_deref().and_then(crate::grpc::timestamp_from_text),
        }
    }
}

impl From<EntryRevision> for ProtoEntryRevision {
    fn from(r: EntryRevision) -> Self {
        ProtoEntryRevision {
            id: r.id,
            entry_id: r.entry_id,
            revision_number: r.revision_number,
            data: serde_json::to_string(&r.data.0).unwrap_or_default(),
            created_by: r.created_by,
            created_at: r.created_at.clone(),
            change_summary: r.change_summary,
            data_value: crate::grpc::json_to_struct(&r.data.0),
            created_at_timestamp: crate::grpc::timestamp_from_text(&r.created_at),
        }
    }
}
