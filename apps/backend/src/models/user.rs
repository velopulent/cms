use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
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
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateManagedUser {
    pub username: String,
    pub email: String,
    pub temporary_password: String,
    pub instance_owner: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateInstanceRole {
    pub instance_owner: bool,
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
    pub username: String,
    pub email: String,
    pub instance_role: Option<String>,
    pub must_change_password: bool,
}
