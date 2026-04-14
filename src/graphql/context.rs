use crate::repository::Repository;

use crate::middleware::auth::{Principal, verify_access_token};

pub struct GqlContext {
    pub repository: Repository,
    pub storage: crate::handlers::file_handler::StorageManager,
    pub principal: Option<Principal>,
    pub site_id: Option<String>,
    pub scopes: std::collections::BTreeSet<String>,
}

impl GqlContext {
    pub async fn from_request(
        repository: Repository,
        storage: crate::handlers::file_handler::StorageManager,
        auth_header: Option<&str>,
        hmac_secret: &str,
    ) -> Self {
        let mut principal = None;
        let mut site_id = None;
        let mut scopes = std::collections::BTreeSet::new();

        if let Some(header) = auth_header {
            if let Some(token) = header.strip_prefix("Bearer ") {
                if token.starts_with("cms_") {
                    if let Ok(auth_principal) = verify_access_token(token, &repository, hmac_secret).await {
                        match &auth_principal {
                            Principal::SiteToken {
                                site_id: key_site_id,
                                scopes: token_scopes,
                                ..
                            } => {
                                site_id = Some(key_site_id.clone());
                                scopes = token_scopes.clone();
                            }
                            Principal::InstanceToken { scopes: token_scopes, .. } => {
                                scopes = token_scopes.clone();
                            }
                            Principal::UserSession { .. } => {}
                        }
                        principal = Some(auth_principal);
                    }
                }
            }
        }

        Self {
            repository,
            storage,
            principal,
            site_id,
            scopes,
        }
    }

    pub fn require_site(&self) -> async_graphql::Result<&str> {
        self.site_id
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("Site token authentication required"))
    }

    pub fn require_instance_scope(&self, scope: &str) -> async_graphql::Result<()> {
        match &self.principal {
            Some(Principal::InstanceToken { scopes, .. }) if scopes.contains(scope) => Ok(()),
            Some(Principal::InstanceToken { .. }) => {
                Err(async_graphql::Error::new("Access token does not have the required admin scope"))
            }
            _ => Err(async_graphql::Error::new("Instance token authentication required")),
        }
    }

    pub fn require_site_scope(&self, scope: &str) -> async_graphql::Result<()> {
        match &self.principal {
            Some(Principal::SiteToken { scopes, .. }) if scopes.contains(scope) => Ok(()),
            Some(Principal::SiteToken { .. }) => {
                Err(async_graphql::Error::new("Access token does not have the required scope"))
            }
            _ => Err(async_graphql::Error::new("Site token authentication required")),
        }
    }

    pub fn require_write(&self) -> async_graphql::Result<()> {
        if self.scopes.contains(crate::middleware::auth::SCOPE_CONTENT_WRITE)
            || self.scopes.contains(crate::middleware::auth::SCOPE_SCHEMA_WRITE)
            || self.scopes.contains(crate::middleware::auth::SCOPE_ASSETS_WRITE)
        {
            Ok(())
        } else if self.site_id.is_none() {
            Err(async_graphql::Error::new("Site token authentication required"))
        } else {
            Err(async_graphql::Error::new("Access token does not have write scope"))
        }
    }

    #[allow(dead_code)]
    pub fn require_site_match(&self, site_id: &str) -> async_graphql::Result<()> {
        let key_site = self.require_site()?;
        if key_site == site_id {
            Ok(())
        } else {
            Err(async_graphql::Error::new("Site token does not have access to this site"))
        }
    }
}
