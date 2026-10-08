/// Lenient parse of the backend-specific timestamp texts: RFC3339, Postgres
/// `::text` (`YYYY-MM-DD HH:MM:SS[.fff]+00`), or naive UTC (SQLite).
pub fn parse_db_timestamp(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    if let Ok(dt) = chrono::DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f%#z") {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f")
        .ok()
        .map(|naive| naive.and_utc())
}

#[cfg(test)]
mod tests {
    #[test]
    fn future_token_expiry_accepts_both_database_formats() {
        let future = chrono::Utc::now() + chrono::Duration::hours(1);
        for value in [
            future.to_rfc3339(),
            future.format("%Y-%m-%d %H:%M:%S%.f+00").to_string(),
            future.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
        ] {
            assert!(crate::middleware::auth::is_token_not_expired(Some(&value)), "{value}");
        }
        let past = (chrono::Utc::now() - chrono::Duration::hours(1))
            .format("%Y-%m-%d %H:%M:%S%.f+00")
            .to_string();
        assert!(!crate::middleware::auth::is_token_not_expired(Some(&past)));
        assert!(!crate::middleware::auth::is_token_not_expired(Some("invalid")));
    }
}
