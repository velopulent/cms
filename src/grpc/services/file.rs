use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::file_service_server::FileService;
use crate::grpc::cms::v1::{
    BatchDeleteFilesRequest, BatchOperationResponse, BatchRestoreFilesRequest, DeleteFileRequest, DeleteResponse,
    File as ProtoFile, GetFileRequest, ListFilesRequest, ListFilesResponse, RestoreFileRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::file::File;
use crate::repository::Repository;
use crate::repository::traits::ListFilesParams;

#[derive(Clone)]
pub struct FileServiceImpl {
    repository: Arc<Repository>,
}

impl FileServiceImpl {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self { repository }
    }
}

#[tonic::async_trait]
impl FileService for FileServiceImpl {
    async fn list_files(&self, request: Request<ListFilesRequest>) -> Result<Response<ListFilesResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_ASSETS_READ)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();

        let params = ListFilesParams {
            site_id: &site_id,
            trashed: false,
            search: req.search.as_deref(),
            file_type: req.file_type.as_deref(),
            page: req.page,
            per_page: req.per_page,
        };

        let result = self
            .repository
            .file
            .list(params)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let response = ListFilesResponse {
            files: result.items.into_iter().map(ProtoFile::from).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        };

        Ok(Response::new(response))
    }

    async fn get_file(&self, request: Request<GetFileRequest>) -> Result<Response<ProtoFile>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_ASSETS_READ)?;
        let site_id = auth.require_site_id()?.to_string();
        let id = request.into_inner().id;

        let file = self
            .repository
            .file
            .get_by_id(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("File not found"))?;

        Ok(Response::new(ProtoFile::from(file)))
    }

    async fn delete_file(&self, request: Request<DeleteFileRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_ASSETS_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();
        let id = request.into_inner().id;

        let deleted = self
            .repository
            .file
            .soft_delete(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "File deleted".to_string()
            } else {
                "File not found".to_string()
            },
        }))
    }

    async fn restore_file(&self, request: Request<RestoreFileRequest>) -> Result<Response<ProtoFile>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_ASSETS_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();
        let id = request.into_inner().id;

        let restored = self
            .repository
            .file
            .restore(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        if restored == 0 {
            return Err(Status::not_found("File not found or not deleted"));
        }

        let file = self
            .repository
            .file
            .get_by_id(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::internal("File not found after restore"))?;

        Ok(Response::new(ProtoFile::from(file)))
    }

    async fn batch_delete_files(
        &self,
        request: Request<BatchDeleteFilesRequest>,
    ) -> Result<Response<BatchOperationResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_ASSETS_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();
        let ids = request.into_inner().ids;

        let affected = self
            .repository
            .file
            .batch_soft_delete(&site_id, &ids)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(BatchOperationResponse {
            affected: affected as i64,
        }))
    }

    async fn batch_restore_files(
        &self,
        request: Request<BatchRestoreFilesRequest>,
    ) -> Result<Response<BatchOperationResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_site_scope(crate::middleware::auth::SCOPE_ASSETS_WRITE)?;
        let site_id = auth.require_site_id()?.to_string();
        let ids = request.into_inner().ids;

        let affected = self
            .repository
            .file
            .batch_restore(&site_id, &ids)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(BatchOperationResponse {
            affected: affected as i64,
        }))
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
            created_at: f.created_at,
        }
    }
}
