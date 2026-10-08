use async_graphql::{Context, Object, Result};

use crate::graphql::context::GqlContext;
use crate::graphql::types::file::FileUploadUrl;

pub struct FileMutation;

#[Object]
impl FileMutation {
    pub async fn create_file_upload(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        filename: String,
        content_type: String,
    ) -> Result<FileUploadUrl> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::FilesWrite)
            .await?;
        crate::services::file::FileService::validate_filename(&filename)
            .map_err(|error| crate::graphql::service_error("mutation.create_file_upload", error))?;
        if !crate::utils::content_types::is_allowed(&content_type) {
            return Err(async_graphql::Error::new("Content type is not allowed"));
        }
        let site = gql_ctx
            .services
            .site
            .get_site(&site_id)
            .await
            .map_err(|error| crate::graphql::service_error("mutation.create_file_upload", error))?
            .ok_or_else(|| async_graphql::Error::new("Site not found"))?;
        let profile = site
            .storage_profile_id
            .ok_or_else(|| async_graphql::Error::new("Storage profile not configured"))?;
        let (token, upload_path) = crate::signed_upload::SignedUploadToken::generate_with_storage_profile(
            &site_id,
            &filename,
            &content_type,
            &profile,
            &gql_ctx.config.signed_upload_key,
            gql_ctx.config.upload_token_expiry_secs,
        );
        let expires_at = token.expires_at();
        let base = gql_ctx
            .config
            .public_url
            .clone()
            .unwrap_or_else(|| format!("http://{}", gql_ctx.config.bind_address));
        Ok(FileUploadUrl {
            upload_url: format!("{}/api/v1/files/upload/{}", base.trim_end_matches('/'), upload_path),
            file_id: token.file_id,
            expires_at,
            method: "PUT".into(),
            content_type,
        })
    }

    pub async fn delete_file(&self, ctx: &Context<'_>, site_id: String, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::FilesWrite)
            .await?;

        gql_ctx
            .services
            .file
            .soft_delete(&id, &site_id)
            .await
            .map_err(|e| crate::graphql::service_error("mutation.delete_file", e))?;

        Ok(true)
    }

    pub async fn restore_file(&self, ctx: &Context<'_>, site_id: String, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::FilesWrite)
            .await?;

        gql_ctx
            .services
            .file
            .restore(&id, &site_id)
            .await
            .map_err(|e| crate::graphql::service_error("mutation.restore_file", e))?;

        Ok(true)
    }

    pub async fn batch_delete_files(&self, ctx: &Context<'_>, site_id: String, ids: Vec<String>) -> Result<i64> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::FilesWrite)
            .await?;

        let count = gql_ctx
            .services
            .file
            .batch_soft_delete(&site_id, &ids)
            .await
            .map_err(|e| crate::graphql::service_error("mutation.batch_delete_files", e))?;

        Ok(count as i64)
    }

    pub async fn batch_restore_files(&self, ctx: &Context<'_>, site_id: String, ids: Vec<String>) -> Result<i64> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::FilesWrite)
            .await?;

        let count = gql_ctx
            .services
            .file
            .batch_restore(&site_id, &ids)
            .await
            .map_err(|e| crate::graphql::service_error("mutation.batch_restore_files", e))?;

        Ok(count as i64)
    }
}
