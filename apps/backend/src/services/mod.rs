pub mod access_token;
pub mod auth;
pub mod authorization;
pub mod backup;
pub mod collection;
pub mod definition_validation;
pub mod entry;
pub mod error;
pub mod file;
pub mod singleton;
pub mod site;
pub mod webhook;

use std::sync::Arc;

use crate::config::Config;
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
    pub webhook: Arc<webhook::WebhookService>,
}

impl Services {
    pub fn new(repository: Arc<Repository>, config: &Config) -> Self {
        let config = Arc::new(config.clone());

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
            entry: Arc::new(entry::EntryService::new(
                repository.entry.clone(),
                repository.file.clone(),
                repository.collection.clone(),
            )),
            file: Arc::new(file::FileService::new(repository.file.clone(), config.clone())),
            singleton: Arc::new(singleton::SingletonService::new(
                repository.collection.clone(),
                repository.entry.clone(),
                repository.file.clone(),
            )),
            webhook: Arc::new(webhook::WebhookService::new(
                repository.webhook.clone(),
                &config.hmac_secret,
                config.webhook_allow_private_targets,
            )),
        }
    }
}
