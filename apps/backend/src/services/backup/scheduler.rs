//! Background poller that runs due backup schedules.
//!
//! A single task wakes on an interval, selects schedules whose `next_run_at` is
//! due, advances each schedule's `next_run_at` *before* running (so a slow backup
//! can't double-fire), then runs the backup. Runs sequentially within the task,
//! so there is no in-process overlap.

use std::sync::Arc;
use std::time::Duration;

use super::{BackupError, BackupService, CreateBackupOptions, Scope, now_iso, schedule};

const POLL_INTERVAL: Duration = Duration::from_secs(60);

/// Run the scheduler loop forever. Spawn this from the serve path.
pub async fn run(service: Arc<BackupService>) {
    let mut ticker = tokio::time::interval(POLL_INTERVAL);
    loop {
        ticker.tick().await;
        if let Err(e) = tick(&service).await {
            tracing::error!(error = %e, "backup scheduler tick failed");
        }
    }
}

async fn tick(service: &BackupService) -> Result<(), BackupError> {
    let now = now_iso();
    let due = super::meta::due_schedules(service.pool(), &now).await?;
    for sched in due {
        let scope = match sched.scope.as_str() {
            "site" => match sched.site_id.clone() {
                Some(s) => Scope::Site(s),
                None => continue,
            },
            _ => Scope::Instance,
        };

        // Advance next_run before running so a long backup can't re-trigger.
        let next = schedule::next_run_iso(&sched.cron).ok();
        if let Err(e) = super::meta::set_schedule_runs(service.pool(), &sched.id, &now, next.as_deref()).await {
            tracing::error!(schedule = %sched.id, error = %e, "failed to update schedule next run time");
        }

        let opts = CreateBackupOptions {
            scope,
            include_files: sched.include_files != 0,
            encrypt: sched.encrypt != 0,
            schedule_id: Some(sched.id.clone()),
            created_by: sched.created_by.clone(),
        };
        if let Err(e) = service.create_backup(opts).await {
            tracing::error!(schedule = %sched.id, error = %e, "scheduled backup failed");
        } else {
            tracing::info!(schedule = %sched.id, "scheduled backup completed");
        }
    }
    Ok(())
}
