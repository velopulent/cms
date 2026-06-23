use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub instance_role: Option<String>,
    pub must_change_password: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateUser {
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    /// Login identity is the user's email address.
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateManagedUser {
    pub name: String,
    pub email: String,
    pub temporary_password: String,
    /// `"instance_owner"`, `"instance_admin"`, or `null` for a non-operator user.
    #[serde(default)]
    pub instance_role: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateInstanceRole {
    /// `"instance_owner"`, `"instance_admin"`, or `null` to clear the operator role.
    #[serde(default)]
    pub instance_role: Option<String>,
}

/// Operator-driven update of another user's display name and email.
#[derive(Deserialize, ToSchema)]
pub struct UpdateUserProfile {
    pub name: String,
    pub email: String,
}

/// Operator-driven password reset for another user.
#[derive(Deserialize, ToSchema)]
pub struct AdminSetPassword {
    pub new_password: String,
}

/// Self-service update of the signed-in user's own display name.
#[derive(Deserialize, ToSchema)]
pub struct UpdateSelfProfile {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

#[derive(Serialize, ToSchema)]
pub struct AuthResponse {
    pub user: UserPublic,
}

#[derive(Serialize, ToSchema)]
pub struct UserPublic {
    pub id: String,
    pub name: String,
    pub email: String,
    pub instance_role: Option<String>,
    pub must_change_password: bool,
}
