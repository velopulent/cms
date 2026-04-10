pub mod error;
pub mod mysql;
pub mod postgres;
pub mod sqlite;
pub mod traits;

use std::sync::Arc;

use crate::database::pool::DbPool;
use crate::repository::traits::{ApiKeyRepository, CollectionRepository, EntryRepository, FileRepository, SiteRepository, UserRepository};

#[derive(Clone)]
pub struct Repository {
    pub user: Arc<dyn UserRepository>,
    pub site: Arc<dyn SiteRepository>,
    pub entry: Arc<dyn EntryRepository>,
    pub collection: Arc<dyn CollectionRepository>,
    pub file: Arc<dyn FileRepository>,
    pub api_key: Arc<dyn ApiKeyRepository>,
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
                api_key: Arc::new(postgres::PostgresApiKeyRepository::new(pg_pool.clone())),
            },
            DbPool::MySql(mysql_pool) => Self {
                user: Arc::new(mysql::MysqlUserRepository::new(mysql_pool.clone())),
                site: Arc::new(mysql::MysqlSiteRepository::new(mysql_pool.clone())),
                entry: Arc::new(mysql::MysqlEntryRepository::new(mysql_pool.clone())),
                collection: Arc::new(mysql::MysqlCollectionRepository::new(mysql_pool.clone())),
                file: Arc::new(mysql::MysqlFileRepository::new(mysql_pool.clone())),
                api_key: Arc::new(mysql::MysqlApiKeyRepository::new(mysql_pool.clone())),
            },
            DbPool::Sqlite(sqlite_pool) => Self {
                user: Arc::new(sqlite::SqliteUserRepository::new(sqlite_pool.clone())),
                site: Arc::new(sqlite::SqliteSiteRepository::new(sqlite_pool.clone())),
                entry: Arc::new(sqlite::SqliteEntryRepository::new(sqlite_pool.clone())),
                collection: Arc::new(sqlite::SqliteCollectionRepository::new(sqlite_pool.clone())),
                file: Arc::new(sqlite::SqliteFileRepository::new(sqlite_pool.clone())),
                api_key: Arc::new(sqlite::SqliteApiKeyRepository::new(sqlite_pool.clone())),
            },
        }
    }
}
