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
