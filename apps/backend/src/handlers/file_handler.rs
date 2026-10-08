use axum::{
    Json,
    body::Body,
    extract::{Extension, Path, Query},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use axum_extra::extract::multipart::Multipart;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::instrument;

#[derive(Deserialize)]
pub struct FileId {
    id: String,
}

use crate::config::Config;
use crate::error::AppError;
use crate::middleware::auth::{RequestContext, require_site_action};
use crate::models::authorization::Action;
use crate::models::file::{BatchFileIds, FileWithUrl};
use crate::repository::Repository;
use crate::repository::traits::ListFilesParams;
use crate::services::Services;
use crate::services::file::StreamingUploadRequest;
use crate::services::settings::SettingsService;
use crate::signed_upload::{SignedUploadError, SignedUploadToken};
use crate::storage::{StorageProvider, StorageRegistry};

#[derive(Deserialize, utoipa::IntoParams)]
pub struct FileListParams {
    pub page: Option<i64>,
    pub search: Option<String>,
    pub r#type: Option<String>,
    pub trashed: Option<String>,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct PublicFileListParams {
    pub cursor: Option<String>,
    pub search: Option<String>,
    pub r#type: Option<String>,
    pub include_total: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateFileUploadUrl {
    pub filename: String,
    pub content_type: String,
}

fn get_storage_for_site(
    site_storage_provider: &str,
    registry: &StorageRegistry,
) -> Result<Arc<dyn StorageProvider>, StatusCode> {
    registry
        .get(site_storage_provider)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[instrument(skip(repository, services, ctx, params, storage_registry))]
pub async fn list_files(
    ctx: RequestContext,
    Query(params): Query<FileListParams>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesRead).await {
        return (status, err).into_response();
    }

    let page = params.page.unwrap_or(1).max(1);
    let per_page: i64 = 30;
    let is_trashed = params.trashed.as_deref() == Some("true");

    let list_params = ListFilesParams {
        site_id: &ctx.site_id,
        trashed: is_trashed,
        search: params.search.as_deref(),
        file_type: params.r#type.as_deref(),
        page,
        per_page,
    };

    let storage_provider = match services.file.get_storage_provider(&ctx.site_id).await {
        Ok(provider) => provider,
        Err(error) => return error.into_response(),
    };

    match services.file.list_files(list_params).await {
        Ok(result) => {
            let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
                Ok(s) => s,
                Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
            };
            let with_urls: Vec<FileWithUrl> = result
                .items
                .iter()
                .map(|f| services.file.file_to_with_url(f, &*storage))
                .collect();
            (
                StatusCode::OK,
                Json(json!({
                    "items": with_urls,
                    "total": result.total,
                    "page": result.page,
                    "per_page": result.per_page,
                })),
            )
                .into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/files",
    params(PublicFileListParams),
    responses((status = 200, description = "List of file metadata")),
    security(("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx, params, storage_registry, config))]
pub async fn list_public_files(
    ctx: RequestContext,
    Query(params): Query<PublicFileListParams>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
    Extension(config): Extension<Config>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesRead).await {
        return (status, err).into_response();
    }
    let per_page = 50_i64;
    let fingerprint = crate::utils::cursor::fingerprint(&(&ctx.site_id, &params.search, &params.r#type, per_page));
    let page = match crate::utils::cursor::resolve_page(params.cursor.as_deref(), &fingerprint, &config.token_index_key)
    {
        Ok(page) => page,
        Err(message) => return crate::utils::cursor::invalid_cursor_response(message),
    };
    let storage_provider = match services.file.get_storage_provider(&ctx.site_id).await {
        Ok(provider) => provider,
        Err(error) => return error.into_response(),
    };
    let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
        Ok(storage) => storage,
        Err(status) => return (status, Json(json!({"error": "storage_not_configured"}))).into_response(),
    };
    match services
        .file
        .list_files(ListFilesParams {
            site_id: &ctx.site_id,
            trashed: false,
            search: params.search.as_deref(),
            file_type: params.r#type.as_deref(),
            page,
            per_page,
        })
        .await
    {
        Ok(result) => {
            let items: Vec<FileWithUrl> = result
                .items
                .iter()
                .map(|file| services.file.file_to_with_url(file, &*storage))
                .collect();
            let has_next_page = page.saturating_mul(per_page) < result.total;
            let next_cursor = has_next_page.then(|| {
                crate::utils::cursor::encode(
                    &crate::utils::cursor::PageCursor {
                        version: 1,
                        page: page + 1,
                        fingerprint,
                    },
                    &config.token_index_key,
                )
            });
            (
                StatusCode::OK,
                Json(json!({
                    "items": items,
                    "page_info": {"has_next_page": has_next_page, "next_cursor": next_cursor},
                    "total": params.include_total.unwrap_or(false).then_some(result.total),
                    "per_page": per_page,
                })),
            )
                .into_response()
        }
        Err(error) => error.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/files",
    responses(
        (status = 201, description = "File uploaded", body = FileWithUrl),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 413, description = "File too large"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, settings, ctx, multipart, storage_registry))]
pub async fn upload_file(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
    Extension(settings): Extension<SettingsService>,
    mut multipart: Multipart,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }
    let site_id = ctx.site_id.clone();

    let site = match services.site.get_site(&site_id).await {
        Ok(Some(site)) => site,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response(),
        Err(error) => return error.into_response(),
    };
    let storage_provider = match services.file.get_storage_provider(&site_id).await {
        Ok(provider) => provider,
        Err(error) => return error.into_response(),
    };

    let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
        Ok(s) => s,
        Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
    };
    let created_by = ctx.auth.actor.user_id().map(String::from);
    let max_bytes = settings.current().general.upload_limit_mb * 1024 * 1024;

    // Validate the complete multipart framing before signaling EOF to storage.
    // A bounded channel keeps the upload streaming while trailing parse errors
    // still abort its transaction rather than creating a partial file record.
    let mut sender = None;
    let mut upload_task = None;
    let mut parse_error = None;
    let mut field_count = 0;
    'fields: loop {
        let mut field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => {
                parse_error = Some("Malformed multipart body");
                break;
            }
        };
        field_count += 1;
        if field_count > 32 {
            parse_error = Some("Too many multipart fields");
            break;
        }
        if field.name() == Some("file") {
            if sender.is_some() {
                parse_error = Some("Only one file field is allowed");
                break;
            }
            let filename = sanitize_filename(field.file_name().unwrap_or("upload"));
            let content_type = field.content_type().unwrap_or("application/octet-stream").to_owned();
            let (tx, rx) =
                tokio::sync::mpsc::channel::<Result<Option<bytes::Bytes>, Box<dyn std::error::Error + Send + Sync>>>(2);
            let service = services.file.clone();
            let site_id = site_id.clone();
            let storage = storage.clone();
            let storage_provider = site.storage_provider.clone();
            let created_by = created_by.clone();
            upload_task = Some(tokio::spawn(async move {
                let chunks = futures_util::stream::unfold((rx, false), |(mut receiver, ended)| async move {
                    if ended {
                        return None;
                    }
                    match receiver.recv().await {
                        Some(Ok(Some(bytes))) => Some((Ok(bytes), (receiver, false))),
                        Some(Ok(None)) => None,
                        Some(Err(error)) => Some((Err(error), (receiver, true))),
                        None => Some((
                            Err(Box::new(std::io::Error::other("Upload interrupted"))
                                as Box<dyn std::error::Error + Send + Sync>),
                            (receiver, true),
                        )),
                    }
                });
                service
                    .upload_file_streaming(
                        StreamingUploadRequest {
                            site_id: &site_id,
                            file_id: None,
                            filename: &filename,
                            content_type: &content_type,
                            created_by: created_by.as_deref(),
                            storage,
                            storage_provider: &storage_provider,
                            max_bytes,
                        },
                        chunks,
                    )
                    .await
            }));
            sender = Some(tx);
            loop {
                match field.chunk().await {
                    Ok(Some(chunk)) => {
                        if sender.as_ref().unwrap().send(Ok(Some(chunk))).await.is_err() {
                            break 'fields;
                        }
                    }
                    Ok(None) => break,
                    Err(_) => {
                        parse_error = Some("Malformed multipart file field");
                        break 'fields;
                    }
                }
            }
        } else {
            let mut metadata_bytes = 0usize;
            loop {
                match field.chunk().await {
                    Ok(Some(chunk)) => {
                        metadata_bytes = metadata_bytes.saturating_add(chunk.len());
                        if metadata_bytes > 64 * 1024 {
                            parse_error = Some("Multipart metadata field is too large");
                            break 'fields;
                        }
                    }
                    Ok(None) => break,
                    Err(_) => {
                        parse_error = Some("Malformed multipart metadata field");
                        break 'fields;
                    }
                }
            }
        }
    }
    if let Some(sender) = sender {
        let final_chunk = match parse_error {
            Some(error) => Err(Box::new(std::io::Error::other(error)) as Box<dyn std::error::Error + Send + Sync>),
            None => Ok(None),
        };
        let _ = sender.send(final_chunk).await;
    }
    match upload_task {
        Some(task) => match task.await {
            Ok(Ok(file)) => (StatusCode::CREATED, Json(file)).into_response(),
            Ok(Err(error)) => error.into_response(),
            Err(error) => {
                tracing::error!(error = ?error, "Upload task failed");
                AppError::Internal("Upload failed".into()).into_response()
            }
        },
        None => AppError::BadRequest(parse_error.unwrap_or("No file provided").into()).into_response(),
    }
}

/// Mint a one-shot upload URL for clients that cannot or should not stream a
/// multipart request through the API process. The signed PUT is still checked
/// against site, storage profile, content type, and the live upload limit.
#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/files/upload-url",
    request_body = CreateFileUploadUrl,
    responses((status = 201, description = "Signed upload URL created")),
    security(("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx, config, headers, payload))]
pub async fn create_file_upload_url(
    ctx: RequestContext,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(config): Extension<Config>,
    Json(payload): Json<CreateFileUploadUrl>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }
    if let Err(error) = crate::services::file::FileService::validate_filename(&payload.filename) {
        return error.into_response();
    }
    if !crate::utils::content_types::is_allowed(&payload.content_type) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_content_type", "message": "Content type is not allowed"})),
        )
            .into_response();
    }
    let site = match services.site.get_site(&ctx.site_id).await {
        Ok(Some(site)) => site,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response(),
        Err(error) => return error.into_response(),
    };
    let Some(storage_profile_id) = site.storage_profile_id else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "storage_not_configured", "message": "Storage profile not configured"})),
        )
            .into_response();
    };
    let (token, upload_path) = SignedUploadToken::generate_with_storage_profile(
        &ctx.site_id,
        &payload.filename,
        &payload.content_type,
        &storage_profile_id,
        &config.signed_upload_key,
        config.upload_token_expiry_secs,
    );
    let host = config
        .trust_proxy_headers
        .then(|| headers.get("x-forwarded-host"))
        .flatten()
        .or_else(|| headers.get(header::HOST))
        .and_then(|value| value.to_str().ok())
        .unwrap_or(&config.bind_address);
    let scheme = config
        .trust_proxy_headers
        .then(|| headers.get("x-forwarded-proto"))
        .flatten()
        .and_then(|value| value.to_str().ok())
        .unwrap_or("http");
    let base = config
        .public_url
        .clone()
        .unwrap_or_else(|| format!("{scheme}://{host}"));
    let upload_url = format!("{}/api/v1/files/upload/{}", base.trim_end_matches('/'), upload_path);
    (
        StatusCode::CREATED,
        Json(json!({
            "upload_url": upload_url,
            "file_id": token.file_id,
            "expires_at": token.expires_at(),
            "method": "PUT",
            "content_type": payload.content_type,
        })),
    )
        .into_response()
}

#[utoipa::path(
    put,
    path = "/api/v1/files/upload/{token}",
    request_body(content = Vec<u8>, description = "Raw file bytes", content_type = "application/octet-stream"),
    responses(
        (status = 201, description = "File uploaded", body = FileWithUrl),
        (status = 400, description = "Bad request (content mismatch or unreadable body)"),
        (status = 401, description = "Invalid token"),
        (status = 409, description = "Upload URL already used"),
        (status = 410, description = "Upload URL expired"),
        (status = 413, description = "File too large"),
    ),
    tag = "files"
)]
#[instrument(skip(token, services, config, settings, storage_registry, headers, body))]
pub async fn upload_via_signed_url(
    Path(token): Path<String>,
    headers: HeaderMap,
    Extension(services): Extension<Services>,
    Extension(config): Extension<Config>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
    Extension(settings): Extension<SettingsService>,
    body: Body,
) -> Response {
    // The token is the auth: HMAC-signed by the server, time-limited, single-use.
    let token = match SignedUploadToken::verify(&token, &config.signed_upload_key) {
        Ok(t) => t,
        Err(SignedUploadError::Expired) => {
            return (StatusCode::GONE, Json(json!({"error": "Upload URL expired"}))).into_response();
        }
        Err(_) => {
            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid upload token"}))).into_response();
        }
    };

    // Fast reject when the client announces an oversized body upfront.
    let max_bytes = settings.current().general.upload_limit_mb * 1024 * 1024;
    if let Some(len) = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        && len > max_bytes as u64
    {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({
                "error": format!("File too large. Maximum size is {}MB", max_bytes / (1024 * 1024))
            })),
        )
            .into_response();
    }

    // The upload's content type was fixed when the URL was minted; a
    // contradicting header is a client bug worth failing loudly on.
    if let Some(ct) = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        let declared = ct.split(';').next().unwrap_or(ct).trim();
        if !declared.eq_ignore_ascii_case(&token.content_type) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Content-Type '{}' does not match the upload URL's '{}'", declared, token.content_type)
                })),
            )
                .into_response();
        }
    }

    let site = match services.site.get_site(&token.site_id).await {
        Ok(Some(site)) => site,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response(),
        Err(error) => return error.into_response(),
    };
    let storage_profile_id = match site.storage_profile_id.as_deref() {
        Some(profile_id) => profile_id,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Storage profile not configured"})),
            )
                .into_response();
        }
    };
    if storage_profile_id != token.storage_profile_id {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": "Storage selection changed; request a new upload URL"})),
        )
            .into_response();
    }
    let storage = match get_storage_for_site(storage_profile_id, &storage_registry) {
        Ok(s) => s,
        Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
    };

    // Consume the URL before reading bytes. A started upload attempt, including
    // a failed or interrupted one, requires a fresh URL for retry.
    if let Err(error) = services
        .file
        .claim_signed_upload(&token.file_id, token.expires_at)
        .await
    {
        return error.into_response();
    }

    let file_name = sanitize_filename(&token.filename);
    let stream = Box::pin(
        body.into_data_stream()
            .map(|r| r.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)),
    );

    match services
        .file
        .upload_file_streaming(
            StreamingUploadRequest {
                site_id: &token.site_id,
                file_id: Some(&token.file_id),
                filename: &file_name,
                content_type: &token.content_type,
                created_by: None,
                storage,
                storage_provider: &site.storage_provider,
                max_bytes,
            },
            stream,
        )
        .await
    {
        Ok(file) => (StatusCode::CREATED, Json(file)).into_response(),
        Err(e) => e.into_response(),
    }
}

/// Reduce an uploaded filename to a safe basename: strips any path components,
/// control/null bytes, and leading dots (so `../`, `..\`, and dotfiles can't
/// escape or hide). Falls back to `"upload"` if nothing usable remains.
fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base.chars().filter(|c| !c.is_control()).collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    if trimmed.is_empty() {
        "upload".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod sanitize_tests {
    use super::sanitize_filename;

    #[test]
    fn strips_path_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("..\\..\\windows\\system32\\x.dll"), "x.dll");
        assert_eq!(sanitize_filename("/abs/path/photo.jpg"), "photo.jpg");
    }

    #[test]
    fn strips_leading_dots_and_control() {
        assert_eq!(sanitize_filename("...env.png"), "env.png");
        assert_eq!(sanitize_filename("na\0me.txt"), "name.txt");
        assert_eq!(sanitize_filename(".."), "upload");
        assert_eq!(sanitize_filename(""), "upload");
    }

    #[test]
    fn keeps_normal_names() {
        assert_eq!(sanitize_filename("report 2026.pdf"), "report 2026.pdf");
        assert_eq!(sanitize_filename("photo.final.jpg"), "photo.final.jpg");
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/files/{id}",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "File item", body = FileWithUrl),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx, storage_registry))]
pub async fn get_file(
    ctx: RequestContext,
    Path(FileId { id }): Path<FileId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesRead).await {
        return (status, err).into_response();
    }

    match if matches!(ctx.auth.actor, crate::middleware::auth::Actor::User(_)) {
        services.file.get_file_including_deleted(&id, &ctx.site_id).await
    } else {
        services.file.get_file(&id, &ctx.site_id).await
    } {
        Ok(Some(file)) => {
            let storage_provider = match services.file.get_storage_provider(&ctx.site_id).await {
                Ok(provider) => provider,
                Err(error) => return error.into_response(),
            };
            let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
                Ok(s) => s,
                Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
            };
            let with_url = services.file.file_to_with_url(&file, &*storage);
            (StatusCode::OK, Json(with_url)).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}/files/{id}",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "File soft-deleted"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn delete_file_handler(
    ctx: RequestContext,
    Path(FileId { id }): Path<FileId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }

    match services.file.soft_delete(&id, &ctx.site_id).await {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response(),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "File deleted"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/files/{id}/references",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "References found", body = Vec<crate::models::file::FileReference>),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn get_file_references(
    ctx: RequestContext,
    Path(FileId { id }): Path<FileId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesRead).await {
        return (status, err).into_response();
    }

    match if matches!(ctx.auth.actor, crate::middleware::auth::Actor::User(_)) {
        services.file.get_file_including_deleted(&id, &ctx.site_id).await
    } else {
        services.file.get_file(&id, &ctx.site_id).await
    } {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response(),
        Err(error) => return error.into_response(),
    }

    match services.file.get_file_references(&id, &ctx.site_id).await {
        Ok(refs) => (StatusCode::OK, Json(refs)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/files/{id}/restore",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "File restored"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn restore_file(
    ctx: RequestContext,
    Path(FileId { id }): Path<FileId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }

    match services.file.restore(&id, &ctx.site_id).await {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "File not found or not deleted"})),
        )
            .into_response(),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "File restored"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, body))]
pub async fn batch_delete_files(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(body): Json<BatchFileIds>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }

    if body.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file IDs provided"}))).into_response();
    }

    match services.file.batch_soft_delete(&ctx.site_id, &body.ids).await {
        Ok(count) => (StatusCode::OK, Json(json!({"deleted": count}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, body))]
pub async fn batch_restore_files(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(body): Json<BatchFileIds>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }

    if body.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file IDs provided"}))).into_response();
    }

    match services.file.batch_restore(&ctx.site_id, &body.ids).await {
        Ok(count) => (StatusCode::OK, Json(json!({"restored": count}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, body, storage_registry))]
pub async fn batch_permanent_delete_files(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
    Json(body): Json<BatchFileIds>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }

    if body.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file IDs provided"}))).into_response();
    }

    let files = match repository.file.get_deleted_by_ids(&ctx.site_id, &body.ids).await {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to fetch files: {}", e)})),
            )
                .into_response();
        }
    };

    let storage_provider = match services.file.get_storage_provider(&ctx.site_id).await {
        Ok(provider) => provider,
        Err(error) => return error.into_response(),
    };
    let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
        Ok(s) => s,
        Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
    };

    for file in &files {
        if let Err(e) = storage.delete(&file.storage_key).await {
            tracing::warn!("Failed to delete file {} from storage: {}", file.id, e);
        }
        if let Some(ref tk) = file.thumbnail_key
            && let Err(e) = storage.delete(tk).await
        {
            tracing::warn!("Failed to delete thumbnail {} from storage: {}", file.id, e);
        }
    }

    match services.file.batch_permanent_delete(&ctx.site_id, &body.ids).await {
        Ok(count) => (StatusCode::OK, Json(json!({"deleted": count}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(services, storage_registry))]
pub async fn serve_file(
    Path(FileId { id }): Path<FileId>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    serve_file_by_key(&id, &services, &storage_registry, false).await
}

#[instrument(skip(services, storage_registry))]
pub async fn serve_file_thumbnail(
    Path(FileId { id }): Path<FileId>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    serve_file_by_key(&id, &services, &storage_registry, true).await
}

async fn serve_file_by_key(
    id: &str,
    services: &Services,
    storage_registry: &StorageRegistry,
    use_thumbnail: bool,
) -> Response {
    let file = match services.file.get_file_any(id).await {
        Ok(Some(f)) => f,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response(),
        Err(e) => return e.into_response(),
    };

    if file.deleted_at.is_some() {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response();
    }

    let storage_profile_id = match services.file.get_storage_provider(&file.site_id).await {
        Ok(profile_id) => profile_id,
        Err(error) => return error.into_response(),
    };
    let storage = match get_storage_for_site(&storage_profile_id, storage_registry) {
        Ok(s) => s,
        Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
    };

    match services.file.serve_file_streaming(id, use_thumbnail, storage).await {
        Ok((size, stream, content_type, original_name)) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(&content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            headers.insert(header::CONTENT_LENGTH, HeaderValue::from(size));
            headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
            if matches!(content_type.as_str(), "text/html" | "image/svg+xml") {
                // Uploaded active documents must not execute in the dashboard origin.
                headers.insert(
                    header::CONTENT_SECURITY_POLICY,
                    HeaderValue::from_static("sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:"),
                );
            }
            if use_thumbnail {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                );
            } else {
                headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=3600"));
                let safe_name: String = original_name
                    .chars()
                    .map(|character| {
                        if character.is_control() || matches!(character, '"' | '\\') {
                            '_'
                        } else {
                            character
                        }
                    })
                    .collect();
                headers.insert(
                    header::CONTENT_DISPOSITION,
                    HeaderValue::from_str(&format!(
                        "inline; filename=\"{}\"; filename*=UTF-8''{}",
                        safe_name
                            .chars()
                            .map(|character| if character.is_ascii() { character } else { '_' })
                            .collect::<String>(),
                        url::form_urlencoded::byte_serialize(safe_name.as_bytes())
                            .collect::<String>()
                            .replace('+', "%20"),
                    ))
                    .unwrap_or(HeaderValue::from_static("inline")),
                );
            }
            (StatusCode::OK, headers, Body::from_stream(stream)).into_response()
        }
        Err(e) => e.into_response(),
    }
}
