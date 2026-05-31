use std::sync::Arc;

use crate::middleware::auth::{Actor, Scope};
use crate::repository::traits::UserRepository;
use crate::services::error::ServiceError;

#[derive(Clone)]
pub struct ScopeChecker {
    user_repo: Arc<dyn UserRepository>,
}

impl ScopeChecker {
    pub fn new(user_repo: Arc<dyn UserRepository>) -> Self {
        Self { user_repo }
    }

    pub async fn require_site_scope(
        &self,
        actor: &Actor,
        site_id: &str,
        scope: &Scope,
        min_role: &str,
    ) -> Result<(), ServiceError> {
        match actor {
            Actor::ApiKey(k) => {
                if site_id != k.site_id {
                    return Err(ServiceError::Forbidden(
                        "Token is not authorized for this site".into(),
                    ));
                }
                let scopes = crate::middleware::auth::ScopeSet::from_permission(&k.permission);
                if scopes.allows(scope) {
                    Ok(())
                } else {
                    Err(ServiceError::InsufficientPermission("write".into()))
                }
            }
            Actor::User(user) => {
                self.check_site_access(&user.user_id, site_id, min_role).await
            }
        }
    }

    pub fn actor_user_id<'a>(&self, actor: &'a Actor) -> Option<&'a str> {
        actor.user_id()
    }

    pub fn actor_site_id<'a>(&self, actor: &'a Actor) -> Option<&'a str> {
        actor.bound_site_id()
    }

    async fn check_site_access(&self, user_id: &str, site_id: &str, min_role: &str) -> Result<(), ServiceError> {
        let role_order = |r: &str| match r {
            "owner" => 4,
            "admin" => 3,
            "editor" => 2,
            "viewer" => 1,
            _ => 0,
        };

        let min_level = role_order(min_role);

        let role = self
            .user_repo
            .get_role(user_id, site_id)
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        match role {
            Some(r) if role_order(&r) >= min_level => Ok(()),
            Some(_) => Err(ServiceError::Forbidden(format!(
                "Insufficient role: requires {}",
                min_role
            ))),
            None => Err(ServiceError::NotFound("Site not found".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::middleware::auth::{Actor, ApiKeyActor, Scope};
    use crate::models::access_token::AccessTokenPermission;
    use crate::test_helpers::InMemoryUserRepository;

    use super::ScopeChecker;

    #[tokio::test]
    async fn site_token_has_read_scope() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));
        let actor = Actor::ApiKey(ApiKeyActor {
            token_id: "token-1".to_string(),
            site_id: "site-1".to_string(),
            permission: AccessTokenPermission::Read,
        });

        let result = checker
            .require_site_scope(&actor, "site-1", &Scope::EntriesRead, "viewer")
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn read_site_token_rejects_write_scope() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));
        let actor = Actor::ApiKey(ApiKeyActor {
            token_id: "token-1".to_string(),
            site_id: "site-1".to_string(),
            permission: AccessTokenPermission::Read,
        });

        let result = checker
            .require_site_scope(&actor, "site-1", &Scope::EntriesWrite, "editor")
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn api_key_rejects_cross_site_access() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));
        let actor = Actor::ApiKey(ApiKeyActor {
            token_id: "token-1".to_string(),
            site_id: "site-1".to_string(),
            permission: AccessTokenPermission::Read,
        });

        let result = checker
            .require_site_scope(&actor, "site-2", &Scope::EntriesRead, "viewer")
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn write_token_has_write_scope() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));
        let actor = Actor::ApiKey(ApiKeyActor {
            token_id: "token-1".to_string(),
            site_id: "site-1".to_string(),
            permission: AccessTokenPermission::Write,
        });

        let result = checker
            .require_site_scope(&actor, "site-1", &Scope::EntriesWrite, "editor")
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn user_actor_requires_site_access() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));
        let actor = Actor::User(crate::middleware::auth::UserActor {
            user_id: "user-1".to_string(),
        });

        let result = checker
            .require_site_scope(&actor, "site-1", &Scope::EntriesRead, "viewer")
            .await;

        assert!(result.is_ok());
    }

    #[test]
    fn test_actor_user_id() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));

        let user_actor = Actor::User(crate::middleware::auth::UserActor {
            user_id: "user-1".to_string(),
        });
        assert_eq!(checker.actor_user_id(&user_actor), Some("user-1"));

        let api_actor = Actor::ApiKey(ApiKeyActor {
            token_id: "token-1".to_string(),
            site_id: "site-1".to_string(),
            permission: AccessTokenPermission::Read,
        });
        assert!(checker.actor_user_id(&api_actor).is_none());
    }

    #[test]
    fn test_actor_site_id() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));

        let api_actor = Actor::ApiKey(ApiKeyActor {
            token_id: "token-1".to_string(),
            site_id: "site-1".to_string(),
            permission: AccessTokenPermission::Read,
        });
        assert_eq!(checker.actor_site_id(&api_actor), Some("site-1"));

        let user_actor = Actor::User(crate::middleware::auth::UserActor {
            user_id: "user-1".to_string(),
        });
        assert!(checker.actor_site_id(&user_actor).is_none());
    }
}
