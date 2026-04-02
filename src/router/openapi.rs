use utoipa::OpenApi;

use crate::models::collection::{Collection, CreateCollection, UpdateCollection};
use crate::models::content::{Content, CreateContent, UpdateContent};
use crate::models::file::{BatchFileIds, File, FileReference, FileWithUrl};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "CMS API",
        version = "1.0.0",
        description = "Headless CMS public API for managing content within a site. \
            All endpoints accept either JWT or API key authentication.",
        contact(name = "CMS", url = "https://cms.velopulent.com"),
        license(name = "MIT")
    ),
    paths(
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
        // Singletons
        crate::handlers::singleton_handler::list_singletons,
        crate::handlers::singleton_handler::get_singleton,
        crate::handlers::singleton_handler::update_singleton,
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
        // Collection
        Collection, CreateCollection, UpdateCollection,
        // Content
        Content, CreateContent, UpdateContent,
        // File
        File, FileWithUrl, FileReference, BatchFileIds,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "collections", description = "Collection management"),
        (name = "content", description = "Content management"),
        (name = "singletons", description = "Singleton management"),
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
