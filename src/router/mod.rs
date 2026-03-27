use axum::Extension;
use axum::{
    Router,
    routing::{delete, get, post, put},
};
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::handlers::api_key_handler::{create_api_key, delete_api_key, list_api_keys};
use crate::handlers::auth_handler::{login, me, register};
use crate::handlers::content_handler::{
    create_content, delete_content, get_content, list_content, publish_content, unpublish_content,
    update_content,
};
use crate::handlers::collection_handler::{
    create_collection, delete_collection, get_collection, list_collections, update_collection,
};
use crate::handlers::site_handler::{
    create_site, delete_site, get_site, invite_member, list_members, list_sites, remove_member,
    update_member_role, update_site,
};
use crate::handlers::ui_handler::ui_handler;

use crate::models::api_key::{ApiKey, ApiKeyResponse, CreateApiKey};
use crate::models::content::{Content, CreateContent, UpdateContent};
use crate::models::collection::{CreateCollection, Collection, UpdateCollection};
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
        contact(name = "CMS", url = "https://github.com/anomalyco/cms"),
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
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "sites", description = "Site management"),
        (name = "members", description = "Site member management"),
        (name = "api-keys", description = "API key management"),
        (name = "collections", description = "Collection management"),
        (name = "content", description = "Content management"),
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

pub fn create_router(pool: SqlitePool) -> Router {
    Router::new()
        // SPA
        .route(
            "/",
            get(|| async { ui_handler(axum::extract::Path("".into())).await }),
        )
        .route("/{*file}", get(ui_handler))
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
        .route("/api/v1/sites/{site_id}/collections", post(create_collection))
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
        // Scalar API docs
        .merge(Scalar::with_url("/api/v1/docs", ApiDoc::openapi()))
        .layer(CorsLayer::permissive())
        .layer(Extension(pool))
}
