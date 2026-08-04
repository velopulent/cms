use std::{sync::Arc, time::Instant};

use crate::{
    database::pool::DbPool,
    models::deployment::{CreateDeploymentTrigger, DeploymentJob, DeploymentTrigger},
    services::webhook::WebhookService,
};
use futures_util::StreamExt;
use uuid::Uuid;

const MAX_DEPLOYMENT_RESPONSE_BYTES: usize = 64 * 1024;

struct DeploymentOutcome {
    status: &'static str,
    code: Option<i32>,
    category: Option<&'static str>,
    retry: Option<i64>,
    response_body: Option<String>,
}

impl DeploymentOutcome {
    fn failure(category: &'static str, error: String) -> Self {
        Self {
            status: "failed",
            code: None,
            category: Some(category),
            retry: None,
            response_body: Some(error),
        }
    }
}

async fn read_bounded_response(response: reqwest::Response) -> Result<String, String> {
    let mut stream = response.bytes_stream();
    let mut body = Vec::with_capacity(MAX_DEPLOYMENT_RESPONSE_BYTES.min(4096));
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        let remaining = MAX_DEPLOYMENT_RESPONSE_BYTES.saturating_sub(body.len());
        if remaining == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

fn map_job_insert_error(error: sqlx::Error) -> String {
    if error
        .as_database_error()
        .is_some_and(|database_error| database_error.is_unique_violation())
    {
        "deployment_in_progress".into()
    } else {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::database::{init_db, pool::DbPool};

    #[tokio::test]
    async fn active_job_is_unique_per_trigger() {
        let pool = init_db("sqlite::memory:").await.expect("database initializes");
        let DbPool::Sqlite(database) = pool else { unreachable!() };
        sqlx::query("INSERT INTO users(id,name,email,password_hash) VALUES('u','User','u@example.com','hash')")
            .execute(&database)
            .await
            .expect("user inserts");
        sqlx::query("INSERT INTO sites(id,name,storage_provider,created_by,storage_profile_id) VALUES('s','Site','filesystem','u','local-filesystem')")
            .execute(&database).await.expect("site inserts");
        sqlx::query("INSERT INTO deployment_triggers(id,site_id,label,provider,url_encrypted,headers_encrypted) VALUES('t','s','Deploy','custom','u','h')")
            .execute(&database).await.expect("trigger inserts");
        sqlx::query("INSERT INTO deployment_jobs(id,trigger_id,site_id,status) VALUES('j1','t','s','queued')")
            .execute(&database)
            .await
            .expect("first active job inserts");
        let error =
            sqlx::query("INSERT INTO deployment_jobs(id,trigger_id,site_id,status) VALUES('j2','t','s','running')")
                .execute(&database)
                .await
                .expect_err("second active job must conflict");
        assert!(
            error
                .as_database_error()
                .is_some_and(|value| value.is_unique_violation())
        );
        sqlx::query("UPDATE deployment_jobs SET status='succeeded' WHERE id='j1'")
            .execute(&database)
            .await
            .expect("job completes");
        sqlx::query("INSERT INTO deployment_jobs(id,trigger_id,site_id,status) VALUES('j2','t','s','queued')")
            .execute(&database)
            .await
            .expect("new active job inserts after completion");
    }
}

#[derive(Clone)]
pub struct DeploymentService {
    pool: DbPool,
    webhooks: Arc<WebhookService>,
}

impl DeploymentService {
    pub fn new(pool: DbPool, webhooks: Arc<WebhookService>) -> Self {
        Self { pool, webhooks }
    }
    pub async fn reconcile_interrupted(&self) -> Result<u64, String> {
        match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query(
                "UPDATE deployment_jobs SET status='failed',error_category='interrupted',finished_at=datetime('now') \
                 WHERE status IN ('queued','running')",
            )
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
            .map_err(|error| error.to_string()),
            DbPool::Postgres(pool) => sqlx::query(
                "UPDATE deployment_jobs SET status='failed',error_category='interrupted',finished_at=NOW() \
                 WHERE status IN ('queued','running')",
            )
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
            .map_err(|error| error.to_string()),
        }
    }

    pub async fn list(&self, site_id: &str) -> Result<Vec<DeploymentTrigger>, String> {
        match &self.pool {
            DbPool::Sqlite(p) => sqlx::query_as("SELECT id,site_id,label,provider,enabled,is_primary,cooldown_seconds,daily_quota,created_by,created_at,updated_at FROM deployment_triggers WHERE site_id=? ORDER BY is_primary DESC,label").bind(site_id).fetch_all(p).await,
            DbPool::Postgres(p) => sqlx::query_as("SELECT id,site_id,label,provider,enabled,is_primary,cooldown_seconds,daily_quota,created_by,created_at::text,updated_at::text FROM deployment_triggers WHERE site_id=$1 ORDER BY is_primary DESC,label").bind(site_id).fetch_all(p).await,
        }.map_err(|e| e.to_string())
    }

    pub async fn get(&self, site_id: &str, id: &str) -> Result<Option<DeploymentTrigger>, String> {
        match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query_as(
                "SELECT id,site_id,label,provider,enabled,is_primary,cooldown_seconds,daily_quota,created_by,created_at,updated_at FROM deployment_triggers WHERE site_id=? AND id=?",
            )
            .bind(site_id)
            .bind(id)
            .fetch_optional(pool)
            .await,
            DbPool::Postgres(pool) => sqlx::query_as(
                "SELECT id,site_id,label,provider,enabled,is_primary,cooldown_seconds,daily_quota,created_by,created_at::text,updated_at::text FROM deployment_triggers WHERE site_id=$1 AND id=$2",
            )
            .bind(site_id)
            .bind(id)
            .fetch_optional(pool)
            .await,
        }
        .map_err(|error| error.to_string())
    }

    pub async fn create(
        &self,
        site_id: &str,
        user_id: Option<&str>,
        value: CreateDeploymentTrigger,
    ) -> Result<DeploymentTrigger, String> {
        if value.label.trim().is_empty()
            || !matches!(
                value.provider.as_str(),
                "cloudflare" | "vercel" | "netlify" | "github" | "custom"
            )
            || value.cooldown_seconds < 0
            || value.daily_quota < 0
        {
            return Err("invalid_deployment_trigger".into());
        }
        let (url, headers) = self
            .webhooks
            .protect_deployment_config(&value.url, &value.headers)
            .map_err(|e| e.to_string())?;
        let id = Uuid::now_v7().to_string();
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
                if value.is_primary {
                    sqlx::query("UPDATE deployment_triggers SET is_primary=0 WHERE site_id=?")
                        .bind(site_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|error| error.to_string())?;
                }
                sqlx::query("INSERT INTO deployment_triggers(id,site_id,label,provider,url_encrypted,headers_encrypted,enabled,is_primary,cooldown_seconds,daily_quota,created_by) VALUES(?,?,?,?,?,?,?,?,?,?,?)")
                    .bind(&id)
                    .bind(site_id)
                    .bind(value.label.trim())
                    .bind(&value.provider)
                    .bind(&url)
                    .bind(&headers)
                    .bind(value.enabled)
                    .bind(value.is_primary)
                    .bind(value.cooldown_seconds)
                    .bind(value.daily_quota)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| error.to_string())?;
                tx.commit().await.map_err(|error| error.to_string())?;
            }
            DbPool::Postgres(pool) => {
                let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
                if value.is_primary {
                    sqlx::query("UPDATE deployment_triggers SET is_primary=FALSE WHERE site_id=$1")
                        .bind(site_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|error| error.to_string())?;
                }
                sqlx::query("INSERT INTO deployment_triggers(id,site_id,label,provider,url_encrypted,headers_encrypted,enabled,is_primary,cooldown_seconds,daily_quota,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
                    .bind(&id)
                    .bind(site_id)
                    .bind(value.label.trim())
                    .bind(&value.provider)
                    .bind(&url)
                    .bind(&headers)
                    .bind(value.enabled)
                    .bind(value.is_primary)
                    .bind(value.cooldown_seconds)
                    .bind(value.daily_quota)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| error.to_string())?;
                tx.commit().await.map_err(|error| error.to_string())?;
            }
        }
        self.get(site_id, &id).await?.ok_or_else(|| "trigger_not_found".into())
    }

    pub async fn update(
        &self,
        site_id: &str,
        id: &str,
        value: CreateDeploymentTrigger,
    ) -> Result<DeploymentTrigger, String> {
        if value.label.trim().is_empty()
            || !matches!(
                value.provider.as_str(),
                "cloudflare" | "vercel" | "netlify" | "github" | "custom"
            )
            || value.cooldown_seconds < 0
            || value.daily_quota < 0
        {
            return Err("invalid_deployment_trigger".into());
        }
        if self.get(site_id, id).await?.is_none() {
            return Err("trigger_not_found".into());
        }
        let (url, headers) = self
            .webhooks
            .protect_deployment_config(&value.url, &value.headers)
            .map_err(|error| error.to_string())?;
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
                if value.is_primary {
                    sqlx::query("UPDATE deployment_triggers SET is_primary=0 WHERE site_id=?")
                        .bind(site_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|error| error.to_string())?;
                }
                let affected = sqlx::query("UPDATE deployment_triggers SET label=?,provider=?,url_encrypted=?,headers_encrypted=?,enabled=?,is_primary=?,cooldown_seconds=?,daily_quota=?,updated_at=datetime('now') WHERE id=? AND site_id=?")
                    .bind(value.label.trim())
                    .bind(&value.provider)
                    .bind(url)
                    .bind(headers)
                    .bind(value.enabled)
                    .bind(value.is_primary)
                    .bind(value.cooldown_seconds)
                    .bind(value.daily_quota)
                    .bind(id)
                    .bind(site_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| error.to_string())?
                    .rows_affected();
                if affected != 1 {
                    return Err("trigger_not_found".into());
                }
                tx.commit().await.map_err(|error| error.to_string())?;
            }
            DbPool::Postgres(pool) => {
                let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
                if value.is_primary {
                    sqlx::query("UPDATE deployment_triggers SET is_primary=FALSE WHERE site_id=$1")
                        .bind(site_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|error| error.to_string())?;
                }
                let affected = sqlx::query("UPDATE deployment_triggers SET label=$1,provider=$2,url_encrypted=$3,headers_encrypted=$4,enabled=$5,is_primary=$6,cooldown_seconds=$7,daily_quota=$8,updated_at=NOW() WHERE id=$9 AND site_id=$10")
                    .bind(value.label.trim())
                    .bind(&value.provider)
                    .bind(url)
                    .bind(headers)
                    .bind(value.enabled)
                    .bind(value.is_primary)
                    .bind(value.cooldown_seconds)
                    .bind(value.daily_quota)
                    .bind(id)
                    .bind(site_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| error.to_string())?
                    .rows_affected();
                if affected != 1 {
                    return Err("trigger_not_found".into());
                }
                tx.commit().await.map_err(|error| error.to_string())?;
            }
        }
        self.get(site_id, id).await?.ok_or_else(|| "trigger_not_found".into())
    }

    pub async fn delete(&self, site_id: &str, id: &str) -> Result<u64, String> {
        match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query("DELETE FROM deployment_triggers WHERE id=? AND site_id=?")
                .bind(id)
                .bind(site_id)
                .execute(pool)
                .await
                .map(|value| value.rows_affected())
                .map_err(|error| error.to_string()),
            DbPool::Postgres(pool) => sqlx::query("DELETE FROM deployment_triggers WHERE id=$1 AND site_id=$2")
                .bind(id)
                .bind(site_id)
                .execute(pool)
                .await
                .map(|value| value.rows_affected())
                .map_err(|error| error.to_string()),
        }
    }

    pub async fn history(&self, trigger_id: &str) -> Result<Vec<DeploymentJob>, String> {
        match &self.pool {
            DbPool::Sqlite(p) => sqlx::query_as("SELECT id,trigger_id,site_id,status,status_code,error_category,response_body,retry_after_seconds,duration_ms,triggered_by,created_at,started_at,finished_at FROM deployment_jobs WHERE trigger_id=? ORDER BY created_at DESC LIMIT 100").bind(trigger_id).fetch_all(p).await,
            DbPool::Postgres(p) => sqlx::query_as("SELECT id,trigger_id,site_id,status,status_code,error_category,response_body,retry_after_seconds,duration_ms,triggered_by,created_at::text,started_at::text,finished_at::text FROM deployment_jobs WHERE trigger_id=$1 ORDER BY created_at DESC LIMIT 100").bind(trigger_id).fetch_all(p).await,
        }.map_err(|e|e.to_string())
    }

    pub async fn get_job(&self, id: &str) -> Result<Option<DeploymentJob>, String> {
        match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query_as(
                "SELECT id,trigger_id,site_id,status,status_code,error_category,response_body,retry_after_seconds,duration_ms,triggered_by,created_at,started_at,finished_at FROM deployment_jobs WHERE id=?",
            )
            .bind(id)
            .fetch_optional(pool)
            .await,
            DbPool::Postgres(pool) => sqlx::query_as(
                "SELECT id,trigger_id,site_id,status,status_code,error_category,response_body,retry_after_seconds,duration_ms,triggered_by,created_at::text,started_at::text,finished_at::text FROM deployment_jobs WHERE id=$1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await,
        }
        .map_err(|error| error.to_string())
    }

    pub async fn trigger(
        self: &Arc<Self>,
        site_id: &str,
        trigger_id: &str,
        user_id: Option<&str>,
    ) -> Result<DeploymentJob, String> {
        let trigger = self.get(site_id, trigger_id).await?.ok_or("trigger_not_found")?;
        if !trigger.enabled {
            return Err("trigger_disabled".into());
        }
        let history = self.history(trigger_id).await?;
        if history
            .iter()
            .any(|j| matches!(j.status.as_str(), "queued" | "running"))
        {
            return Err("deployment_in_progress".into());
        }
        let now = chrono::Utc::now();
        let age_seconds = |j: &DeploymentJob| {
            crate::middleware::auth::parse_db_timestamp(&j.created_at).map(|d| (now - d).num_seconds())
        };
        if trigger.cooldown_seconds > 0
            && history
                .first()
                .and_then(age_seconds)
                .is_some_and(|age| age < trigger.cooldown_seconds)
        {
            return Err(format!(
                "deployment_cooldown:{}",
                trigger.cooldown_seconds - history.first().and_then(age_seconds).unwrap_or(0)
            ));
        }
        if trigger.daily_quota > 0 && self.count_recent_jobs(trigger_id).await? >= trigger.daily_quota {
            return Err("deployment_daily_quota".into());
        }
        let job = self.insert_job(site_id, trigger_id, user_id).await?;
        let service = self.clone();
        let job_id = job.id.clone();
        tokio::spawn(async move {
            service.run_job(trigger, job_id).await;
        });
        Ok(job)
    }

    async fn count_recent_jobs(&self, trigger_id: &str) -> Result<i64, String> {
        match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query_scalar(
                "SELECT COUNT(*) FROM deployment_jobs WHERE trigger_id=? AND created_at >= datetime('now','-1 day')",
            )
            .bind(trigger_id)
            .fetch_one(pool)
            .await,
            DbPool::Postgres(pool) => sqlx::query_scalar(
                "SELECT COUNT(*) FROM deployment_jobs WHERE trigger_id=$1 AND created_at >= NOW() - INTERVAL '24 hours'",
            )
            .bind(trigger_id)
            .fetch_one(pool)
            .await,
        }
        .map_err(|error| error.to_string())
    }

    async fn insert_job(
        &self,
        site_id: &str,
        trigger_id: &str,
        user_id: Option<&str>,
    ) -> Result<DeploymentJob, String> {
        let id = Uuid::now_v7().to_string();
        match &self.pool {
            DbPool::Sqlite(p) => sqlx::query(
                "INSERT INTO deployment_jobs(id,trigger_id,site_id,status,triggered_by) VALUES(?,?,?,'queued',?)",
            )
            .bind(&id)
            .bind(trigger_id)
            .bind(site_id)
            .bind(user_id)
            .execute(p)
            .await
            .map(|_| ())
            .map_err(map_job_insert_error),
            DbPool::Postgres(p) => sqlx::query(
                "INSERT INTO deployment_jobs(id,trigger_id,site_id,status,triggered_by) VALUES($1,$2,$3,'queued',$4)",
            )
            .bind(&id)
            .bind(trigger_id)
            .bind(site_id)
            .bind(user_id)
            .execute(p)
            .await
            .map(|_| ())
            .map_err(map_job_insert_error),
        }?;
        self.get_job(&id).await?.ok_or("job_not_found".into())
    }

    async fn run_job(&self, trigger: DeploymentTrigger, job_id: String) {
        let started = Instant::now();
        match self.mark_running(&job_id).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(job_id = %job_id, "deployment job was not queued");
                return;
            }
            Err(error) => {
                tracing::error!(job_id = %job_id, %error, "failed to mark deployment job running");
                return;
            }
        }

        let outcome = match self.load_secret(&trigger.id).await {
            Ok((encrypted_url, encrypted_headers)) => {
                match self
                    .webhooks
                    .reveal_deployment_config(&encrypted_url, &encrypted_headers)
                {
                    Ok((url, headers)) => match self.webhooks.build_protected_client(&url).await {
                        Ok(client) => {
                            let mut request = client.post(&url);
                            for (key, value) in headers {
                                request = request.header(key, value);
                            }
                            match request.send().await {
                                Ok(response) => {
                                    let code = response.status().as_u16() as i32;
                                    let retry = response
                                        .headers()
                                        .get("retry-after")
                                        .and_then(|value| value.to_str().ok())
                                        .and_then(|value| value.parse().ok());
                                    let body = match read_bounded_response(response).await {
                                        Ok(body) => Some(body),
                                        Err(error) => Some(error),
                                    };
                                    let category = if (200..300).contains(&code) {
                                        None
                                    } else if code == 429 {
                                        Some("provider_rate_limit")
                                    } else if code >= 500 {
                                        Some("provider_server")
                                    } else {
                                        Some("configuration")
                                    };
                                    DeploymentOutcome {
                                        status: if category.is_none() { "succeeded" } else { "failed" },
                                        code: Some(code),
                                        category,
                                        retry,
                                        response_body: body,
                                    }
                                }
                                Err(error) => DeploymentOutcome::failure("network", error.to_string()),
                            }
                        }
                        Err(error) => DeploymentOutcome::failure("configuration", error.to_string()),
                    },
                    Err(error) => DeploymentOutcome::failure("configuration", error.to_string()),
                }
            }
            Err(error) => DeploymentOutcome::failure("configuration", error),
        };
        if let Err(error) = self
            .finish(&job_id, &outcome, started.elapsed().as_millis() as i64)
            .await
        {
            tracing::error!(job_id = %job_id, %error, "failed to persist deployment terminal state");
        }
    }

    async fn load_secret(&self, id: &str) -> Result<(String, String), String> {
        match &self.pool {
            DbPool::Sqlite(p) => {
                sqlx::query_as("SELECT url_encrypted,headers_encrypted FROM deployment_triggers WHERE id=?")
                    .bind(id)
                    .fetch_one(p)
                    .await
            }
            DbPool::Postgres(p) => {
                sqlx::query_as("SELECT url_encrypted,headers_encrypted FROM deployment_triggers WHERE id=$1")
                    .bind(id)
                    .fetch_one(p)
                    .await
            }
        }
        .map_err(|e| e.to_string())
    }
    async fn mark_running(&self, id: &str) -> Result<bool, String> {
        let affected = match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query(
                "UPDATE deployment_jobs SET status='running',started_at=datetime('now') WHERE id=? AND status='queued'",
            )
            .bind(id)
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?
            .rows_affected(),
            DbPool::Postgres(pool) => sqlx::query(
                "UPDATE deployment_jobs SET status='running',started_at=NOW() WHERE id=$1 AND status='queued'",
            )
            .bind(id)
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?
            .rows_affected(),
        };
        Ok(affected == 1)
    }

    async fn finish(&self, id: &str, outcome: &DeploymentOutcome, duration: i64) -> Result<(), String> {
        let affected = match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query(
                "UPDATE deployment_jobs SET status=?,status_code=?,error_category=?,response_body=?,retry_after_seconds=?,duration_ms=?,finished_at=datetime('now') WHERE id=? AND status='running'",
            )
            .bind(outcome.status)
            .bind(outcome.code)
            .bind(outcome.category)
            .bind(&outcome.response_body)
            .bind(outcome.retry)
            .bind(duration)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?
            .rows_affected(),
            DbPool::Postgres(pool) => sqlx::query(
                "UPDATE deployment_jobs SET status=$1,status_code=$2,error_category=$3,response_body=$4,retry_after_seconds=$5,duration_ms=$6,finished_at=NOW() WHERE id=$7 AND status='running'",
            )
            .bind(outcome.status)
            .bind(outcome.code)
            .bind(outcome.category)
            .bind(&outcome.response_body)
            .bind(outcome.retry)
            .bind(duration)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?
            .rows_affected(),
        };
        if affected != 1 {
            return Err("deployment_job_not_running".into());
        }
        Ok(())
    }
}
