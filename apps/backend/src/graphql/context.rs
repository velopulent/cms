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
    pub site_id: Option<String>,
    pub config: Config,
}

impl GqlContext {
    pub async fn from_request(
        repository: Repository,
        services: Services,
        auth_header: Option<&str>,
        requested_site: Option<&str>,
        hmac_secret: &str,
        config: Config,
    ) -> Self {
        let mut actor = None;
        let mut site_id = None;

        if let Some(header) = auth_header
            && let Some(token) = crate::middleware::auth::parse_bearer_header(header)
            && (token.starts_with("vcms_site_") || token.starts_with("vcms_pat_"))
            && let Ok(auth_actor) = verify_access_token(token, &repository, hmac_secret).await
        {
            if let Actor::ApiKey(k) = &auth_actor {
                site_id = Some(k.site_id.clone());
            } else if let Actor::PersonalToken(_) = &auth_actor
                && let Some(requested_site) = requested_site.map(str::trim).filter(|site| !site.is_empty())
            {
                // Context selection is not authority. Each resolver checks its
                // actual action and live role, just like REST's site resolver.
                // Requiring site.read here broke content-only PAT/site-key parity.
                site_id = Some(requested_site.to_owned());
            }
            actor = Some(auth_actor);
        }

        Self {
            repository,
            services,
            actor,
            site_id,
            config,
        }
    }

    pub fn require_site(&self) -> async_graphql::Result<&str> {
        self.site_id
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("Site token authentication required"))
    }

    async fn require_action(&self, action: Action, message: &'static str) -> async_graphql::Result<()> {
        let actor = self
            .actor
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Site token authentication required"))?;
        let site_id = self.require_site()?;
        AuthorizationService::new(self.repository.user.clone())
            .require_site_action(actor, site_id, action)
            .await
            .map_err(|_| async_graphql::Error::new(message))
    }

    pub async fn require_site_action_for(&self, site_id: &str, action: Action) -> async_graphql::Result<()> {
        let actor = self
            .actor
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Authentication required"))?;
        AuthorizationService::new(self.repository.user.clone())
            .require_site_action(actor, site_id, action)
            .await
            .map_err(|error| crate::graphql::service_error("authorization", error))
    }

    pub async fn require_read(&self, action: Action) -> async_graphql::Result<()> {
        self.require_action(action, "Access token does not have required permission")
            .await
    }

    pub fn require_token_scope(&self, action: Action) -> async_graphql::Result<()> {
        let actor = self
            .actor
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Authentication required"))?;
        match actor {
            Actor::ApiKey(key) if crate::middleware::auth::scopes_allow_action(&key.scopes, action) => Ok(()),
            Actor::PersonalToken(token) if crate::middleware::auth::scopes_allow_action(&token.scopes, action) => {
                Ok(())
            }
            Actor::User(_) => Ok(()),
            _ => Err(async_graphql::Error::new(
                "Access token does not have required permission",
            )),
        }
    }

    pub async fn require_content_read(&self, include_drafts: bool) -> async_graphql::Result<()> {
        let action = if include_drafts {
            Action::ContentPreviewRead
        } else {
            Action::ContentRead
        };
        self.require_read(action).await
    }

    pub async fn require_write(&self, action: Action) -> async_graphql::Result<()> {
        self.require_action(action, "Access token does not have write permission")
            .await
    }

    pub fn require_site_match(&self, site_id: &str) -> async_graphql::Result<()> {
        let key_site = self.require_site()?;
        if key_site == site_id {
            Ok(())
        } else {
            Err(async_graphql::Error::new(
                "Site token does not have access to this site",
            ))
        }
    }

    pub fn user_id(&self) -> Option<&str> {
        self.actor.as_ref().and_then(|a| a.user_id())
    }
}
