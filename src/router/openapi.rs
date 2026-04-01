use utoipa::OpenApi;

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
pub struct ApiDoc;

pub struct SecurityAddon;

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
