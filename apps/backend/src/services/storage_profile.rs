use crate::{
    database::pool::DbPool,
    models::storage_profile::{CreateStorageProfile, StorageProfile, UpdateStorageProfile},
    storage::{S3Storage, StorageProvider, StorageRegistry},
};
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use rand::Rng;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct StorageProfileService {
    pool: DbPool,
    key: [u8; 32],
}
impl StorageProfileService {
    pub fn new(pool: DbPool, secret: &str) -> Self {
        Self {
            pool,
            key: Sha256::digest(secret.as_bytes()).into(),
        }
    }
    pub async fn list(&self) -> Result<Vec<StorageProfile>, String> {
        match &self.pool{
        DbPool::Sqlite(p)=>sqlx::query_as("SELECT id,name,kind,endpoint,region,bucket,public_url,enabled,immutable,created_by,created_at,updated_at FROM storage_profiles ORDER BY immutable DESC,name").fetch_all(p).await,
        DbPool::Postgres(p)=>sqlx::query_as("SELECT id,name,kind,endpoint,region,bucket,public_url,enabled,immutable,created_by,created_at::text,updated_at::text FROM storage_profiles ORDER BY immutable DESC,name").fetch_all(p).await,
    }.map_err(|e|e.to_string())
    }
    pub async fn create(&self, v: CreateStorageProfile, by: &str) -> Result<StorageProfile, String> {
        if v.name.trim().is_empty() || v.bucket.trim().is_empty() {
            return Err("name_and_bucket_required".into());
        }
        url::Url::parse(&v.endpoint).map_err(|_| "invalid_endpoint")?;
        let id = Uuid::now_v7().to_string();
        let credentials = self.encrypt(
            &serde_json::json!({"access_key_id":v.access_key_id,"secret_access_key":v.secret_access_key}).to_string(),
        )?;
        match &self.pool{
            DbPool::Sqlite(p)=>sqlx::query("INSERT INTO storage_profiles(id,name,kind,endpoint,region,bucket,public_url,credentials_encrypted,created_by)VALUES(?,?,'s3',?,?,?,?,?,?)").bind(&id).bind(v.name.trim()).bind(v.endpoint).bind(v.region).bind(v.bucket).bind(v.public_url).bind(credentials).bind(by).execute(p).await.map(|_|()).map_err(|e|e.to_string()),
            DbPool::Postgres(p)=>sqlx::query("INSERT INTO storage_profiles(id,name,kind,endpoint,region,bucket,public_url,credentials_encrypted,created_by)VALUES($1,$2,'s3',$3,$4,$5,$6,$7,$8)").bind(&id).bind(v.name.trim()).bind(v.endpoint).bind(v.region).bind(v.bucket).bind(v.public_url).bind(credentials).bind(by).execute(p).await.map(|_|()).map_err(|e|e.to_string()),
        }?;
        self.list()
            .await?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or("profile_not_found".into())
    }
    pub async fn update(&self, id: &str, v: UpdateStorageProfile) -> Result<StorageProfile, String> {
        let current = self
            .list()
            .await?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or("profile_not_found")?;
        if current.immutable {
            return Err("immutable_profile".into());
        }
        if !v.enabled && self.reference_count(id).await? > 0 {
            return Err("profile_in_use".into());
        }
        if v.name.trim().is_empty() || v.bucket.trim().is_empty() {
            return Err("name_and_bucket_required".into());
        }
        url::Url::parse(&v.endpoint).map_err(|_| "invalid_endpoint")?;
        let credentials = match (v.access_key_id, v.secret_access_key) {
            (Some(access), Some(secret)) if !access.is_empty() && !secret.is_empty() => {
                Some(self.encrypt(&serde_json::json!({"access_key_id":access,"secret_access_key":secret}).to_string())?)
            }
            (None, None) => None,
            _ => return Err("both_credentials_required".into()),
        };
        match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query("UPDATE storage_profiles SET name=?,endpoint=?,region=?,bucket=?,public_url=?,enabled=?,credentials_encrypted=COALESCE(?,credentials_encrypted),updated_at=datetime('now') WHERE id=?")
                .bind(v.name.trim()).bind(v.endpoint).bind(v.region).bind(v.bucket).bind(v.public_url).bind(v.enabled).bind(credentials).bind(id).execute(pool).await.map(|_| ()),
            DbPool::Postgres(pool) => sqlx::query("UPDATE storage_profiles SET name=$1,endpoint=$2,region=$3,bucket=$4,public_url=$5,enabled=$6,credentials_encrypted=COALESCE($7,credentials_encrypted),updated_at=NOW() WHERE id=$8")
                .bind(v.name.trim()).bind(v.endpoint).bind(v.region).bind(v.bucket).bind(v.public_url).bind(v.enabled).bind(credentials).bind(id).execute(pool).await.map(|_| ()),
        }.map_err(|error| error.to_string())?;
        self.list()
            .await?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| "profile_not_found".into())
    }

    pub async fn probe(&self, id: &str) -> Result<(), String> {
        let profile = self
            .list()
            .await?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or("profile_not_found")?;
        if profile.kind == "filesystem" {
            return Ok(());
        }
        let provider = self.build_provider(&profile).await?;
        let key = format!("vcms-probes/{}.txt", Uuid::now_v7());
        provider
            .put(&key, bytes::Bytes::from_static(b"vcms storage probe"), "text/plain")
            .await
            .map_err(|error| error.to_string())?;
        provider.delete(&key).await.map_err(|error| error.to_string())
    }
    async fn reference_count(&self, id: &str) -> Result<i64, String> {
        let count: i64 = match &self.pool {
            DbPool::Sqlite(p) => {
                sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM sites WHERE storage_profile_id=?) + (SELECT COUNT(*) FROM backup_schedules WHERE storage_profile_id=?) + (SELECT COUNT(*) FROM backups WHERE storage_profile_id=? AND status IN ('pending','running'))")
                    .bind(id)
                    .bind(id)
                    .bind(id)
                    .fetch_one(p)
                    .await
            }
            DbPool::Postgres(p) => {
                sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM sites WHERE storage_profile_id=$1) + (SELECT COUNT(*) FROM backup_schedules WHERE storage_profile_id=$1) + (SELECT COUNT(*) FROM backups WHERE storage_profile_id=$1 AND status IN ('pending','running'))")
                    .bind(id)
                    .fetch_one(p)
                    .await
            }
        }.map_err(|error| error.to_string())?;
        Ok(count)
    }
    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let profile = self
            .list()
            .await?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or("profile_not_found")?;
        if profile.immutable {
            return Err("immutable_profile".into());
        }
        if self.reference_count(id).await? > 0 {
            return Err("profile_in_use".into());
        }
        match &self.pool {
            DbPool::Sqlite(p) => sqlx::query("DELETE FROM storage_profiles WHERE id=?")
                .bind(id)
                .execute(p)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
            DbPool::Postgres(p) => sqlx::query("DELETE FROM storage_profiles WHERE id=$1")
                .bind(id)
                .execute(p)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
        }?;
        Ok(())
    }
    pub async fn assign_site(&self, site: &str, profile: &str) -> Result<(), String> {
        let p = self
            .list()
            .await?
            .into_iter()
            .find(|v| v.id == profile && v.enabled)
            .ok_or("storage_profile_not_found")?;
        match &self.pool {
            DbPool::Sqlite(db) => sqlx::query("UPDATE sites SET storage_profile_id=?,storage_provider=? WHERE id=?")
                .bind(profile)
                .bind(&p.kind)
                .bind(site)
                .execute(db)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
            DbPool::Postgres(db) => {
                sqlx::query("UPDATE sites SET storage_profile_id=$1,storage_provider=$2 WHERE id=$3")
                    .bind(profile)
                    .bind(&p.kind)
                    .bind(site)
                    .execute(db)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }?;
        Ok(())
    }
    pub async fn register_all(&self, registry: &StorageRegistry) -> Result<(), String> {
        if let Some(local) = registry.get("filesystem") {
            registry.register("local-filesystem", local);
        }
        for profile in self
            .list()
            .await?
            .into_iter()
            .filter(|profile| profile.kind == "s3" && profile.enabled)
        {
            registry.register(&profile.id, self.build_provider(&profile).await?);
        }
        Ok(())
    }
    async fn build_provider(&self, profile: &StorageProfile) -> Result<Arc<dyn StorageProvider>, String> {
        let credentials = self.credentials(&profile.id).await?;
        let access = credentials
            .get("access_key_id")
            .and_then(|value| value.as_str())
            .ok_or("invalid_storage_credentials")?;
        let secret = credentials
            .get("secret_access_key")
            .and_then(|value| value.as_str())
            .ok_or("invalid_storage_credentials")?;
        Ok(Arc::new(
            S3Storage::new(
                access,
                secret,
                profile.bucket.as_deref().ok_or("missing_bucket")?,
                profile.region.as_deref().unwrap_or("auto"),
                profile.endpoint.as_deref(),
                profile.public_url.as_deref(),
            )
            .map_err(|error| error.to_string())?,
        ))
    }
    async fn credentials(&self, id: &str) -> Result<serde_json::Value, String> {
        let encrypted: Option<String> = match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query_scalar("SELECT credentials_encrypted FROM storage_profiles WHERE id=?")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
            }
            DbPool::Postgres(pool) => {
                sqlx::query_scalar("SELECT credentials_encrypted FROM storage_profiles WHERE id=$1")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
            }
        }
        .map_err(|error| error.to_string())?
        .flatten();
        serde_json::from_str(&self.decrypt(encrypted.as_deref().ok_or("missing_storage_credentials")?)?)
            .map_err(|error| error.to_string())
    }
    fn decrypt(&self, value: &str) -> Result<String, String> {
        let data = B64
            .decode(value.strip_prefix("v1:").ok_or("invalid_envelope")?)
            .map_err(|error| error.to_string())?;
        if data.len() < 13 {
            return Err("invalid_envelope".into());
        }
        let nonce: [u8; 12] = data[..12].try_into().map_err(|_| "invalid_nonce")?;
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|error| error.to_string())?;
        let plain = cipher
            .decrypt(&Nonce::from(nonce), &data[12..])
            .map_err(|_| "decryption_failed")?;
        String::from_utf8(plain).map_err(|error| error.to_string())
    }
    fn encrypt(&self, text: &str) -> Result<String, String> {
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|e| e.to_string())?;
        let mut nonce = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce);
        let mut out = nonce.to_vec();
        let nonce_value = Nonce::from(nonce);
        out.extend(
            cipher
                .encrypt(&nonce_value, text.as_bytes())
                .map_err(|_| "encryption_failed")?,
        );
        Ok(format!("v1:{}", B64.encode(out)))
    }
}
