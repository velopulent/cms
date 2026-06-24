use utoipa::OpenApi;

use crate::models::collection::{Collection, CreateCollection, UpdateCollection};
use crate::models::entry::{CreateEntry, Entry, EntryRevisionResponse, RevisionsListResponse, UpdateEntry};
use crate::models::file::{BatchFileIds, File, FileReference, FileWithUrl};
use crate::models::site::Site;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Velopulent CMS REST API",
        version = "0.1.0",
        description = "Headless CMS unified API. Consumer access uses site-bound vcms_site_* tokens with read or write permission. Dashboard access uses revocable opaque sessions.",
        contact(name = "Velopulent CMS", url = "https://cms.velopulent.com"),
        license(name = "AGPL-3.0", url = "https://github.com/velopulent/cms/blob/main/LICENSE"),
    ),
    paths(
        // Public API: Site info
        crate::handlers::site_handler::get_current_site,
        // Public API: Collections
        crate::handlers::collection_handler::list_collections,
        crate::handlers::collection_handler::get_collection,
        crate::handlers::collection_handler::create_collection,
        crate::handlers::collection_handler::update_collection,
        crate::handlers::collection_handler::delete_collection,
        // Public API: Entries
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
        // Public API: Singletons
        crate::handlers::singleton_handler::list_singletons,
        crate::handlers::singleton_handler::get_singleton,
        crate::handlers::singleton_handler::update_singleton,
        // Public API: Files
        crate::handlers::file_handler::list_files,
        crate::handlers::file_handler::upload_file,
        crate::handlers::file_handler::get_file,
        crate::handlers::file_handler::delete_file_handler,
        crate::handlers::file_handler::get_file_references,
        crate::handlers::file_handler::restore_file,
        crate::handlers::file_handler::batch_delete_files,
        crate::handlers::file_handler::batch_restore_files,
        crate::handlers::file_handler::batch_permanent_delete_files,
        // Public API: Webhooks
        crate::handlers::webhook_handler::list_webhooks,
        crate::handlers::webhook_handler::create_webhook,
        crate::handlers::webhook_handler::get_webhook,
        crate::handlers::webhook_handler::update_webhook,
        crate::handlers::webhook_handler::delete_webhook,
        crate::handlers::webhook_handler::trigger_webhook,
        crate::handlers::webhook_handler::list_deliveries,
    ),
    components(schemas(
        Site,
        Collection, CreateCollection, UpdateCollection,
        Entry, CreateEntry, UpdateEntry, EntryRevisionResponse, RevisionsListResponse,
        File, FileWithUrl, FileReference, BatchFileIds,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "site", description = "Current site info"),
        (name = "collections", description = "Collection management (site-scoped)"),
        (name = "entries", description = "Entry management (site-scoped)"),
        (name = "singletons", description = "Singleton management (site-scoped)"),
        (name = "files", description = "File management (site-scoped)"),
        (name = "webhooks", description = "Webhook management (site-scoped)"),
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
                    .bearer_format("Opaque dashboard session")
                    .build(),
            ),
        );
        components.add_security_scheme(
            "access_token",
            utoipa::openapi::security::SecurityScheme::Http(
                utoipa::openapi::security::HttpBuilder::new()
                    .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                    .bearer_format("Access Token (vcms_site_...)")
                    .build(),
            ),
        );
    }
}
