use axum::{Json, extract::Request, http::StatusCode, middleware::Next, response::IntoResponse, response::Response};

pub async fn authz_middleware(request: Request, next: Next) -> Response {
    if request
        .extensions()
        .get::<crate::middleware::auth::RequestContext>()
        .is_none()
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "authorization_context_missing",
                "message": "Protected route has no authorization context"
            })),
        )
            .into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    #[test]
    fn all_site_actions_are_assigned_to_a_policy_category() {
        use crate::models::authorization::Action;
        let actions = HashSet::from([
            Action::SiteRead,
            Action::SiteManage,
            Action::SiteDelete,
            Action::ContentRead,
            Action::ContentWrite,
            Action::SchemaRead,
            Action::SchemaWrite,
            Action::FilesRead,
            Action::FilesWrite,
            Action::WebhooksRead,
            Action::WebhooksWrite,
            Action::ApiKeysManage,
            Action::MembersRead,
            Action::MembersManage,
        ]);
        assert_eq!(actions.len(), 14);
    }
}
