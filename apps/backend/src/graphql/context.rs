use crate::middleware::auth::{Actor, verify_access_token};
use crate::models::access_token::{TokenScope, TokenScopes};
use crate::models::authorization::Action;
use crate::repository::Repository;
use crate::services::Services;
use crate::services::authorization::AuthorizationService;

pub struct GqlContext {
    pub repository: Repository,
    pub services: Services,
    pub actor: Option<Actor>,
    pub site_id: Option<String>,
    pub permission: Option<TokenScopes>,
}

impl GqlContext {
    pub async fn from_request(
        repository: Repository,
        services: Services,
        auth_header: Option<&str>,
        requested_site: Option<&str>,
        hmac_secret: &str,
    ) -> Self {
        let mut actor = None;
        let mut site_id = None;
        let mut permission = None;

        if let Some(header) = auth_header
            && let Some(token) = header.strip_prefix("Bearer ")
            && (token.starts_with("vcms_site_") || token.starts_with("vcms_pat_"))
            && let Ok(auth_actor) = verify_access_token(token, &repository, hmac_secret).await
        {
            if let Actor::ApiKey(k) = &auth_actor {
                site_id = Some(k.site_id.clone());
                permission = Some(k.scopes.clone());
            } else if let Actor::PersonalToken(k) = &auth_actor {
                permission = Some(k.scopes.clone());
                if let Some(requested_site) = requested_site
                    && AuthorizationService::new(repository.user.clone())
                        .require_site_action(&auth_actor, requested_site, Action::SiteRead)
                        .await
                        .is_ok()
                {
                    site_id = Some(requested_site.to_owned());
                }
            }
            actor = Some(auth_actor);
        }

        Self {
            repository,
            services,
            actor,
            site_id,
            permission,
        }
    }

    pub fn require_site(&self) -> async_graphql::Result<&str> {
        self.site_id
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("Site token authentication required"))
    }

    pub fn require_read(&self) -> async_graphql::Result<()> {
        match (&self.actor, &self.permission) {
            (Some(Actor::ApiKey(_) | Actor::PersonalToken(_)), Some(scopes))
                if scopes.contains(&TokenScope::ContentRead) =>
            {
                Ok(())
            }
            (Some(Actor::ApiKey(_) | Actor::PersonalToken(_)), None) => Err(async_graphql::Error::new(
                "Access token does not have required permission",
            )),
            _ => Err(async_graphql::Error::new("Site token authentication required")),
        }
    }

    pub fn require_write(&self) -> async_graphql::Result<()> {
        match &self.permission {
            Some(scopes) if scopes.contains(&TokenScope::ContentWrite) => Ok(()),
            Some(_) => Err(async_graphql::Error::new("Access token does not have write permission")),
            None => Err(async_graphql::Error::new("Site token authentication required")),
        }
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
