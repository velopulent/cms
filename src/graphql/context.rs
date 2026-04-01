use sqlx::SqlitePool;

use crate::middleware::auth::verify_api_key;

/// GraphQL context injected per-request.
/// Only API key authentication is supported — JWT is for the dashboard REST API.
pub struct GqlContext {
    pub pool: SqlitePool,
    pub storage: crate::handlers::file_handler::StorageManager,
    /// The site_id this API key grants access to, if authenticated.
    pub site_id: Option<String>,
}

impl GqlContext {
    pub async fn from_request(
        pool: SqlitePool,
        storage: crate::handlers::file_handler::StorageManager,
        auth_header: Option<&str>,
    ) -> Self {
        let mut site_id = None;

        if let Some(header) = auth_header {
            if let Some(token) = header.strip_prefix("Bearer ") {
                if token.starts_with("cms_") {
                    if let Ok(crate::middleware::auth::AuthContext::ApiKey {
                        site_id: key_site_id,
                    }) = verify_api_key(token, &pool).await
                    {
                        site_id = Some(key_site_id);
                    }
                }
            }
        }

        Self {
            pool,
            storage,
            site_id,
        }
    }

    /// Require that an API key is present and return its site_id.
    pub fn require_site(&self) -> async_graphql::Result<&str> {
        self.site_id
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("API key authentication required"))
    }

    /// Require that the API key's site matches the requested site_id.
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
