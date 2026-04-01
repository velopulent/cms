use async_graphql::Result;
use sqlx::SqlitePool;

use crate::middleware::auth::{AuthContext, verify_api_key, verify_token};

pub enum GqlAuth {
    Jwt { user_id: String },
    ApiKey { site_id: String },
}

pub struct GqlContext {
    pub pool: SqlitePool,
    pub config: crate::config::Config,
    pub storage: crate::handlers::file_handler::StorageManager,
    pub auth: Option<GqlAuth>,
}

impl GqlContext {
    pub async fn from_request(
        pool: SqlitePool,
        config: crate::config::Config,
        storage: crate::handlers::file_handler::StorageManager,
        auth_header: Option<&str>,
    ) -> Self {
        let mut ctx = Self {
            pool: pool.clone(),
            config: config.clone(),
            storage,
            auth: None,
        };

        if let Some(header) = auth_header {
            if let Some(token) = header.strip_prefix("Bearer ") {
                if token.starts_with("cms_") {
                    match verify_api_key(token, &pool).await {
                        Ok(AuthContext::ApiKey { site_id }) => {
                            ctx.auth = Some(GqlAuth::ApiKey { site_id });
                        }
                        _ => {}
                    }
                } else {
                    match verify_token(token, &config.jwt_secret) {
                        Ok(claims) => {
                            ctx.auth = Some(GqlAuth::Jwt { user_id: claims.sub });
                        }
                        Err(_) => {}
                    }
                }
            }
        }

        ctx
    }

    pub fn require_jwt(&self) -> Result<&str> {
        match &self.auth {
            Some(GqlAuth::Jwt { user_id }) => Ok(user_id),
            _ => Err(async_graphql::Error::new("Authentication required")),
        }
    }

    pub async fn require_site_access(&self, site_id: &str, min_role: &str) -> Result<String> {
        let user_id = self.require_jwt()?;

        let role_order = |r: &str| match r {
            "owner" => 4,
            "admin" => 3,
            "editor" => 2,
            "viewer" => 1,
            _ => 0,
        };

        let min_level = role_order(min_role);

        let result: Option<(String,)> = sqlx::query_as(
            "SELECT sm.role FROM site_members sm WHERE sm.user_id = ? AND sm.site_id = ?",
        )
        .bind(user_id)
        .bind(site_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match result {
            Some((role,)) if role_order(&role) >= min_level => Ok(user_id.to_string()),
            Some(_) => Err(async_graphql::Error::new("Insufficient permissions")),
            None => Err(async_graphql::Error::new("Site not found")),
        }
    }

    pub fn require_api_key_site(&self, site_id: &str) -> Result<()> {
        match &self.auth {
            Some(GqlAuth::ApiKey {
                site_id: key_site_id,
            }) => {
                if key_site_id == site_id {
                    Ok(())
                } else {
                    Err(async_graphql::Error::new(
                        "API key does not have access to this site",
                    ))
                }
            }
            _ => Err(async_graphql::Error::new("API key authentication required")),
        }
    }

    #[allow(dead_code)]
    pub fn require_any_auth(&self) -> Result<()> {
        match &self.auth {
            Some(_) => Ok(()),
            None => Err(async_graphql::Error::new("Authentication required")),
        }
    }
}
