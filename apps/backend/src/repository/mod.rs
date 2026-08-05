pub mod error;
pub mod postgres;
pub mod sqlite;
pub mod traits;

use std::sync::Arc;

use crate::database::pool::DbPool;
use crate::repository::traits::{
    AccessTokenRepository, CollectionRepository, EntryRepository, FileRepository, SessionRepository, SiteRepository,
    UserRepository, WebhookRepository,
};

#[derive(Clone)]
pub struct Repository {
    pub user: Arc<dyn UserRepository>,
    pub site: Arc<dyn SiteRepository>,
    pub entry: Arc<dyn EntryRepository>,
    pub collection: Arc<dyn CollectionRepository>,
    pub file: Arc<dyn FileRepository>,
    pub access_token: Arc<dyn AccessTokenRepository>,
    pub webhook: Arc<dyn WebhookRepository>,
    pub session: Arc<dyn SessionRepository>,
}

impl Repository {
    pub fn new(pool: &DbPool) -> Self {
        match pool {
            DbPool::Postgres(pg_pool) => Self {
                user: Arc::new(postgres::PostgresUserRepository::new(pg_pool.clone())),
                site: Arc::new(postgres::PostgresSiteRepository::new(pg_pool.clone())),
                entry: Arc::new(postgres::PostgresEntryRepository::new(pg_pool.clone())),
                collection: Arc::new(postgres::PostgresCollectionRepository::new(pg_pool.clone())),
                file: Arc::new(postgres::PostgresFileRepository::new(pg_pool.clone())),
                access_token: Arc::new(postgres::PostgresAccessTokenRepository::new(pg_pool.clone())),
                webhook: Arc::new(postgres::PostgresWebhookRepository::new(pg_pool.clone())),
                session: Arc::new(postgres::PostgresSessionRepository::new(pg_pool.clone())),
            },
            DbPool::Sqlite(sqlite_pool) => Self {
                user: Arc::new(sqlite::SqliteUserRepository::new(sqlite_pool.clone())),
                site: Arc::new(sqlite::SqliteSiteRepository::new(sqlite_pool.clone())),
                entry: Arc::new(sqlite::SqliteEntryRepository::new(sqlite_pool.clone())),
                collection: Arc::new(sqlite::SqliteCollectionRepository::new(sqlite_pool.clone())),
                file: Arc::new(sqlite::SqliteFileRepository::new(sqlite_pool.clone())),
                access_token: Arc::new(sqlite::SqliteAccessTokenRepository::new(sqlite_pool.clone())),
                webhook: Arc::new(sqlite::SqliteWebhookRepository::new(sqlite_pool.clone())),
                session: Arc::new(sqlite::SqliteSessionRepository::new(sqlite_pool.clone())),
            },
        }
    }
}
