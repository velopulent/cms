use axum::Extension;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderMap;
use axum::{
    Router,
    response::{Html, IntoResponse},
    routing::{delete, get, post, put},
};
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::config::Config;
use crate::graphql::context::GqlContext;
use crate::graphql::schema::{CmsSchema, build_schema};
use crate::handlers::api_key_handler::{create_api_key, delete_api_key, list_api_keys};
use crate::handlers::auth_handler::{login, me, register};
use crate::handlers::collection_handler::{
    create_collection, delete_collection, get_collection, list_collections, update_collection,
};
use crate::handlers::content_handler::{
    create_content, delete_content, get_content, list_content, publish_content, unpublish_content,
    update_content,
};
use crate::handlers::file_handler::{
    StorageManager, batch_delete_files, batch_permanent_delete_files, batch_restore_files,
    delete_file_handler, get_file, get_file_references, list_files, restore_file, serve_file,
    serve_file_thumbnail, upload_file,
};
use crate::handlers::site_handler::{
    create_site, delete_site, get_site, invite_member, list_members, list_sites, remove_member,
    update_member_role, update_site,
};
use crate::handlers::ui_handler::ui_handler;

use crate::models::api_key::{ApiKey, ApiKeyResponse, CreateApiKey};
use crate::models::collection::{Collection, CreateCollection, UpdateCollection};
use crate::models::content::{Content, CreateContent, UpdateContent};
use crate::models::file::{BatchFileIds, File, FileReference, FileWithUrl};
use crate::models::site::{
    CreateSite, InviteMember, Site, SiteMember, SiteWithRole, UpdateMemberRole, UpdateSite,
};
use crate::models::user::{AuthResponse, CreateUser, LoginRequest, UserPublic};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "CMS API",
        version = "1.0.0",
        description = "Headless CMS API for managing sites, collections, and content. \
            Dashboard endpoints require JWT authentication. \
            Public API endpoints accept either JWT or API key authentication.",
        contact(name = "CMS", url = "https://cms.velopulent.com"),
        license(name = "MIT")
    ),
    paths(
        // Auth
        crate::handlers::auth_handler::register,
        crate::handlers::auth_handler::login,
        crate::handlers::auth_handler::me,
        // Sites
        crate::handlers::site_handler::list_sites,
        crate::handlers::site_handler::create_site,
        crate::handlers::site_handler::get_site,
        crate::handlers::site_handler::update_site,
        crate::handlers::site_handler::delete_site,
        // Members
        crate::handlers::site_handler::list_members,
        crate::handlers::site_handler::invite_member,
        crate::handlers::site_handler::update_member_role,
        crate::handlers::site_handler::remove_member,
        // API Keys
        crate::handlers::api_key_handler::list_api_keys,
        crate::handlers::api_key_handler::create_api_key,
        crate::handlers::api_key_handler::delete_api_key,
        // Collections
        crate::handlers::collection_handler::list_collections,
        crate::handlers::collection_handler::get_collection,
        crate::handlers::collection_handler::create_collection,
        crate::handlers::collection_handler::update_collection,
        crate::handlers::collection_handler::delete_collection,
        // Content
        crate::handlers::content_handler::list_content,
        crate::handlers::content_handler::get_content,
        crate::handlers::content_handler::create_content,
        crate::handlers::content_handler::update_content,
        crate::handlers::content_handler::delete_content,
        crate::handlers::content_handler::publish_content,
        crate::handlers::content_handler::unpublish_content,
        // Files
        crate::handlers::file_handler::list_files,
        crate::handlers::file_handler::upload_file,
        crate::handlers::file_handler::get_file,
        crate::handlers::file_handler::delete_file_handler,
        crate::handlers::file_handler::get_file_references,
        crate::handlers::file_handler::restore_file,
        crate::handlers::file_handler::batch_delete_files,
        crate::handlers::file_handler::batch_restore_files,
        crate::handlers::file_handler::batch_permanent_delete_files,
    ),
    components(schemas(
        // User
        CreateUser, LoginRequest, AuthResponse, UserPublic,
        // Site
        Site, SiteWithRole, CreateSite, UpdateSite, SiteMember, InviteMember, UpdateMemberRole,
        // API Key
        ApiKey, CreateApiKey, ApiKeyResponse,
        // Collection
        Collection, CreateCollection, UpdateCollection,
        // Content
        Content, CreateContent, UpdateContent,
        // File
        File, FileWithUrl, FileReference, BatchFileIds,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "sites", description = "Site management"),
        (name = "members", description = "Site member management"),
        (name = "api-keys", description = "API key management"),
        (name = "collections", description = "Collection management"),
        (name = "content", description = "Content management"),
        (name = "files", description = "File management"),
    )
)]
struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer",
            utoipa::openapi::security::SecurityScheme::Http(
                utoipa::openapi::security::HttpBuilder::new()
                    .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
        components.add_security_scheme(
            "api_key",
            utoipa::openapi::security::SecurityScheme::Http(
                utoipa::openapi::security::HttpBuilder::new()
                    .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                    .bearer_format("API Key (cms_...)")
                    .build(),
            ),
        );
    }
}

// --- GraphQL handlers ---

async fn graphql_handler(
    axum::extract::Extension(schema): axum::extract::Extension<CmsSchema>,
    axum::extract::Extension(pool): axum::extract::Extension<SqlitePool>,
    axum::extract::Extension(storage): axum::extract::Extension<StorageManager>,
    headers: HeaderMap,
    req: async_graphql_axum::GraphQLRequest,
) -> async_graphql_axum::GraphQLResponse {
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let gql_ctx = GqlContext::from_request(pool, storage, auth_header).await;

    let response = schema.execute(req.into_inner().data(gql_ctx)).await;
    async_graphql_axum::GraphQLResponse::from(response)
}

async fn graphiql_handler() -> impl IntoResponse {
    Html(
        async_graphql::http::GraphiQLSource::build()
            .endpoint("/api/graphql")
            .finish(),
    )
}

pub fn create_router(pool: SqlitePool, config: Config, storage: StorageManager) -> Router {
    let max_upload_bytes = config.max_upload_size_bytes;

    Router::new()
        // Auth (unversioned, dashboard-only)
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
        // Sites
        .route("/api/v1/sites", get(list_sites))
        .route("/api/v1/sites", post(create_site))
        .route("/api/v1/sites/{site_id}", get(get_site))
        .route("/api/v1/sites/{site_id}", put(update_site))
        .route("/api/v1/sites/{site_id}", delete(delete_site))
        // Site Members
        .route("/api/v1/sites/{site_id}/members", get(list_members))
        .route("/api/v1/sites/{site_id}/members", post(invite_member))
        .route(
            "/api/v1/sites/{site_id}/members/{user_id}",
            put(update_member_role),
        )
        .route(
            "/api/v1/sites/{site_id}/members/{user_id}",
            delete(remove_member),
        )
        // API Keys (site-scoped)
        .route("/api/v1/sites/{site_id}/api-keys", get(list_api_keys))
        .route("/api/v1/sites/{site_id}/api-keys", post(create_api_key))
        .route(
            "/api/v1/sites/{site_id}/api-keys/{key_id}",
            delete(delete_api_key),
        )
        // Collections (site-scoped)
        .route("/api/v1/sites/{site_id}/collections", get(list_collections))
        .route(
            "/api/v1/sites/{site_id}/collections",
            post(create_collection),
        )
        .route(
            "/api/v1/sites/{site_id}/collections/{collection_slug}",
            get(get_collection),
        )
        .route(
            "/api/v1/sites/{site_id}/collections/{collection_slug}",
            put(update_collection),
        )
        .route(
            "/api/v1/sites/{site_id}/collections/{collection_slug}",
            delete(delete_collection),
        )
        // Content (site-scoped)
        .route("/api/v1/sites/{site_id}/content", get(list_content))
        .route("/api/v1/sites/{site_id}/content", post(create_content))
        .route("/api/v1/sites/{site_id}/content/{id}", get(get_content))
        .route("/api/v1/sites/{site_id}/content/{id}", put(update_content))
        .route(
            "/api/v1/sites/{site_id}/content/{id}",
            delete(delete_content),
        )
        .route(
            "/api/v1/sites/{site_id}/content/{id}/publish",
            post(publish_content),
        )
        .route(
            "/api/v1/sites/{site_id}/content/{id}/unpublish",
            post(unpublish_content),
        )
        // Files (site-scoped)
        .route("/api/v1/sites/{site_id}/files", get(list_files))
        // Upload route uses a nested router to disable DefaultBodyLimit
        // before applying RequestBodyLimitLayer (avoids type inference issue
        // with MethodRouter::layer)
        .merge(
            Router::new()
                .route("/api/v1/sites/{site_id}/files", post(upload_file))
                .layer(DefaultBodyLimit::disable())
                .layer(RequestBodyLimitLayer::new(max_upload_bytes)),
        )
        .route(
            "/api/v1/sites/{site_id}/files/batch-delete",
            post(batch_delete_files),
        )
        .route(
            "/api/v1/sites/{site_id}/files/batch-restore",
            post(batch_restore_files),
        )
        .route(
            "/api/v1/sites/{site_id}/files/batch-permanent-delete",
            post(batch_permanent_delete_files),
        )
        .route("/api/v1/sites/{site_id}/files/{id}", get(get_file))
        .route(
            "/api/v1/sites/{site_id}/files/{id}",
            delete(delete_file_handler),
        )
        .route(
            "/api/v1/sites/{site_id}/files/{id}/references",
            get(get_file_references),
        )
        .route(
            "/api/v1/sites/{site_id}/files/{id}/restore",
            post(restore_file),
        )
        // File serving (public, no auth)
        .route("/api/files/{id}", get(serve_file))
        .route("/api/files/{id}/thumbnail", get(serve_file_thumbnail))
        // GraphQL API
        .route("/api/graphql", get(graphiql_handler).post(graphql_handler))
        .layer(Extension(build_schema()))
        // Scalar API docs
        .merge(Scalar::with_url("/api/v1/docs", ApiDoc::openapi()))
        // SPA fallback — must be last
        .route(
            "/",
            get(|| async { ui_handler(axum::extract::Path("".into())).await }),
        )
        .route("/{*file}", get(ui_handler))
        .layer(CorsLayer::permissive())
        .layer(Extension(pool))
        .layer(Extension(config))
        .layer(Extension(storage))
}
