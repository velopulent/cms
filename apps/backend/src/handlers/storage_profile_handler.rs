use crate::{
    error::AppError,
    middleware::{
        auth::{AuthContext, require_instance_action},
        error::AuthError,
    },
    models::{
        authorization::Action,
        storage_profile::{CreateStorageProfile, StorageProbeResult, UpdateStorageProfile},
    },
    repository::Repository,
    services::Services,
    storage::StorageRegistry,
};
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
};
use std::sync::Arc;

fn map_auth_error((status, Json(error)): (StatusCode, Json<AuthError>)) -> AppError {
    match status {
        StatusCode::UNAUTHORIZED => AppError::Unauthorized(error.message),
        StatusCode::FORBIDDEN => AppError::Forbidden(error.message),
        StatusCode::NOT_FOUND => AppError::NotFound(error.message),
        _ => AppError::Internal("Authorization check failed".into()),
    }
}

fn map_storage_error(operation: &'static str, error: String) -> AppError {
    match error.as_str() {
        "name_and_bucket_required" => AppError::BadRequest("Name and bucket are required".into()),
        "invalid_endpoint" => AppError::BadRequest("Storage endpoint is invalid".into()),
        "invalid_storage_configuration" => AppError::BadRequest("Storage configuration is invalid".into()),
        "both_credentials_required" => {
            AppError::BadRequest("Access key ID and secret access key must be supplied together".into())
        }
        "profile_not_found" | "storage_profile_not_found" => AppError::NotFound("Storage profile not found".into()),
        "immutable_profile" => AppError::Conflict("Immutable storage profiles cannot be changed".into()),
        "profile_in_use" => AppError::Conflict("Storage profile is in use".into()),
        "profile_name_exists" => AppError::Conflict("A storage profile with this name already exists".into()),
        _ => {
            tracing::error!(operation, %error, "storage profile operation failed");
            AppError::Internal("Storage profile operation failed".into())
        }
    }
}

pub async fn list(
    auth: AuthContext,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Result<Json<Vec<crate::models::storage_profile::StorageProfile>>, AppError> {
    require_instance_action(&auth, &repo, Action::InstanceManage)
        .await
        .map_err(map_auth_error)?;
    services
        .storage_profile
        .list()
        .await
        .map(Json)
        .map_err(|error| map_storage_error("list", error))
}
pub async fn create(
    auth: AuthContext,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(registry): Extension<Arc<StorageRegistry>>,
    Json(value): Json<CreateStorageProfile>,
) -> Result<(StatusCode, Json<crate::models::storage_profile::StorageProfile>), AppError> {
    let user = require_instance_action(&auth, &repo, Action::InstanceManage)
        .await
        .map_err(map_auth_error)?;
    let profile = services
        .storage_profile
        .create(value, &user, &registry)
        .await
        .map_err(|error| map_storage_error("create", error))?;
    Ok((StatusCode::CREATED, Json(profile)))
}
pub async fn delete(
    auth: AuthContext,
    Path(id): Path<String>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(registry): Extension<Arc<StorageRegistry>>,
) -> Result<StatusCode, AppError> {
    require_instance_action(&auth, &repo, Action::InstanceManage)
        .await
        .map_err(map_auth_error)?;
    services
        .storage_profile
        .delete(&id)
        .await
        .map_err(|error| map_storage_error("delete", error))?;
    registry.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}
pub async fn update(
    auth: AuthContext,
    Path(id): Path<String>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(registry): Extension<Arc<StorageRegistry>>,
    Json(value): Json<UpdateStorageProfile>,
) -> Result<Json<crate::models::storage_profile::StorageProfile>, AppError> {
    require_instance_action(&auth, &repo, Action::InstanceManage)
        .await
        .map_err(map_auth_error)?;
    services
        .storage_profile
        .update(&id, value, &registry)
        .await
        .map(Json)
        .map_err(|error| map_storage_error("update", error))
}
pub async fn probe(
    auth: AuthContext,
    Path(id): Path<String>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Result<Json<StorageProbeResult>, AppError> {
    require_instance_action(&auth, &repo, Action::InstanceManage)
        .await
        .map_err(map_auth_error)?;
    match services.storage_profile.probe(&id).await {
        Ok(()) => Ok(Json(StorageProbeResult { ok: true })),
        Err(error) if error == "profile_not_found" => Err(AppError::NotFound("Storage profile not found".into())),
        Err(error) => {
            tracing::warn!(profile_id = %id, %error, "storage profile probe failed");
            Err(AppError::BadGateway("Storage probe failed".into()))
        }
    }
}
