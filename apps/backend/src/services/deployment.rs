use std::{sync::Arc, time::Instant};

use crate::{
    database::pool::DbPool,
    models::deployment::{CreateDeploymentTrigger, DeploymentJob, DeploymentTrigger},
    services::webhook::WebhookService,
};
use uuid::Uuid;

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
        match &self.pool{DbPool::Sqlite(p)=>sqlx::query("UPDATE deployment_jobs SET status='failed',error_category='interrupted',finished_at=datetime('now') WHERE status IN ('queued','running')").execute(p).await.map(|v|v.rows_affected()).map_err(|e|e.to_string()),DbPool::Postgres(p)=>sqlx::query("UPDATE deployment_jobs SET status='failed',error_category='interrupted',finished_at=NOW() WHERE status IN ('queued','running')").execute(p).await.map(|v|v.rows_affected()).map_err(|e|e.to_string())}
    }

    pub async fn list(&self, site_id: &str) -> Result<Vec<DeploymentTrigger>, String> {
        match &self.pool {
            DbPool::Sqlite(p) => sqlx::query_as("SELECT id,site_id,label,provider,enabled,is_primary,cooldown_seconds,daily_quota,created_by,created_at,updated_at FROM deployment_triggers WHERE site_id=? ORDER BY is_primary DESC,label").bind(site_id).fetch_all(p).await,
            DbPool::Postgres(p) => sqlx::query_as("SELECT id,site_id,label,provider,enabled,is_primary,cooldown_seconds,daily_quota,created_by,created_at::text,updated_at::text FROM deployment_triggers WHERE site_id=$1 ORDER BY is_primary DESC,label").bind(site_id).fetch_all(p).await,
        }.map_err(|e| e.to_string())
    }

    pub async fn create(
        &self,
        site_id: &str,
        user_id: &str,
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
        if value.is_primary {
            self.clear_primary(site_id).await?;
        }
        let id = Uuid::now_v7().to_string();
        match &self.pool {
            DbPool::Sqlite(p) => sqlx::query("INSERT INTO deployment_triggers(id,site_id,label,provider,url_encrypted,headers_encrypted,enabled,is_primary,cooldown_seconds,daily_quota,created_by) VALUES(?,?,?,?,?,?,?,?,?,?,?)").bind(&id).bind(site_id).bind(value.label.trim()).bind(&value.provider).bind(&url).bind(&headers).bind(value.enabled).bind(value.is_primary).bind(value.cooldown_seconds).bind(value.daily_quota).bind(user_id).execute(p).await.map(|_|()).map_err(|e|e.to_string()),
            DbPool::Postgres(p) => sqlx::query("INSERT INTO deployment_triggers(id,site_id,label,provider,url_encrypted,headers_encrypted,enabled,is_primary,cooldown_seconds,daily_quota,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)").bind(&id).bind(site_id).bind(value.label.trim()).bind(&value.provider).bind(&url).bind(&headers).bind(value.enabled).bind(value.is_primary).bind(value.cooldown_seconds).bind(value.daily_quota).bind(user_id).execute(p).await.map(|_|()).map_err(|e|e.to_string()),
        }?;
        self.list(site_id)
            .await?
            .into_iter()
            .find(|v| v.id == id)
            .ok_or_else(|| "trigger_not_found".into())
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
        if !self.list(site_id).await?.iter().any(|trigger| trigger.id == id) {
            return Err("trigger_not_found".into());
        }
        let (url, headers) = self
            .webhooks
            .protect_deployment_config(&value.url, &value.headers)
            .map_err(|error| error.to_string())?;
        if value.is_primary {
            self.clear_primary(site_id).await?;
        }
        match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query("UPDATE deployment_triggers SET label=?,provider=?,url_encrypted=?,headers_encrypted=?,enabled=?,is_primary=?,cooldown_seconds=?,daily_quota=?,updated_at=datetime('now') WHERE id=? AND site_id=?")
                .bind(value.label.trim()).bind(value.provider).bind(url).bind(headers).bind(value.enabled).bind(value.is_primary).bind(value.cooldown_seconds).bind(value.daily_quota).bind(id).bind(site_id).execute(pool).await.map(|_| ()),
            DbPool::Postgres(pool) => sqlx::query("UPDATE deployment_triggers SET label=$1,provider=$2,url_encrypted=$3,headers_encrypted=$4,enabled=$5,is_primary=$6,cooldown_seconds=$7,daily_quota=$8,updated_at=NOW() WHERE id=$9 AND site_id=$10")
                .bind(value.label.trim()).bind(value.provider).bind(url).bind(headers).bind(value.enabled).bind(value.is_primary).bind(value.cooldown_seconds).bind(value.daily_quota).bind(id).bind(site_id).execute(pool).await.map(|_| ()),
        }
        .map_err(|error| error.to_string())?;
        self.list(site_id)
            .await?
            .into_iter()
            .find(|trigger| trigger.id == id)
            .ok_or_else(|| "trigger_not_found".into())
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

    async fn clear_primary(&self, site_id: &str) -> Result<(), String> {
        match &self.pool {
            DbPool::Sqlite(p) => sqlx::query("UPDATE deployment_triggers SET is_primary=0 WHERE site_id=?")
                .bind(site_id)
                .execute(p)
                .await
                .map(|_| ()),
            DbPool::Postgres(p) => sqlx::query("UPDATE deployment_triggers SET is_primary=FALSE WHERE site_id=$1")
                .bind(site_id)
                .execute(p)
                .await
                .map(|_| ()),
        }
        .map_err(|e| e.to_string())
    }

    pub async fn history(&self, trigger_id: &str) -> Result<Vec<DeploymentJob>, String> {
        match &self.pool {
            DbPool::Sqlite(p) => sqlx::query_as("SELECT id,trigger_id,site_id,status,status_code,error_category,response_body,retry_after_seconds,duration_ms,triggered_by,created_at,started_at,finished_at FROM deployment_jobs WHERE trigger_id=? ORDER BY created_at DESC LIMIT 100").bind(trigger_id).fetch_all(p).await,
            DbPool::Postgres(p) => sqlx::query_as("SELECT id,trigger_id,site_id,status,status_code,error_category,response_body,retry_after_seconds,duration_ms,triggered_by,created_at::text,started_at::text,finished_at::text FROM deployment_jobs WHERE trigger_id=$1 ORDER BY created_at DESC LIMIT 100").bind(trigger_id).fetch_all(p).await,
        }.map_err(|e|e.to_string())
    }

    pub async fn trigger(
        self: &Arc<Self>,
        site_id: &str,
        trigger_id: &str,
        user_id: &str,
    ) -> Result<DeploymentJob, String> {
        let trigger = self
            .list(site_id)
            .await?
            .into_iter()
            .find(|v| v.id == trigger_id)
            .ok_or("trigger_not_found")?;
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
        if trigger.daily_quota > 0
            && history
                .iter()
                .filter(|j| age_seconds(j).is_some_and(|age| age < 86_400))
                .count() as i64
                >= trigger.daily_quota
        {
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

    async fn insert_job(&self, site_id: &str, trigger_id: &str, user_id: &str) -> Result<DeploymentJob, String> {
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
        self.history(trigger_id)
            .await?
            .into_iter()
            .find(|j| j.id == id)
            .ok_or("job_not_found".into())
    }

    async fn run_job(&self, trigger: DeploymentTrigger, job_id: String) {
        let started = Instant::now();
        let result = self.load_secret(&trigger.id).await.and_then(|(u, h)| {
            self.webhooks
                .reveal_deployment_config(&u, &h)
                .map_err(|e| e.to_string())
        });
        let result = match result {
            Ok((url, headers)) => match self.webhooks.build_protected_client(&url).await {
                Ok(client) => {
                    let mut request = client.post(&url);
                    for (key, value) in headers {
                        request = request.header(key, value);
                    }
                    request.send().await.map_err(|error| error.to_string()).map(|response| {
                        (
                            response.status().as_u16() as i32,
                            response
                                .headers()
                                .get("retry-after")
                                .and_then(|value| value.to_str().ok())
                                .and_then(|value| value.parse().ok()),
                        )
                    })
                }
                Err(error) => Err(error.to_string()),
            },
            Err(e) => Err(e),
        };
        let (status, code, category, retry) = match result {
            Ok((c, r)) if (200..300).contains(&c) => ("succeeded", Some(c), None, r),
            Ok((429, r)) => ("failed", Some(429), Some("provider_rate_limit"), r),
            Ok((c, r)) if c >= 500 => ("failed", Some(c), Some("provider_server"), r),
            Ok((c, r)) => ("failed", Some(c), Some("configuration"), r),
            Err(_) => ("failed", None, Some("network"), None),
        };
        let _ = self
            .finish(
                &job_id,
                status,
                code,
                category,
                retry,
                started.elapsed().as_millis() as i64,
            )
            .await;
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
    async fn finish(
        &self,
        id: &str,
        status: &str,
        code: Option<i32>,
        category: Option<&str>,
        retry: Option<i64>,
        duration: i64,
    ) -> Result<(), String> {
        match &self.pool{DbPool::Sqlite(p)=>sqlx::query("UPDATE deployment_jobs SET status=?,status_code=?,error_category=?,retry_after_seconds=?,duration_ms=?,started_at=COALESCE(started_at,created_at),finished_at=datetime('now') WHERE id=?").bind(status).bind(code).bind(category).bind(retry).bind(duration).bind(id).execute(p).await.map(|_|()).map_err(|e|e.to_string()),DbPool::Postgres(p)=>sqlx::query("UPDATE deployment_jobs SET status=$1,status_code=$2,error_category=$3,retry_after_seconds=$4,duration_ms=$5,started_at=COALESCE(started_at,created_at),finished_at=NOW() WHERE id=$6").bind(status).bind(code).bind(category).bind(retry).bind(duration).bind(id).execute(p).await.map(|_|()).map_err(|e|e.to_string())}
    }
}
