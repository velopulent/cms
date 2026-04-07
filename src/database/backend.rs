use sqlx::Error;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseBackend {
    Postgres,
    MySQL,
    SQLite,
}

impl DatabaseBackend {
    pub fn from_url(url: &str) -> Option<Self> {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            Some(DatabaseBackend::Postgres)
        } else if url.starts_with("mysql://") {
            Some(DatabaseBackend::MySQL)
        } else if url.starts_with("sqlite:") {
            Some(DatabaseBackend::SQLite)
        } else {
            None
        }
    }

    pub fn datetime_now(&self) -> &'static str {
        match self {
            DatabaseBackend::Postgres => "NOW()",
            DatabaseBackend::MySQL => "NOW()",
            DatabaseBackend::SQLite => "datetime('now')",
        }
    }

    pub fn placeholder(&self, index: u32) -> String {
        match self {
            DatabaseBackend::Postgres => format!("${}", index),
            DatabaseBackend::MySQL => "?".to_string(),
            DatabaseBackend::SQLite => "?".to_string(),
        }
    }

    pub fn is_unique_violation(&self, error: &Error) -> bool {
        match self {
            DatabaseBackend::Postgres => {
                if let Error::Database(db) = error {
                    return db.code().map(|c| c == "23505").unwrap_or(false);
                }
                false
            }
            DatabaseBackend::MySQL => {
                if let Error::Database(db) = error {
                    return db.code().map(|c| c == "23000").unwrap_or(false);
                }
                false
            }
            DatabaseBackend::SQLite => {
                if let Error::Database(db) = error {
                    return db
                        .code()
                        .map(|c| c == "1555" || c == "2067")
                        .unwrap_or(false);
                }
                false
            }
        }
    }

    pub fn is_not_found(&self, error: &Error) -> bool {
        matches!(error, Error::RowNotFound)
    }
}

impl FromStr for DatabaseBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgres" | "postgresql" => Ok(DatabaseBackend::Postgres),
            "mysql" => Ok(DatabaseBackend::MySQL),
            "sqlite" => Ok(DatabaseBackend::SQLite),
            _ => Err(format!("Unknown database backend: {}", s)),
        }
    }
}
