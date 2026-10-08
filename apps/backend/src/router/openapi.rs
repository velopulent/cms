use utoipa::OpenApi;

use crate::handlers::entry_handler::CreateCollectionEntry;
use crate::handlers::file_handler::CreateFileUploadUrl;
use crate::handlers::site_handler::PublicSite;
use crate::models::collection::{PublicCollection, SingletonResponse, UpdateSingletonData};
use crate::models::entry::PublicEntry;
use crate::models::entry::{EntryRevisionResponse, RevisionsListResponse, UpdateEntry};
use crate::models::file::{File, FileReference, FileWithUrl};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Velopulent CMS REST API",
        version = "0.1.0",
        description = "Headless CMS data-plane API. Consumer access uses scoped vcms_site_* keys or vcms_pat_* personal tokens with an explicit site path. Dashboard management remains under /api/dashboard.",
        contact(name = "Velopulent CMS", url = "https://cms.velopulent.com/docs"),
        license(name = "AGPL-3.0", url = "https://github.com/velopulent/cms/blob/main/LICENSE"),
    ),
    paths(
        // Public data plane. Dashboard and management handlers are intentionally
        // absent from this document.
        crate::handlers::site_handler::list_public_sites,
        crate::handlers::site_handler::get_current_site,
        crate::handlers::collection_handler::list_public_collections,
        crate::handlers::collection_handler::get_public_collection,
        crate::handlers::entry_handler::list_collection_entries,
        crate::handlers::entry_handler::create_collection_entry,
        crate::handlers::entry_handler::get_public_entry,
        crate::handlers::entry_handler::update_public_entry,
        crate::handlers::entry_handler::delete_entry,
        crate::handlers::entry_handler::publish_public_entry,
        crate::handlers::entry_handler::unpublish_public_entry,
        crate::handlers::entry_handler::list_entry_revisions,
        crate::handlers::entry_handler::get_entry_revision,
        crate::handlers::entry_handler::restore_public_entry_revision,
        crate::handlers::singleton_handler::list_singletons,
        crate::handlers::singleton_handler::get_singleton,
        crate::handlers::singleton_handler::update_singleton,
        crate::handlers::file_handler::list_public_files,
        crate::handlers::file_handler::upload_file,
        crate::handlers::file_handler::create_file_upload_url,
        crate::handlers::file_handler::upload_via_signed_url,
        crate::handlers::file_handler::get_file,
        crate::handlers::file_handler::delete_file_handler,
        crate::handlers::file_handler::get_file_references,
        crate::handlers::file_handler::restore_file,
    ),
    components(schemas(
        PublicSite,
        PublicCollection, UpdateSingletonData, SingletonResponse,
        PublicEntry, UpdateEntry, EntryRevisionResponse, RevisionsListResponse,
        File, FileWithUrl, FileReference, CreateCollectionEntry, CreateFileUploadUrl,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "sites", description = "Discover accessible sites"),
        (name = "collections", description = "Read collection schemas"),
        (name = "entries", description = "Headless content entries"),
        (name = "singletons", description = "Singleton content"),
        (name = "files", description = "File metadata and uploads"),
    )
)]
pub struct CmsApiDoc;

pub struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        for (path, item) in &mut openapi.paths.paths {
            if !path.starts_with("/api/v1/files/upload/") {
                for operation in [
                    &mut item.get,
                    &mut item.post,
                    &mut item.put,
                    &mut item.patch,
                    &mut item.delete,
                ]
                .into_iter()
                .flatten()
                {
                    operation.security = Some(vec![utoipa::openapi::security::SecurityRequirement::new(
                        "access_token",
                        Vec::<String>::new(),
                    )]);
                }
            }
            if path.contains("{site_id}") {
                let parameters = item.parameters.get_or_insert_with(Vec::new);
                parameters.push(
                    utoipa::openapi::path::ParameterBuilder::new()
                        .name("site_id")
                        .parameter_in(utoipa::openapi::path::ParameterIn::Path)
                        .schema(Some(
                            utoipa::openapi::schema::ObjectBuilder::new()
                                .schema_type(utoipa::openapi::schema::Type::String),
                        ))
                        .description(Some("Explicit target site; must be authorized by the token"))
                        .build(),
                );
            }
        }
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "access_token",
            utoipa::openapi::security::SecurityScheme::Http(
                utoipa::openapi::security::HttpBuilder::new()
                    .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                    .bearer_format("Scoped access token (vcms_site_* or vcms_pat_*)")
                    .build(),
            ),
        );
    }
}
