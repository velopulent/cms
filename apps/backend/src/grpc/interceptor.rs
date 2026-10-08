use std::sync::Arc;

use crate::config::Config;
use crate::middleware::auth::Actor;
use crate::models::authorization::Action;
use crate::repository::Repository;
use crate::services::authorization::AuthorizationService;
use crate::services::error::ServiceError;

/// Raw bearer credential captured by [`AuthInterceptor`]. Verification needs the
/// database, so it happens lazily in [`get_auth_context`].
#[derive(Clone)]
struct BearerToken {
    token: String,
    config: Arc<Config>,
}

/// Verified caller of one RPC.
#[derive(Clone, Debug)]
pub struct GrpcAuthContext {
    pub actor: Actor,
}

impl GrpcAuthContext {
    /// Every site-scoped request names its site; a site key only reaches its own.
    pub fn resolve_site_id(&self, requested: &str) -> Result<String, tonic::Status> {
        if requested.is_empty() {
            return Err(tonic::Status::invalid_argument("site_id is required"));
        }
        if self.actor.bound_site_id().is_some_and(|site_id| site_id != requested) {
            return Err(tonic::Status::permission_denied(
                "Token is not authorized for this site",
            ));
        }
        Ok(requested.to_string())
    }

    /// Authorize the site with both token scope and the user's live site/instance
    /// role: the same policy REST, GraphQL and MCP apply.
    pub async fn require_action(
        &self,
        repository: &Repository,
        site_id: &str,
        action: Action,
    ) -> Result<(), tonic::Status> {
        AuthorizationService::new(repository.user.clone())
            .require_site_action(&self.actor, site_id, action)
            .await
            .map_err(|error| match error {
                ServiceError::NotFound(_) => tonic::Status::not_found("Site not found"),
                ServiceError::InsufficientPermission(_)
                | ServiceError::Forbidden(_)
                | ServiceError::SiteTokenDenied => tonic::Status::permission_denied(error.error_message()),
                _ => tonic::Status::internal("Authorization service unavailable"),
            })
    }
}

#[derive(Clone)]
pub struct AuthInterceptor {
    config: Arc<Config>,
}

impl AuthInterceptor {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        let token = request
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(crate::middleware::auth::parse_bearer_header)
            .filter(|token| token.starts_with("vcms_site_") || token.starts_with("vcms_pat_"))
            .ok_or_else(|| tonic::Status::unauthenticated("Missing or malformed access token"))?
            .to_owned();
        request.extensions_mut().insert(BearerToken {
            token,
            config: self.config.clone(),
        });
        Ok(request)
    }
}

pub async fn get_auth_context<T>(
    request: &mut tonic::Request<T>,
    repository: &Repository,
) -> Result<GrpcAuthContext, tonic::Status> {
    if let Some(ctx) = request.extensions().get::<GrpcAuthContext>() {
        return Ok(ctx.clone());
    }
    let bearer = request
        .extensions()
        .get::<BearerToken>()
        .cloned()
        .ok_or_else(|| tonic::Status::internal("Missing auth context"))?;
    let actor = crate::middleware::auth::verify_access_token(&bearer.token, repository, &bearer.config.token_index_key)
        .await
        .map_err(|(status, error)| {
            if status.is_server_error() {
                tonic::Status::internal("Authentication service unavailable")
            } else {
                tonic::Status::unauthenticated(error.0.message.clone())
            }
        })?;
    let ctx = GrpcAuthContext { actor };
    request.extensions_mut().insert(ctx.clone());
    Ok(ctx)
}
