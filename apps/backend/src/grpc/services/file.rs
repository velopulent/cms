use std::sync::Arc;

use futures_util::StreamExt;
use tonic::{Request, Response, Status};

use crate::config::Config;
use crate::grpc::cms::v1::File as ProtoFile;
use crate::grpc::cms::v1::file_service_server::FileService;
use crate::grpc::cms::v1::{
    DeleteFileRequest, DeleteResponse, FileReference as ProtoFileReference, GetFileReferencesRequest, GetFileRequest,
    ListFileReferencesResponse, ListFilesRequest, ListFilesResponse, RestoreFileRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::authorization::Action;
use crate::models::file::File;
use crate::repository::Repository;
use crate::repository::traits::ListFilesParams;
use crate::services::file::FileService as AppFileService;
use crate::services::file::StreamingUploadRequest;
use crate::storage::{StorageProvider, StorageRegistry};

#[derive(Clone)]
pub struct FileServiceImpl {
    app_file_service: Arc<AppFileService>,
    repository: Arc<Repository>,
    storage_registry: Arc<StorageRegistry>,
    config: Arc<Config>,
}

impl FileServiceImpl {
    pub fn new(
        file_service: Arc<AppFileService>,
        repository: Arc<Repository>,
        storage_registry: Arc<StorageRegistry>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            app_file_service: file_service,
            repository,
            storage_registry,
            config,
        }
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

        let per_page_val = if req.page_size <= 0 {
            if req.per_page <= 0 {
                30
            } else {
                req.per_page.clamp(1, 200)
            }
        } else {
            i64::from(req.page_size).clamp(1, 200)
        };
        let fingerprint = crate::utils::cursor::fingerprint(&(&site_id, &req.search, &req.file_type, per_page_val));
        let page_val = if req.page_token.is_empty() {
            if req.page <= 0 { 1 } else { req.page }
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
        let response = ListFilesResponse {
            files: result.items.into_iter().map(ProtoFile::from).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
            next_page_token,
            total_size: result.total,
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

        Ok(Response::new(ProtoFile::from(file)))
    }

    async fn list_file_references(
        &self,
        mut request: Request<GetFileReferencesRequest>,
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
        let storage_provider = self
            .repository
            .file
            .get_storage_provider(&site_id)
            .await
            .map_err(|_| Status::internal("Storage configuration unavailable"))?;
        let storage = self
            .storage_registry
            .get(&storage_provider)
            .map(|storage| storage as Arc<dyn StorageProvider>)
            .ok_or_else(|| Status::internal("Storage provider not configured"))?;

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
                    max_bytes: self.config.max_upload_size_bytes,
                },
                chunks,
            )
            .await
            .map_err(crate::grpc::service_error)?;
        Ok(Response::new(ProtoFile::from(file_without_url(file))))
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

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "File deleted".to_string()
            } else {
                "File not found".to_string()
            },
        }))
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

        Ok(Response::new(ProtoFile::from(file)))
    }
}

fn file_without_url(file: crate::models::file::FileWithUrl) -> File {
    File {
        id: file.id,
        site_id: file.site_id,
        filename: file.filename,
        original_name: file.original_name,
        mime_type: file.mime_type,
        size: file.size,
        storage_provider: file.storage_provider,
        storage_key: file.storage_key,
        thumbnail_key: file.thumbnail_key,
        width: file.width,
        height: file.height,
        deleted_at: file.deleted_at,
        created_by: file.created_by,
        created_at: file.created_at,
    }
}

impl From<File> for ProtoFile {
    fn from(f: File) -> Self {
        ProtoFile {
            id: f.id,
            site_id: f.site_id,
            filename: f.filename,
            original_name: f.original_name,
            mime_type: f.mime_type,
            size: f.size,
            storage_provider: f.storage_provider,
            storage_key: f.storage_key,
            thumbnail_key: f.thumbnail_key,
            width: f.width,
            height: f.height,
            deleted_at: f.deleted_at,
            created_by: f.created_by,
            created_at: f.created_at.clone(),
            created_at_timestamp: crate::grpc::timestamp_from_text(&f.created_at),
        }
    }
}
