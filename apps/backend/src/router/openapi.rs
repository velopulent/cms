use utoipa::OpenApi;

use crate::models::access_token::{AccessToken, AccessTokenResponse, CreateInstanceToken, CreateSiteToken};
use crate::models::collection::{Collection, CreateCollection, UpdateCollection};
use crate::models::entry::{CreateEntry, Entry, EntryRevisionResponse, RevisionsListResponse, UpdateEntry};
use crate::models::file::{BatchFileIds, File, FileReference, FileWithUrl};
use crate::models::site::{CreateSite, InviteMember, Site, SiteMember, SiteWithRole, UpdateMemberRole, UpdateSite};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "CMS API",
        version = "1.0.0",
        description = "Headless CMS unified API. Site-scoped endpoints use cms_sk_* tokens. \
            Instance-scoped endpoints use cms_ik_* tokens. Dashboard JWT callers can pass x-cms-site-id for site context.",
        contact(name = "CMS", url = "https://cms.velopulent.com"),
        license(name = "MIT")
    ),
    paths(
        // Instance-scoped: Sites
        crate::handlers::site_handler::list_sites,
        crate::handlers::site_handler::create_site,
        crate::handlers::site_handler::get_site,
        crate::handlers::site_handler::update_site,
        crate::handlers::site_handler::delete_site,
        // Instance-scoped: Site members
        crate::handlers::site_handler::list_members,
        crate::handlers::site_handler::invite_member,
        crate::handlers::site_handler::update_member_role,
        crate::handlers::site_handler::remove_member,
        // Instance-scoped: Tokens
        crate::handlers::access_token_handler::list_instance_tokens,
        crate::handlers::access_token_handler::create_instance_token,
        crate::handlers::access_token_handler::delete_instance_token,
        // Site-scoped: Collections
        crate::handlers::collection_handler::list_collections,
        crate::handlers::collection_handler::get_collection,
        crate::handlers::collection_handler::create_collection,
        crate::handlers::collection_handler::update_collection,
        crate::handlers::collection_handler::delete_collection,
        // Site-scoped: Entries
        crate::handlers::entry_handler::list_entries,
        crate::handlers::entry_handler::get_entry,
        crate::handlers::entry_handler::create_entry,
        crate::handlers::entry_handler::update_entry,
        crate::handlers::entry_handler::delete_entry,
        crate::handlers::entry_handler::publish_entry,
        crate::handlers::entry_handler::unpublish_entry,
        crate::handlers::entry_handler::list_entry_revisions,
        crate::handlers::entry_handler::get_entry_revision,
        crate::handlers::entry_handler::restore_entry_revision,
        // Site-scoped: Singletons
        crate::handlers::singleton_handler::list_singletons,
        crate::handlers::singleton_handler::get_singleton,
        crate::handlers::singleton_handler::update_singleton,
        // Site-scoped: Files
        crate::handlers::file_handler::list_files,
        crate::handlers::file_handler::upload_file,
        crate::handlers::file_handler::get_file,
        crate::handlers::file_handler::delete_file_handler,
        crate::handlers::file_handler::get_file_references,
        crate::handlers::file_handler::restore_file,
        crate::handlers::file_handler::batch_delete_files,
        crate::handlers::file_handler::batch_restore_files,
        crate::handlers::file_handler::batch_permanent_delete_files,
        // Site-scoped: Tokens
        crate::handlers::access_token_handler::list_site_tokens,
        crate::handlers::access_token_handler::create_site_token,
        crate::handlers::access_token_handler::delete_site_token,
    ),
    components(schemas(
        // Site
        Site, SiteWithRole, CreateSite, UpdateSite,
        // Site members
        SiteMember, InviteMember, UpdateMemberRole,
        // Tokens
        AccessToken, CreateInstanceToken, CreateSiteToken, AccessTokenResponse,
        // Collection
        Collection, CreateCollection, UpdateCollection,
        // Entry
        Entry, CreateEntry, UpdateEntry, EntryRevisionResponse, RevisionsListResponse,
        // File
        File, FileWithUrl, FileReference, BatchFileIds,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "sites", description = "Instance-wide site management"),
        (name = "site-members", description = "Site membership management"),
        (name = "instance-tokens", description = "Instance-scoped access token management"),
        (name = "collections", description = "Collection management (site-scoped)"),
        (name = "entries", description = "Entry management (site-scoped)"),
        (name = "singletons", description = "Singleton management (site-scoped)"),
        (name = "files", description = "File management (site-scoped)"),
        (name = "site-tokens", description = "Site-scoped access token management"),
    )
)]
pub struct CmsApiDoc;

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
            "access_token",
            utoipa::openapi::security::SecurityScheme::Http(
                utoipa::openapi::security::HttpBuilder::new()
                    .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                    .bearer_format("Access Token (cms_site_... or cms_inst_...)")
                    .build(),
            ),
        );
    }
}
