use crate::{
    middleware::auth::{AuthContext, require_instance_action},
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
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::sync::Arc;
pub async fn list(
    auth: AuthContext,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err(v) = require_instance_action(&auth, &repo, Action::InstanceManage).await {
        return v.into_response();
    }
    match services.storage_profile.list().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":e}))).into_response(),
    }
}
pub async fn create(
    auth: AuthContext,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(registry): Extension<Arc<StorageRegistry>>,
    Json(value): Json<CreateStorageProfile>,
) -> Response {
    let user = match require_instance_action(&auth, &repo, Action::InstanceManage).await {
        Ok(v) => v,
        Err(v) => return v.into_response(),
    };
    match services.storage_profile.create(value, &user).await {
        Ok(v) => match services.storage_profile.register_all(&registry).await {
            Ok(()) => (StatusCode::CREATED, Json(v)).into_response(),
            Err(error) => (StatusCode::BAD_REQUEST, Json(json!({"error":error}))).into_response(),
        },
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error":e}))).into_response(),
    }
}
pub async fn delete(
    auth: AuthContext,
    Path(id): Path<String>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    if let Err(v) = require_instance_action(&auth, &repo, Action::InstanceManage).await {
        return v.into_response();
    }
    match services.storage_profile.delete(&id).await {
        Ok(_) => {
            registry.remove(&id);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::CONFLICT, Json(json!({"error":e}))).into_response(),
    }
}
pub async fn update(
    auth: AuthContext,
    Path(id): Path<String>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(registry): Extension<Arc<StorageRegistry>>,
    Json(value): Json<UpdateStorageProfile>,
) -> Response {
    if let Err(response) = require_instance_action(&auth, &repo, Action::InstanceManage).await {
        return response.into_response();
    }
    match services.storage_profile.update(&id, value).await {
        Ok(profile) => {
            registry.remove(&id);
            match services.storage_profile.register_all(&registry).await {
                Ok(()) => Json(profile).into_response(),
                Err(error) => (StatusCode::BAD_REQUEST, Json(json!({"error":error}))).into_response(),
            }
        }
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({"error":error}))).into_response(),
    }
}
pub async fn probe(
    auth: AuthContext,
    Path(id): Path<String>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err(response) = require_instance_action(&auth, &repo, Action::InstanceManage).await {
        return response.into_response();
    }
    match services.storage_profile.probe(&id).await {
        Ok(()) => Json(StorageProbeResult { ok: true }).into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error":"storage_probe_failed","message":error})),
        )
            .into_response(),
    }
}
