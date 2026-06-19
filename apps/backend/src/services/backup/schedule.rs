//! Cron helpers for backup schedules.

use std::str::FromStr as _;

use chrono::Utc;
use croner::Cron;

use super::BackupError;

fn parse(expr: &str) -> Result<Cron, BackupError> {
    Cron::from_str(expr).map_err(|e| BackupError::Invalid(format!("invalid cron expression: {e}")))
}

/// Validate a cron expression without computing anything.
pub fn validate_cron(expr: &str) -> Result<(), BackupError> {
    parse(expr).map(|_| ())
}

/// Compute the next run time (ISO-8601 UTC) strictly after now.
pub fn next_run_iso(expr: &str) -> Result<String, BackupError> {
    let cron = parse(expr)?;
    let next = cron
        .find_next_occurrence(&Utc::now(), false)
        .map_err(|e| BackupError::Invalid(format!("cron has no next occurrence: {e}")))?;
    Ok(next.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}
