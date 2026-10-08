use std::sync::Arc;

use futures_util::StreamExt;
use tonic::{Request, Response, Status};

use crate::config::Config;
use crate::grpc::cms::v1::File as ProtoFile;
use crate::grpc::cms::v1::file_service_server::FileService;
use crate::grpc::cms::v1::{
    DeleteFileRequest, DeleteResponse, FileReference as ProtoFileReference, GetFileRequest, ListFileReferencesRequest,
    ListFileReferencesResponse, ListFilesRequest, ListFilesResponse, RestoreFileRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::authorization::Action;
use crate::models::file::{File, FileWithUrl};
use crate::repository::Repository;
use crate::repository::traits::ListFilesParams;
use crate::services::file::FileService as AppFileService;
use crate::services::file::StreamingUploadRequest;
use crate::services::settings::SettingsService;
use crate::storage::{StorageProvider, StorageRegistry};

#[derive(Clone)]
pub struct FileServiceImpl {
    app_file_service: Arc<AppFileService>,
    repository: Arc<Repository>,
    storage_registry: Arc<StorageRegistry>,
    config: Arc<Config>,
    settings: SettingsService,
}

impl FileServiceImpl {
    pub fn new(
        file_service: Arc<AppFileService>,
        repository: Arc<Repository>,
        storage_registry: Arc<StorageRegistry>,
        config: Arc<Config>,
        settings: SettingsService,
    ) -> Self {
        Self {
            app_file_service: file_service,
            repository,
            storage_registry,
            config,
            settings,
        }
    }

    async fn storage(&self, site_id: &str) -> Result<Arc<dyn StorageProvider>, Status> {
        let profile = self
            .repository
            .file
            .get_storage_provider(site_id)
            .await
            .map_err(|_| Status::internal("Storage configuration unavailable"))?;
        self.storage_registry
            .get(&profile)
            .map(|storage| storage as Arc<dyn StorageProvider>)
            .ok_or_else(|| Status::internal("Storage provider not configured"))
    }

    fn to_proto(&self, file: &File, storage: &dyn StorageProvider) -> ProtoFile {
        ProtoFile::from(self.app_file_service.file_to_with_url(file, storage))
    }
}

#[tonic::async_trait]
impl FileService for FileServiceImpl {
    async fn list_files(&self, mut request: Request<ListFilesRequest>) -> Result<Response<ListFilesResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::FilesRead)
            .await?;

        let per_page_val = crate::grpc::page_size(req.page_size);
        let fingerprint = crate::utils::cursor::fingerprint(&(&site_id, &req.search, &req.file_type, per_page_val));
        let page_val = crate::grpc::resolve_page(&req.page_token, &fingerprint, &self.config.token_index_key)?;
        let params = ListFilesParams {
            site_id: &site_id,
            trashed: false,
            search: req.search.as_deref(),
            file_type: req.file_type.as_deref(),
            page: page_val,
            per_page: per_page_val,
        };

        let result = self
            .app_file_service
            .list_files(params)
            .await
            .map_err(crate::grpc::service_error)?;

        let storage = self.storage(&site_id).await?;
        let response = ListFilesResponse {
            next_page_token: crate::grpc::next_page_token(
                result.page,
                result.per_page,
                result.total,
                fingerprint,
                &self.config.token_index_key,
            ),
            total_size: result.total,
            files: result.items.iter().map(|file| self.to_proto(file, &*storage)).collect(),
        };

        Ok(Response::new(response))
    }

    async fn get_file(&self, mut request: Request<GetFileRequest>) -> Result<Response<ProtoFile>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::FilesRead)
            .await?;
        let id = req.id;

        let file = self
            .app_file_service
            .get_file(&id, &site_id)
            .await
            .map_err(crate::grpc::service_error)?
            .ok_or_else(|| Status::not_found("File not found"))?;

        let storage = self.storage(&site_id).await?;
        Ok(Response::new(self.to_proto(&file, &*storage)))
    }

    async fn list_file_references(
        &self,
        mut request: Request<ListFileReferencesRequest>,
    ) -> Result<Response<ListFileReferencesResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::FilesRead)
            .await?;
        self.app_file_service
            .get_file(&req.id, &site_id)
            .await
            .map_err(crate::grpc::service_error)?
            .ok_or_else(|| Status::not_found("File not found"))?;
        let references = self
            .app_file_service
            .get_file_references(&req.id, &site_id)
            .await
            .map_err(crate::grpc::service_error)?
            .into_iter()
            .map(|reference| ProtoFileReference {
                entry_id: reference.entry_id,
                collection_name: reference.collection_name,
                field_name: reference.field_name,
            })
            .collect();
        Ok(Response::new(ListFileReferencesResponse { references }))
    }

    async fn upload_file(
        &self,
        mut request: Request<tonic::Streaming<crate::grpc::cms::v1::UploadFileRequest>>,
    ) -> Result<Response<ProtoFile>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let mut stream = request.into_inner();
        let first = tokio::time::timeout(std::time::Duration::from_secs(30), stream.message())
            .await
            .map_err(|_| Status::deadline_exceeded("Upload stream idle timeout"))?
            .map_err(|_| Status::invalid_argument("Unable to read upload stream"))?
            .ok_or_else(|| Status::invalid_argument("Upload stream is empty"))?;
        let site_id = auth.resolve_site_id(&first.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::FilesWrite)
            .await?;
        if first.filename.trim().is_empty() || first.content_type.trim().is_empty() {
            return Err(Status::invalid_argument(
                "filename and content_type are required in the first upload message",
            ));
        }
        if !crate::utils::content_types::is_allowed(&first.content_type) {
            return Err(Status::invalid_argument("content_type is not allowed"));
        }
        let storage = self.storage(&site_id).await?;

        let storage_kind = self
            .repository
            .site
            .get_by_id(&site_id)
            .await
            .map_err(|_| Status::internal("Storage configuration unavailable"))?
            .ok_or_else(|| Status::not_found("Site not found"))?
            .storage_provider;
        let filename = first.filename.clone();
        let content_type = first.content_type.clone();
        let first_chunk = bytes::Bytes::from(first.chunk);
        let expected_site = site_id.clone();
        let expected_filename = filename.clone();
        let expected_type = content_type.clone();
        let remaining =
            tokio_stream::StreamExt::timeout(stream, std::time::Duration::from_secs(30)).map(move |message| {
                let message = message
                    .map_err(|_| Status::deadline_exceeded("Upload stream idle timeout"))
                    .and_then(|message| message)
                    .and_then(|request| {
                        if (!request.site_id.is_empty() && request.site_id != expected_site)
                            || (!request.filename.is_empty() && request.filename != expected_filename)
                            || (!request.content_type.is_empty() && request.content_type != expected_type)
                        {
                            Err(Status::invalid_argument("Upload metadata changed between chunks"))
                        } else {
                            Ok(bytes::Bytes::from(request.chunk))
                        }
                    });
                message.map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)
            });
        let chunks =
            futures_util::stream::once(async move { Ok::<_, Box<dyn std::error::Error + Send + Sync>>(first_chunk) })
                .chain(remaining);
        let file = self
            .app_file_service
            .upload_file_streaming(
                StreamingUploadRequest {
                    site_id: &site_id,
                    file_id: None,
                    filename: &filename,
                    content_type: &content_type,
                    created_by: auth.actor.user_id(),
                    storage,
                    storage_provider: &storage_kind,
                    max_bytes: self.settings.current().general.upload_limit_mb * 1024 * 1024,
                },
                chunks,
            )
            .await
            .map_err(crate::grpc::service_error)?;
        Ok(Response::new(ProtoFile::from(file)))
    }

    async fn delete_file(&self, mut request: Request<DeleteFileRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::FilesWrite)
            .await?;
        let id = req.id;

        let deleted = self
            .app_file_service
            .soft_delete(&id, &site_id)
            .await
            .map_err(crate::grpc::service_error)?;

        if deleted == 0 {
            return Err(Status::not_found("File not found"));
        }
        Ok(Response::new(DeleteResponse { deleted: true }))
    }

    async fn restore_file(&self, mut request: Request<RestoreFileRequest>) -> Result<Response<ProtoFile>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        let req = request.into_inner();
        let site_id = auth.resolve_site_id(&req.site_id)?;
        auth.require_action(&self.repository, &site_id, Action::FilesWrite)
            .await?;
        let id = req.id;

        let restored = self
            .app_file_service
            .restore(&id, &site_id)
            .await
            .map_err(crate::grpc::service_error)?;

        if restored == 0 {
            return Err(Status::not_found("File not found or not deleted"));
        }

        let file = self
            .app_file_service
            .get_file(&id, &site_id)
            .await
            .map_err(crate::grpc::service_error)?
            .ok_or_else(|| Status::internal("File not found after restore"))?;

        let storage = self.storage(&site_id).await?;
        Ok(Response::new(self.to_proto(&file, &*storage)))
    }
}

impl From<FileWithUrl> for ProtoFile {
    fn from(f: FileWithUrl) -> Self {
        ProtoFile {
            deleted_at: f.deleted_at.as_deref().and_then(crate::grpc::timestamp_from_text),
            created_at: crate::grpc::timestamp_from_text(&f.created_at),
            id: f.id,
            site_id: f.site_id,
            filename: f.filename,
            original_name: f.original_name,
            mime_type: f.mime_type,
            size: f.size,
            url: f.url,
            thumbnail_url: f.thumbnail_url,
            width: f.width,
            height: f.height,
            created_by: f.created_by,
        }
    }
}
