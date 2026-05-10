use std::collections::BTreeSet;
use std::sync::Arc;

use crate::middleware::auth::{
    Principal, SCOPE_MEMBERS_READ, SCOPE_MEMBERS_WRITE, SCOPE_SITES_DELETE, SCOPE_SITES_READ, SCOPE_SITES_WRITE,
    SCOPE_TOKENS_READ, SCOPE_TOKENS_WRITE, SCOPE_WEBHOOKS_READ, SCOPE_WEBHOOKS_TRIGGER, SCOPE_WEBHOOKS_WRITE,
};
use crate::repository::traits::UserRepository;
use crate::services::error::ServiceError;

#[derive(Clone)]
pub struct ScopeChecker {
    user_repo: Arc<dyn UserRepository>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use crate::middleware::auth::{Principal, SCOPE_CONTENT_READ};
    use crate::test_helpers::InMemoryUserRepository;

    use super::ScopeChecker;

    #[tokio::test]
    async fn site_token_uses_bound_site_when_site_id_is_omitted() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));
        let principal = Principal::SiteToken {
            token_id: "token-1".to_string(),
            site_id: "site-1".to_string(),
            scopes: BTreeSet::from([SCOPE_CONTENT_READ.to_string()]),
        };

        let site = checker
            .require_site_scope(&principal, None, SCOPE_CONTENT_READ, "viewer")
            .await
            .expect("bound site should be used");

        assert_eq!(site.site_id, "site-1");
    }

    #[tokio::test]
    async fn site_token_rejects_mismatched_explicit_site_id() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));
        let principal = Principal::SiteToken {
            token_id: "token-1".to_string(),
            site_id: "site-1".to_string(),
            scopes: BTreeSet::from([SCOPE_CONTENT_READ.to_string()]),
        };

        let result = checker
            .require_site_scope(&principal, Some("site-2"), SCOPE_CONTENT_READ, "viewer")
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn instance_token_still_requires_explicit_site_id() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));
        let principal = Principal::InstanceToken {
            token_id: "token-1".to_string(),
            scopes: BTreeSet::from([SCOPE_CONTENT_READ.to_string()]),
        };

        let result = checker
            .require_site_scope(&principal, None, SCOPE_CONTENT_READ, "viewer")
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn instance_token_uses_explicit_site_id_when_scope_allows() {
        let checker = ScopeChecker::new(Arc::new(InMemoryUserRepository::new()));
        let principal = Principal::InstanceToken {
            token_id: "token-1".to_string(),
            scopes: BTreeSet::from([SCOPE_CONTENT_READ.to_string()]),
        };

        let site = checker
            .require_site_scope(&principal, Some("site-1"), SCOPE_CONTENT_READ, "viewer")
            .await
            .expect("instance token should use explicit site");

        assert_eq!(site.site_id, "site-1");
    }
}

impl ScopeChecker {
    pub fn new(user_repo: Arc<dyn UserRepository>) -> Self {
        Self { user_repo }
    }

    pub async fn require_admin_scope(
        &self,
        principal: &Principal,
        site_id: Option<&str>,
        scope: &str,
    ) -> Result<(), ServiceError> {
        match principal {
            Principal::InstanceToken { scopes, .. } => {
                if scopes.contains(scope) {
                    Ok(())
                } else {
                    Err(ServiceError::InsufficientScope(scope.to_string()))
                }
            }
            Principal::UserSession { user_id } => match scope {
                SCOPE_SITES_READ | SCOPE_SITES_WRITE => Ok(()),
                SCOPE_SITES_DELETE => {
                    let site_id = site_id.ok_or_else(|| ServiceError::BadRequest("Site id is required".into()))?;
                    self.check_site_access(user_id, site_id, "owner").await
                }
                SCOPE_MEMBERS_READ | SCOPE_WEBHOOKS_READ => {
                    let site_id = site_id.ok_or_else(|| ServiceError::BadRequest("Site id is required".into()))?;
                    self.check_site_access(user_id, site_id, "viewer").await
                }
                SCOPE_MEMBERS_WRITE | SCOPE_TOKENS_READ | SCOPE_TOKENS_WRITE | SCOPE_WEBHOOKS_WRITE => {
                    let site_id = site_id.ok_or_else(|| ServiceError::BadRequest("Site id is required".into()))?;
                    self.check_site_access(user_id, site_id, "admin").await
                }
                SCOPE_WEBHOOKS_TRIGGER => {
                    let site_id = site_id.ok_or_else(|| ServiceError::BadRequest("Site id is required".into()))?;
                    self.check_site_access(user_id, site_id, "editor").await
                }
                _ => Err(ServiceError::Forbidden("Unsupported admin scope".into())),
            },
            Principal::SiteToken { .. } => Err(ServiceError::SiteTokenDenied),
        }
    }

    pub async fn resolve_site_context(
        &self,
        principal: &Principal,
        explicit_site_id: Option<&str>,
    ) -> Result<SiteContext, ServiceError> {
        match principal {
            Principal::SiteToken { site_id, .. } => {
                if let Some(explicit) = explicit_site_id {
                    if explicit != site_id {
                        return Err(ServiceError::Forbidden(
                            "Site token does not have access to this site".into(),
                        ));
                    }
                }
                Ok(SiteContext {
                    site_id: site_id.clone(),
                })
            }
            Principal::UserSession { user_id } => {
                let site_id = explicit_site_id.ok_or_else(|| ServiceError::MissingSiteContext)?;
                self.check_site_access(user_id, site_id, "viewer").await?;
                Ok(SiteContext {
                    site_id: site_id.to_string(),
                })
            }
            Principal::InstanceToken { .. } => {
                let site_id = explicit_site_id.ok_or_else(|| ServiceError::MissingSiteContext)?;
                Ok(SiteContext {
                    site_id: site_id.to_string(),
                })
            }
        }
    }

    pub async fn require_site_scope(
        &self,
        principal: &Principal,
        explicit_site_id: Option<&str>,
        scope: &str,
        min_role: &str,
    ) -> Result<SiteContext, ServiceError> {
        let site = self.resolve_site_context(principal, explicit_site_id).await?;

        match principal {
            Principal::SiteToken { scopes, .. } => {
                if scopes.contains(scope) {
                    Ok(site)
                } else {
                    Err(ServiceError::InsufficientScope(scope.to_string()))
                }
            }
            Principal::UserSession { user_id } => {
                self.check_site_access(user_id, &site.site_id, min_role).await?;
                Ok(site)
            }
            Principal::InstanceToken { scopes, .. } => {
                if scopes.contains(scope) {
                    Ok(site)
                } else {
                    Err(ServiceError::InsufficientScope(scope.to_string()))
                }
            }
        }
    }

    pub async fn admin_site_context(&self, principal: &Principal, site_id: &str) -> Result<SiteContext, ServiceError> {
        match principal {
            Principal::InstanceToken { .. } => Ok(SiteContext {
                site_id: site_id.to_string(),
            }),
            Principal::UserSession { user_id } => {
                self.check_site_access(user_id, site_id, "viewer").await?;
                Ok(SiteContext {
                    site_id: site_id.to_string(),
                })
            }
            Principal::SiteToken {
                site_id: token_site_id, ..
            } => {
                if token_site_id != site_id {
                    return Err(ServiceError::Forbidden(
                        "Site token does not have access to this site".into(),
                    ));
                }
                Ok(SiteContext {
                    site_id: site_id.to_string(),
                })
            }
        }
    }

    pub fn get_site_id(&self, principal: &Principal, explicit: Option<&str>) -> Result<String, ServiceError> {
        match principal {
            Principal::SiteToken { site_id, .. } => {
                if let Some(explicit) = explicit {
                    if explicit != site_id {
                        return Err(ServiceError::Forbidden(
                            "Site token does not have access to this site".into(),
                        ));
                    }
                }
                Ok(site_id.clone())
            }
            Principal::UserSession { .. } | Principal::InstanceToken { .. } => {
                explicit.map(String::from).ok_or(ServiceError::MissingSiteContext)
            }
        }
    }

    pub fn check_scope(&self, principal: &Principal, scope: &str) -> Result<(), ServiceError> {
        match principal {
            Principal::InstanceToken { scopes, .. } => {
                if scopes.contains(scope) {
                    Ok(())
                } else {
                    Err(ServiceError::InsufficientScope(scope.to_string()))
                }
            }
            Principal::UserSession { .. } => Ok(()),
            Principal::SiteToken { scopes, .. } => {
                if scopes.contains(scope) {
                    Ok(())
                } else {
                    Err(ServiceError::InsufficientScope(scope.to_string()))
                }
            }
        }
    }

    pub fn principal_user_id<'a>(&self, principal: &'a Principal) -> Option<&'a str> {
        principal.user_id()
    }

    pub fn principal_site_id<'a>(&self, principal: &'a Principal) -> Option<&'a str> {
        principal.bound_site_id()
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

#[derive(Debug, Clone)]
pub struct SiteContext {
    pub site_id: String,
}

#[derive(Debug, Clone)]
pub struct AdminSiteContext {
    pub site_id: String,
    pub scopes: Option<BTreeSet<String>>,
}
