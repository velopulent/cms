use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Record not found")]
    NotFound,

    #[error("Unique constraint violated: {0}")]
    UniqueViolation(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for RepositoryError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => RepositoryError::NotFound,
            sqlx::Error::Database(db) => {
                if db.is_unique_violation() {
                    RepositoryError::UniqueViolation(db.to_string())
                } else {
                    RepositoryError::Database(db.to_string())
                }
            }
            _ => RepositoryError::Database(err.to_string()),
        }
    }
}
