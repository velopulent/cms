use std::sync::Arc;

use axum::{
    Json,
    extract::Extension,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::Config;
use crate::database::pool::DbPool;
use crate::error::AppError;
use crate::middleware::auth::{AuthContext, require_instance_action};
use crate::models::authorization::Action;
use crate::repository::Repository;
use crate::services::backup::BackupService;
use crate::services::settings::{
    BackupSettings, CredentialPair, EncryptedCredentials, GeneralSettings, InstanceSettings, SecuritySettings,
    SettingsService, StorageSettings,
};
use crate::storage::{FileSystemStorage, S3Storage, StorageProvider, StorageRegistry};

#[derive(Serialize)]
pub struct SettingsResponse {
    #[serde(flatten)]
    settings: InstanceSettings,
    storage_credentials: CredentialState,
    backup_credentials: CredentialState,
}

#[derive(Serialize)]
pub struct CredentialState {
    configured: bool,
    masked_access_key_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageUpdate {
    #[serde(flatten)]
    settings: StorageSettings,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    #[serde(default)]
    confirm_target_change: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupUpdate {
    #[serde(flatten)]
    settings: BackupSettings,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    #[serde(default)]
    confirm_target_change: bool,
}

async fn owner(auth: &AuthContext, repository: &Repository) -> Result<(), Response> {
    require_instance_action(auth, repository, Action::InstanceSettings)
        .await
        .map(|_| ())
        .map_err(|error| error.into_response())
}

pub async fn get_settings(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(settings): Extension<SettingsService>,
) -> Response {
    if let Err(response) = owner(&auth, &repository).await {
        return response;
    }
    response(&settings).await.into_response()
}

pub async fn update_general(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(settings): Extension<SettingsService>,
    Json(section): Json<GeneralSettings>,
) -> Response {
    if let Err(response) = owner(&auth, &repository).await {
        return response;
    }
    let mut next = (*settings.current()).clone();
    next.general = section;
    publish(&settings, next, settings.credentials().await).await
}

pub async fn update_security(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(settings): Extension<SettingsService>,
    Json(section): Json<SecuritySettings>,
) -> Response {
    if let Err(response) = owner(&auth, &repository).await {
        return response;
    }
    let mut next = (*settings.current()).clone();
    next.security = section;
    publish(&settings, next, settings.credentials().await).await
}

pub async fn update_storage(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(pool): Extension<DbPool>,
    Extension(settings): Extension<SettingsService>,
    Extension(registry): Extension<Arc<StorageRegistry>>,
    Json(update): Json<StorageUpdate>,
) -> Response {
    if let Err(response) = owner(&auth, &repository).await {
        return response;
    }
    let current = settings.current();
    if target_changed(&current.storage, &update.settings) && !update.confirm_target_change {
        return AppError::Conflict("Changing the S3 bucket or endpoint requires confirm_target_change=true".into())
            .into_response();
    }
    if update.settings.provider == "filesystem" && current.storage.provider == "s3" {
        match s3_site_count(&pool).await {
            Ok(count) if count > 0 => {
                return AppError::Conflict(format!("{count} site(s) still use S3 storage")).into_response();
            }
            Err(error) => return AppError::Internal(error.to_string()).into_response(),
            _ => {}
        }
    }
    let mut credentials = settings.credentials().await;
    let provider: Option<S3Storage> = if update.settings.provider == "filesystem" {
        credentials.storage = None;
        None
    } else {
        match merged_pair(
            credentials.storage.clone(),
            update.access_key_id,
            update.secret_access_key,
        ) {
            Ok(pair) => {
                let provider = match probe_s3(&update.settings, &pair).await {
                    Ok(provider) => provider,
                    Err(error) => return AppError::BadRequest(format!("S3 probe failed: {error}")).into_response(),
                };
                credentials.storage = Some(pair);
                Some(provider)
            }
            Err(error) => return AppError::BadRequest(error).into_response(),
        }
    };
    let mut next = (*current).clone();
    next.storage = update.settings;
    if settings.current().storage != current.storage {
        return AppError::Conflict("Storage settings changed while the S3 probe was running; review and retry".into())
            .into_response();
    }
    if let Err(error) = settings.publish(next, credentials).await {
        return AppError::BadRequest(error).into_response();
    }
    match provider {
        Some(provider) => registry.register("s3", Arc::new(provider)),
        None => registry.remove("s3"),
    }
    response(&settings).await.into_response()
}

pub async fn update_backups(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(config): Extension<Config>,
    Extension(settings): Extension<SettingsService>,
    Extension(backup): Extension<Arc<BackupService>>,
    Json(update): Json<BackupUpdate>,
) -> Response {
    if let Err(response) = owner(&auth, &repository).await {
        return response;
    }
    let current = settings.current();
    if backup_target_changed(&current.backups, &update.settings) && !update.confirm_target_change {
        return AppError::Conflict(
            "Changing the backup S3 bucket or endpoint requires confirm_target_change=true".into(),
        )
        .into_response();
    }
    let mut credentials = settings.credentials().await;
    let destination: Arc<dyn StorageProvider> = if update.settings.destination == "filesystem" {
        credentials.backups = None;
        match FileSystemStorage::new(config.backup_local_path.as_deref().unwrap_or("vcms_data/backups")) {
            Ok(provider) => Arc::new(provider),
            Err(error) => return AppError::Internal(error.to_string()).into_response(),
        }
    } else {
        match merged_pair(
            credentials.backups.clone(),
            update.access_key_id,
            update.secret_access_key,
        ) {
            Ok(pair) => {
                let storage = StorageSettings {
                    provider: "s3".into(),
                    bucket: update.settings.bucket.clone(),
                    region: update.settings.region.clone(),
                    endpoint: update.settings.endpoint.clone(),
                    public_url: None,
                };
                let provider = match probe_s3(&storage, &pair).await {
                    Ok(provider) => provider,
                    Err(error) => return AppError::BadRequest(format!("S3 probe failed: {error}")).into_response(),
                };
                credentials.backups = Some(pair);
                Arc::new(provider)
            }
            Err(error) => return AppError::BadRequest(error).into_response(),
        }
    };
    let mut next = (*current).clone();
    next.backups = update.settings;
    if settings.current().backups != current.backups {
        return AppError::Conflict("Backup settings changed while the S3 probe was running; review and retry".into())
            .into_response();
    }
    if let Err(error) = settings.publish(next, credentials).await {
        return AppError::BadRequest(error).into_response();
    }
    backup.set_destination(destination);
    response(&settings).await.into_response()
}

async fn publish(settings: &SettingsService, next: InstanceSettings, credentials: EncryptedCredentials) -> Response {
    match settings.publish(next, credentials).await {
        Ok(()) => response(settings).await.into_response(),
        Err(error) => AppError::BadRequest(error).into_response(),
    }
}

async fn response(settings: &SettingsService) -> Json<SettingsResponse> {
    let credentials = settings.credentials().await;
    Json(SettingsResponse {
        settings: (*settings.current()).clone(),
        storage_credentials: credential_state(credentials.storage.as_ref()),
        backup_credentials: credential_state(credentials.backups.as_ref()),
    })
}

fn credential_state(pair: Option<&CredentialPair>) -> CredentialState {
    CredentialState {
        configured: pair.is_some(),
        masked_access_key_id: pair.map(|pair| {
            let tail: String = pair
                .access_key_id
                .chars()
                .rev()
                .take(4)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            format!("••••{tail}")
        }),
    }
}

fn merged_pair(
    current: Option<CredentialPair>,
    id: Option<String>,
    secret: Option<String>,
) -> Result<CredentialPair, String> {
    match (id, secret) {
        (Some(access_key_id), Some(secret_access_key))
            if !access_key_id.is_empty() && !secret_access_key.is_empty() =>
        {
            Ok(CredentialPair {
                access_key_id,
                secret_access_key,
            })
        }
        (None, None) => current.ok_or_else(|| "S3 credentials are required".into()),
        _ => Err("provide both access_key_id and secret_access_key, or omit both".into()),
    }
}

async fn probe_s3(settings: &StorageSettings, credentials: &CredentialPair) -> Result<S3Storage, String> {
    let storage = S3Storage::new(
        &credentials.access_key_id,
        &credentials.secret_access_key,
        settings.bucket.as_deref().ok_or("bucket is required")?,
        settings.region.as_deref().unwrap_or("us-east-1"),
        settings.endpoint.as_deref(),
        settings.public_url.as_deref(),
    )
    .map_err(|error| error.to_string())?;
    let key = format!(".vcms-probe/{}", Uuid::new_v4());
    storage
        .put(&key, Bytes::from_static(b"vcms"), "application/octet-stream")
        .await
        .map_err(|error| error.to_string())?;
    let read = storage.get(&key).await.map_err(|error| error.to_string());
    let cleanup = storage.delete(&key).await.map_err(|error| error.to_string());
    let read = read?;
    cleanup?;
    if read.as_ref() != b"vcms" {
        return Err("probe read returned unexpected content".into());
    }
    Ok(storage)
}

fn target_changed(old: &StorageSettings, new: &StorageSettings) -> bool {
    (old.provider == "s3" || new.provider == "s3")
        && (old.provider != new.provider
            || old.bucket != new.bucket
            || old.region != new.region
            || old.endpoint != new.endpoint)
}

fn backup_target_changed(old: &BackupSettings, new: &BackupSettings) -> bool {
    (old.destination == "s3" || new.destination == "s3")
        && (old.destination != new.destination
            || old.bucket != new.bucket
            || old.region != new.region
            || old.endpoint != new.endpoint)
}

async fn s3_site_count(pool: &DbPool) -> Result<i64, sqlx::Error> {
    match pool {
        DbPool::Postgres(pool) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM sites WHERE storage_provider = 's3'")
                .fetch_one(pool)
                .await
        }
        DbPool::MySql(pool) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM sites WHERE storage_provider = 's3'")
                .fetch_one(pool)
                .await
        }
        DbPool::Sqlite(pool) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM sites WHERE storage_provider = 's3'")
                .fetch_one(pool)
                .await
        }
    }
}
