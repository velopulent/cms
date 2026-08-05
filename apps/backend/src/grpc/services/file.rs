use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::File as ProtoFile;
use crate::grpc::cms::v1::file_service_server::FileService;
use crate::grpc::cms::v1::{
    BatchDeleteFilesRequest, BatchOperationResponse, BatchRestoreFilesRequest, DeleteFileRequest, DeleteResponse,
    GetFileRequest, ListFilesRequest, ListFilesResponse, RestoreFileRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::models::access_token::TokenScope;
use crate::models::file::File;
use crate::repository::Repository;
use crate::repository::traits::ListFilesParams;
use crate::services::file::FileService as AppFileService;

#[derive(Clone)]
pub struct FileServiceImpl {
    app_file_service: Arc<AppFileService>,
    repository: Arc<Repository>,
}

impl FileServiceImpl {
    pub fn new(file_service: Arc<AppFileService>, repository: Arc<Repository>) -> Self {
        Self {
            app_file_service: file_service,
            repository,
        }
    }
}

#[tonic::async_trait]
impl FileService for FileServiceImpl {
    async fn list_files(&self, mut request: Request<ListFilesRequest>) -> Result<Response<ListFilesResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::FilesRead)?;
        let site_id = auth.require_site_id()?.to_string();

        let req = request.into_inner();

        let page_val = if req.page <= 0 { 1 } else { req.page };
        let per_page_val = if req.per_page <= 0 { 30 } else { req.per_page };

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
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        let response = ListFilesResponse {
            files: result.items.into_iter().map(ProtoFile::from).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        };

        Ok(Response::new(response))
    }

    async fn get_file(&self, mut request: Request<GetFileRequest>) -> Result<Response<ProtoFile>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::FilesRead)?;
        let site_id = auth.require_site_id()?.to_string();
        let id = request.into_inner().id;

        let file = self
            .app_file_service
            .get_file(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("File not found"))?;

        Ok(Response::new(ProtoFile::from(file)))
    }

    async fn delete_file(&self, mut request: Request<DeleteFileRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::FilesWrite)?;
        let site_id = auth.require_site_id()?.to_string();
        let id = request.into_inner().id;

        let deleted = self
            .app_file_service
            .soft_delete(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

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
        auth.require_scope(TokenScope::FilesWrite)?;
        let site_id = auth.require_site_id()?.to_string();
        let id = request.into_inner().id;

        let restored = self
            .app_file_service
            .restore(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        if restored == 0 {
            return Err(Status::not_found("File not found or not deleted"));
        }

        let file = self
            .app_file_service
            .get_file(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::internal("File not found after restore"))?;

        Ok(Response::new(ProtoFile::from(file)))
    }

    async fn batch_delete_files(
        &self,
        mut request: Request<BatchDeleteFilesRequest>,
    ) -> Result<Response<BatchOperationResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::FilesWrite)?;
        let site_id = auth.require_site_id()?.to_string();
        let ids = request.into_inner().ids;

        let affected = self
            .app_file_service
            .batch_soft_delete(&site_id, &ids)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(BatchOperationResponse {
            affected: affected as i64,
        }))
    }

    async fn batch_restore_files(
        &self,
        mut request: Request<BatchRestoreFilesRequest>,
    ) -> Result<Response<BatchOperationResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_scope(TokenScope::FilesWrite)?;
        let site_id = auth.require_site_id()?.to_string();
        let ids = request.into_inner().ids;

        let affected = self
            .app_file_service
            .batch_restore(&site_id, &ids)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

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
