use std::sync::Arc;

use async_trait::async_trait;
use cms::models::user::User;
use cms::repository::error::RepositoryError;
use cms::repository::traits::UserRepository;
use cms::services::auth::{AuthError, AuthService};

pub struct MockUserRepository {
    users: std::collections::HashMap<String, User>,
}

impl MockUserRepository {
    pub fn new() -> Self {
        Self {
            users: std::collections::HashMap::new(),
        }
    }

    pub fn add_user(&mut self, user: User) {
        self.users.insert(user.id.clone(), user);
    }
}

#[async_trait]
impl UserRepository for MockUserRepository {
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, RepositoryError> {
        Ok(self.users.values().find(|u| u.username == username).map(|u| User {
            id: u.id.clone(),
            username: u.username.clone(),
            email: u.email.clone(),
            password_hash: u.password_hash.clone(),
            created_at: u.created_at.clone(),
            updated_at: u.updated_at.clone(),
        }))
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        Ok(self.users.get(id).map(|u| User {
            id: u.id.clone(),
            username: u.username.clone(),
            email: u.email.clone(),
            password_hash: u.password_hash.clone(),
            created_at: u.created_at.clone(),
            updated_at: u.updated_at.clone(),
        }))
    }

    async fn find_id_by_username(&self, _username: &str) -> Result<Option<String>, RepositoryError> {
        unimplemented!()
    }

    async fn create(
        &self,
        _id: &str,
        _username: &str,
        _email: &str,
        _password_hash: &str,
    ) -> Result<(), RepositoryError> {
        unimplemented!()
    }

    async fn exists(&self, _username: &str) -> Result<bool, RepositoryError> {
        unimplemented!()
    }

    async fn get_role(&self, _user_id: &str, _site_id: &str) -> Result<Option<String>, RepositoryError> {
        unimplemented!()
    }
}

fn make_test_user() -> User {
    User {
        id: "user-123".to_string(),
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        password_hash: bcrypt::hash("password123", bcrypt::DEFAULT_COST).unwrap(),
        created_at: "2024-01-01 00:00:00".to_string(),
        updated_at: "2024-01-01 00:00:00".to_string(),
    }
}

#[tokio::test]
async fn test_auth_service_login_success() {
    let mut mock_repo = MockUserRepository::new();
    mock_repo.add_user(make_test_user());

    let auth_service = AuthService::new(Arc::new(mock_repo), "test-jwt-secret".to_string(), false);

    let result = auth_service.login("testuser", "password123").await;
    assert!(result.is_ok(), "Login should succeed with correct credentials");
    let (user, token) = result.unwrap();
    assert_eq!(user.username, "testuser");
    assert!(!token.is_empty());
}

#[tokio::test]
async fn test_auth_service_login_invalid_password() {
    let mut mock_repo = MockUserRepository::new();
    mock_repo.add_user(make_test_user());

    let auth_service = AuthService::new(Arc::new(mock_repo), "test-jwt-secret".to_string(), false);

    let result = auth_service.login("testuser", "wrong_password").await;
    assert!(matches!(result, Err(AuthError::InvalidCredentials)));
}

#[tokio::test]
async fn test_auth_service_login_user_not_found() {
    let mock_repo = MockUserRepository::new();

    let auth_service = AuthService::new(Arc::new(mock_repo), "test-jwt-secret".to_string(), false);

    let result = auth_service.login("nonexistent", "password").await;
    assert!(matches!(result, Err(AuthError::InvalidCredentials)));
}
