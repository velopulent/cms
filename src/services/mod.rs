pub mod access_token;
pub mod auth;
pub mod collection;
pub mod entry;
pub mod file;
pub mod singleton;
pub mod site;

use std::sync::Arc;

use crate::config::Config;
use crate::handlers::file_handler::StorageManager;
use crate::repository::Repository;

#[derive(Clone)]
pub struct Services {
    pub auth: Arc<auth::AuthService>,
    pub site: Arc<site::SiteService>,
    pub access_token: Arc<access_token::AccessTokenService>,
    pub collection: Arc<collection::CollectionService>,
    pub entry: Arc<entry::EntryService>,
    pub file: Arc<file::FileService>,
    pub singleton: Arc<singleton::SingletonService>,
}

impl Services {
    pub fn new(repository: Repository, config: &Config, storage: StorageManager) -> Self {
        let repository = Arc::new(repository);
        let config = Arc::new(config.clone());

        Self {
            auth: Arc::new(auth::AuthService::new(
                repository.clone(),
                config.jwt_secret.clone(),
                config.cookie_secure,
            )),
            site: Arc::new(site::SiteService::new(repository.clone())),
            access_token: Arc::new(access_token::AccessTokenService::new(
                repository.clone(),
                config.hmac_secret.clone(),
            )),
            collection: Arc::new(collection::CollectionService::new(repository.clone())),
            entry: Arc::new(entry::EntryService::new(repository.clone(), storage.clone())),
            file: Arc::new(file::FileService::new(
                repository.clone(),
                storage.clone(),
                config.clone(),
            )),
            singleton: Arc::new(singleton::SingletonService::new(repository.clone(), storage)),
        }
    }
}
