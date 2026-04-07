use crate::repository::Repository;

use crate::middleware::auth::verify_api_key;

pub struct GqlContext {
    pub repository: Repository,
    pub storage: crate::handlers::file_handler::StorageManager,
    pub site_id: Option<String>,
    pub permissions: Option<String>,
}

impl GqlContext {
    pub async fn from_request(
        repository: Repository,
        storage: crate::handlers::file_handler::StorageManager,
        auth_header: Option<&str>,
    ) -> Self {
        let mut site_id = None;
        let mut permissions = None;

        if let Some(header) = auth_header {
            if let Some(token) = header.strip_prefix("Bearer ") {
                if token.starts_with("cms_") {
                    if let Ok(crate::middleware::auth::AuthContext::ApiKey {
                        site_id: key_site_id,
                        permissions: key_permissions,
                    }) = verify_api_key(token, &repository).await
                    {
                        site_id = Some(key_site_id);
                        permissions = Some(key_permissions);
                    }
                }
            }
        }

        Self {
            repository,
            storage,
            site_id,
            permissions,
        }
    }

    pub fn require_site(&self) -> async_graphql::Result<&str> {
        self.site_id
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("API key authentication required"))
    }

    pub fn require_write(&self) -> async_graphql::Result<()> {
        match self.permissions.as_deref() {
            Some("write") => Ok(()),
            Some("read") => Err(async_graphql::Error::new(
                "API key does not have write permissions",
            )),
            None => Err(async_graphql::Error::new(
                "API key authentication required",
            )),
            Some(other) => Err(async_graphql::Error::new(format!(
                "Unknown API key permission level: {}",
                other
            ))),
        }
    }

    #[allow(dead_code)]
    pub fn require_site_match(&self, site_id: &str) -> async_graphql::Result<()> {
        let key_site = self.require_site()?;
        if key_site == site_id {
            Ok(())
        } else {
            Err(async_graphql::Error::new(
                "API key does not have access to this site",
            ))
        }
    }
}
