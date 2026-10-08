pub mod access_token;
pub mod auth;
pub mod authorization;
pub mod backup;
pub mod collection;
pub mod definition_validation;
pub mod deployment;
pub mod entry;
pub mod error;
pub mod file;
pub mod search;
pub mod settings;
pub mod singleton;
pub mod site;
pub mod storage_profile;
pub mod webhook;

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::Config;
use crate::database::pool::DbPool;
use crate::repository::Repository;
use crate::services::search::SearchService;
use crate::services::search::queue::SearchQueue;

#[derive(Clone)]
pub struct Services {
    pub auth: Arc<auth::AuthService>,
    pub site: Arc<site::SiteService>,
    pub access_token: Arc<access_token::AccessTokenService>,
    pub collection: Arc<collection::CollectionService>,
    pub entry: Arc<entry::EntryService>,
    pub file: Arc<file::FileService>,
    pub singleton: Arc<singleton::SingletonService>,
    pub webhook: Arc<webhook::WebhookService>,
    pub deployment: Arc<deployment::DeploymentService>,
    pub storage_profile: Arc<storage_profile::StorageProfileService>,
    /// Full-text search engine used for **reads** (ranked queries). `None` when
    /// search is disabled or the index couldn't be opened — callers then fall back
    /// to the SQL `LIKE` path.
    pub search: Option<Arc<SearchService>>,
    /// Durable queue used for **writes**: content changes enqueue here and the
    /// server's indexer applies them. Present whenever search is enabled.
    pub search_queue: Option<Arc<SearchQueue>>,
}

impl Services {
    /// Build services for the running server: opens the search index **read-write**
    /// (this process owns the single writer and runs the indexer).
    pub fn new(repository: Arc<Repository>, pool: &DbPool, config: &Config) -> Self {
        let search = build_search(config);
        let config = Arc::new(config.clone());

        let search_queue = if config.search_enabled {
            Some(Arc::new(SearchQueue::new(pool.clone())))
        } else {
            None
        };

        let webhook = Arc::new(webhook::WebhookService::new(
            repository.webhook.clone(),
            &config.webhook_encryption_key,
            config.webhook_allow_private_targets,
        ));
        Self {
            auth: Arc::new(auth::AuthService::new(
                repository.user.clone(),
                repository.session.clone(),
                config.session_auth_key.clone(),
                config.cookie_secure,
                config.session_lifetime_hours,
                config.public_registration_enabled,
                config.bcrypt_cost,
            )),
            site: Arc::new(site::SiteService::new(repository.site.clone(), repository.user.clone())),
            access_token: Arc::new(access_token::AccessTokenService::new(
                repository.access_token.clone(),
                config.token_index_key.clone(),
            )),
            collection: Arc::new(collection::CollectionService::new(repository.collection.clone())),
            entry: Arc::new(
                entry::EntryService::new(
                    repository.entry.clone(),
                    repository.file.clone(),
                    repository.collection.clone(),
                )
                .with_search(search.clone())
                .with_queue(search_queue.clone()),
            ),
            file: Arc::new(file::FileService::new(repository.file.clone(), config.clone())),
            singleton: Arc::new(
                singleton::SingletonService::new(
                    repository.collection.clone(),
                    repository.entry.clone(),
                    repository.file.clone(),
                )
                .with_queue(search_queue.clone()),
            ),
            webhook: webhook.clone(),
            deployment: Arc::new(deployment::DeploymentService::new(pool.clone(), webhook)),
            storage_profile: Arc::new(storage_profile::StorageProfileService::new(
                pool.clone(),
                &config.webhook_encryption_key,
            )),
            search,
            search_queue,
        }
    }
}

fn search_index_path(config: &Config) -> Option<PathBuf> {
    config.search_index_path.clone().map(PathBuf::from)
}

/// Open the search index read-write: the server owns the single writer and runs
/// the indexer. Returns `None` when search is disabled or the index can't be
/// opened, so the app degrades to the SQL `LIKE` fallback instead of failing.
fn build_search(config: &Config) -> Option<Arc<SearchService>> {
    if !config.search_enabled {
        return None;
    }
    let Some(path) = search_index_path(config) else {
        tracing::error!("Full-text search disabled: runtime search path is missing");
        return None;
    };
    match SearchService::open(&path) {
        Ok(search) => Some(Arc::new(search)),
        Err(error) => {
            tracing::error!(
                "Full-text search disabled: failed to open index at {}: {}",
                path.display(),
                error
            );
            None
        }
    }
}
