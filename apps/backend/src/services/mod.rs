pub mod access_token;
pub mod auth;
pub mod authorization;
pub mod backup;
pub mod collection;
pub mod definition_validation;
pub mod entry;
pub mod error;
pub mod file;
pub mod search;
pub mod singleton;
pub mod site;
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
    /// Full-text search engine used for **reads** (ranked queries). `None` when
    /// search is disabled or the index couldn't be opened — callers then fall back
    /// to the SQL `LIKE` path. Read-write in the server process, read-only in
    /// `vcms mcp stdio`.
    pub search: Option<Arc<SearchService>>,
    /// Durable queue used for **writes**: content changes enqueue here and the
    /// server's indexer applies them. Present whenever search is enabled, even in
    /// read-only processes (so their writes still reach the server's index).
    pub search_queue: Option<Arc<SearchQueue>>,
}

impl Services {
    /// Build services for the running server: opens the search index **read-write**
    /// (this process owns the single writer and runs the indexer).
    pub fn new(repository: Arc<Repository>, pool: &DbPool, config: &Config) -> Self {
        let search = build_search(config, IndexAccess::ReadWrite);
        Self::assemble(repository, pool, config, search)
    }

    /// Build services for an auxiliary process (e.g. `vcms mcp stdio`): opens the
    /// index **read-only** so it can search without contending for the writer lock.
    /// Its writes still enqueue for the server to index.
    pub fn new_read_only(repository: Arc<Repository>, pool: &DbPool, config: &Config) -> Self {
        let search = build_search(config, IndexAccess::ReadOnly);
        Self::assemble(repository, pool, config, search)
    }

    fn assemble(
        repository: Arc<Repository>,
        pool: &DbPool,
        config: &Config,
        search: Option<Arc<SearchService>>,
    ) -> Self {
        let config = Arc::new(config.clone());

        let search_queue = if config.search_enabled {
            Some(Arc::new(SearchQueue::new(pool.clone())))
        } else {
            None
        };

        Self {
            auth: Arc::new(auth::AuthService::new(
                repository.user.clone(),
                repository.session.clone(),
                config.hmac_secret.clone(),
                config.cookie_secure,
                config.session_lifetime_hours,
                config.public_registration_enabled,
                config.bcrypt_cost,
            )),
            site: Arc::new(site::SiteService::new(repository.site.clone(), repository.user.clone())),
            access_token: Arc::new(access_token::AccessTokenService::new(
                repository.access_token.clone(),
                config.hmac_secret.clone(),
                config.bcrypt_cost,
            )),
            collection: Arc::new(collection::CollectionService::new(
                repository.collection.clone(),
                repository.entry.clone(),
            )),
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
            webhook: Arc::new(webhook::WebhookService::new(
                repository.webhook.clone(),
                &config.hmac_secret,
                config.webhook_allow_private_targets,
            )),
            search,
            search_queue,
        }
    }
}

enum IndexAccess {
    ReadWrite,
    ReadOnly,
}

fn search_index_path(config: &Config) -> PathBuf {
    config
        .search_index_path
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(crate::paths::search_dir)
}

/// Open the search index for queries. Returns `None` when search is disabled or the
/// index can't be opened, so the app degrades to the SQL `LIKE` fallback rather than
/// failing to start. A missing index in read-only mode is expected (the server
/// hasn't built it yet) and logged softly.
fn build_search(config: &Config, access: IndexAccess) -> Option<Arc<SearchService>> {
    if !config.search_enabled {
        return None;
    }
    let path = search_index_path(config);
    let opened = match access {
        IndexAccess::ReadWrite => SearchService::open(&path),
        IndexAccess::ReadOnly => SearchService::open_read_only(&path),
    };
    match opened {
        Ok(svc) => Some(Arc::new(svc)),
        Err(e) => {
            match access {
                IndexAccess::ReadWrite => {
                    tracing::error!(
                        "Full-text search disabled: failed to open index at {}: {}",
                        path.display(),
                        e
                    )
                }
                IndexAccess::ReadOnly => tracing::warn!(
                    "Read-only search index unavailable at {} ({}); using SQL LIKE fallback",
                    path.display(),
                    e
                ),
            }
            None
        }
    }
}
