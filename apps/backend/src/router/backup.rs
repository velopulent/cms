//! Backup & restore routes.
//!
//! `instance_routes` mount under `/api/dashboard/instance` (session auth only,
//! owner-gated in the handlers). `site_routes` mount under
//! `/api/dashboard/sites/{site_id}` (behind the site resolver, operator-gated).

use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handlers::backup_handler as h;

pub fn instance_routes() -> Router {
    Router::new()
        .route(
            "/instance/backups",
            get(h::list_instance_backups).post(h::create_instance_backup),
        )
        .route("/instance/backups/{backup_id}", delete(h::delete_instance_backup))
        .route(
            "/instance/backups/{backup_id}/download",
            get(h::download_instance_backup),
        )
        .route("/instance/restore", post(h::restore_instance))
        .route("/instance/restore/upload", post(h::restore_instance_upload))
        .route("/instance/restore/inspect", post(h::inspect_instance_backup))
        .route(
            "/instance/restore/inspect/upload",
            post(h::inspect_instance_backup_upload),
        )
        .route(
            "/instance/backup-schedules",
            get(h::list_instance_schedules).post(h::create_instance_schedule),
        )
        .route(
            "/instance/backup-schedules/{schedule_id}",
            put(h::update_instance_schedule).delete(h::delete_instance_schedule),
        )
        .route(
            "/instance/backup-schedules/{schedule_id}/run",
            post(h::run_instance_schedule),
        )
        .route("/instance/search/reindex", post(h::reindex_instance))
}

pub fn site_routes() -> Router {
    Router::new()
        .route("/backups", get(h::list_site_backups).post(h::create_site_backup))
        .route("/backups/{backup_id}", delete(h::delete_site_backup))
        .route("/backups/{backup_id}/download", get(h::download_site_backup))
        .route("/restore", post(h::restore_site))
        .route("/restore/upload", post(h::restore_site_upload))
        .route(
            "/backup-schedules",
            get(h::list_site_schedules).post(h::create_site_schedule),
        )
        .route(
            "/backup-schedules/{schedule_id}",
            put(h::update_site_schedule).delete(h::delete_site_schedule),
        )
        .route("/backup-schedules/{schedule_id}/run", post(h::run_site_schedule))
        .route("/search/reindex", post(h::reindex_site))
}
