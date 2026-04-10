use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Record not found")]
    NotFound,

    #[error("Unique constraint violated: {0}")]
    UniqueViolation(String),

    #[error("Database error: {0}")]
    Database(String),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_messages() {
        assert_eq!(RepositoryError::NotFound.to_string(), "Record not found");
        assert_eq!(
            RepositoryError::UniqueViolation("email_idx".into()).to_string(),
            "Unique constraint violated: email_idx"
        );
        assert_eq!(
            RepositoryError::Database("connection refused".into()).to_string(),
            "Database error: connection refused"
        );
        assert_eq!(
            RepositoryError::UniqueViolation(String::new()).to_string(),
            "Unique constraint violated: "
        );
    }

    #[test]
    fn test_error_debug_format() {
        assert!(format!("{:?}", RepositoryError::NotFound).contains("NotFound"));
        assert!(format!("{:?}", RepositoryError::UniqueViolation("x".into())).contains("UniqueViolation"));
    }
}
