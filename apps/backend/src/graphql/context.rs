use crate::config::Config;
use crate::middleware::auth::{Actor, verify_access_token};
use crate::models::authorization::Action;
use crate::repository::Repository;
use crate::services::Services;
use crate::services::authorization::AuthorizationService;

pub struct GqlContext {
    pub repository: Repository,
    pub services: Services,
    pub actor: Option<Actor>,
    pub config: Config,
}

impl GqlContext {
    pub async fn from_request(
        repository: Repository,
        services: Services,
        auth_header: Option<&str>,
        hmac_secret: &str,
        config: Config,
    ) -> Self {
        let mut actor = None;
        if let Some(header) = auth_header
            && let Some(token) = crate::middleware::auth::parse_bearer_header(header)
            && (token.starts_with("vcms_site_") || token.starts_with("vcms_pat_"))
            && let Ok(auth_actor) = verify_access_token(token, &repository, hmac_secret).await
        {
            actor = Some(auth_actor);
        }
        Self {
            repository,
            services,
            actor,
            config,
        }
    }

    pub fn require_actor(&self) -> async_graphql::Result<&Actor> {
        self.actor
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Authentication required"))
    }

    /// Authorize `action` on `site_id` with token scope and the live site role.
    pub async fn require_site_action_for(&self, site_id: &str, action: Action) -> async_graphql::Result<()> {
        AuthorizationService::new(self.repository.user.clone())
            .require_site_action(self.require_actor()?, site_id, action)
            .await
            .map_err(|error| crate::graphql::service_error("authorization", error))
    }

    /// Scope gate for operations that span sites, such as `sites`.
    pub fn require_token_scope(&self, action: Action) -> async_graphql::Result<()> {
        let scopes = match self.require_actor()? {
            Actor::ApiKey(key) => &key.scopes,
            Actor::PersonalToken(token) => &token.scopes,
            Actor::User(_) => return Ok(()),
        };
        if crate::middleware::auth::scopes_allow_action(scopes, action) {
            Ok(())
        } else {
            let scope = crate::middleware::auth::scope_for_action(action).map_or("unavailable", |scope| scope.as_str());
            Err(async_graphql::Error::new(format!(
                "Token requires the '{scope}' scope."
            )))
        }
    }

    pub fn user_id(&self) -> Option<&str> {
        self.actor.as_ref().and_then(|a| a.user_id())
    }
}
