use async_trait::async_trait;
use serde_json::Value;

use crate::models::{
    access_token::AccessToken,
    collection::Collection,
    entry::{Entry, EntryRevision},
    file::{File, FileReference},
    session::Session,
    site::{Site, SiteMember, SiteWithRole},
    user::User,
    webhook::{SiteWebhook, WebhookDelivery},
};
use crate::repository::error::RepositoryError;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, RepositoryError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError>;
    async fn list(&self) -> Result<Vec<User>, RepositoryError>;
    async fn find_id_by_username(&self, username: &str) -> Result<Option<String>, RepositoryError>;
    async fn create(&self, id: &str, username: &str, email: &str, password_hash: &str) -> Result<(), RepositoryError>;
    async fn exists(&self, username: &str) -> Result<bool, RepositoryError>;
    async fn get_role(&self, user_id: &str, site_id: &str) -> Result<Option<String>, RepositoryError>;
    async fn count(&self) -> Result<i64, RepositoryError>;
    async fn count_instance_owners(&self) -> Result<i64, RepositoryError>;
    async fn set_instance_role(&self, user_id: &str, role: Option<&str>) -> Result<u64, RepositoryError>;
    async fn update_password(
        &self,
        user_id: &str,
        password_hash: &str,
        must_change: bool,
    ) -> Result<u64, RepositoryError>;
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(
        &self,
        id: &str,
        user_id: &str,
        token_hash: &str,
        csrf_token_hash: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError>;
    async fn find_active_by_hash(&self, token_hash: &str) -> Result<Option<Session>, RepositoryError>;
    async fn touch(&self, id: &str) -> Result<(), RepositoryError>;
    async fn revoke(&self, id: &str, user_id: &str) -> Result<u64, RepositoryError>;
    async fn revoke_all(&self, user_id: &str) -> Result<u64, RepositoryError>;
    async fn list(&self, user_id: &str) -> Result<Vec<Session>, RepositoryError>;
}

#[async_trait]
pub trait SiteRepository: Send + Sync {
    async fn list_all(&self) -> Result<Vec<Site>, RepositoryError>;
    async fn list_for_user(&self, user_id: &str) -> Result<Vec<SiteWithRole>, RepositoryError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Site>, RepositoryError>;
    async fn create(
        &self,
        id: &str,
        name: &str,
        storage_provider: &str,
        created_by: &str,
    ) -> Result<Site, RepositoryError>;
    async fn update(&self, id: &str, name: &str) -> Result<Site, RepositoryError>;
    async fn delete(&self, id: &str) -> Result<u64, RepositoryError>;
    async fn list_members(&self, site_id: &str) -> Result<Vec<SiteMember>, RepositoryError>;
    async fn add_member(
        &self,
        id: &str,
        site_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<SiteMember, RepositoryError>;
    async fn update_member_role(
        &self,
        site_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<Option<SiteMember>, RepositoryError>;
    async fn remove_member(&self, site_id: &str, user_id: &str) -> Result<u64, RepositoryError>;
    async fn transfer_ownership(
        &self,
        site_id: &str,
        current_owner_id: &str,
        new_owner_id: &str,
    ) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait CollectionRepository: Send + Sync {
    async fn list(&self, site_id: &str) -> Result<Vec<Collection>, RepositoryError>;
    async fn list_singletons_only(&self, site_id: &str) -> Result<Vec<Collection>, RepositoryError>;
    async fn get_by_slug(&self, site_id: &str, slug: &str) -> Result<Option<Collection>, RepositoryError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Collection>, RepositoryError>;
    async fn create(
        &self,
        id: &str,
        site_id: &str,
        name: &str,
        slug: &str,
        definition: &str,
        is_singleton: bool,
    ) -> Result<Collection, RepositoryError>;
    async fn update(&self, id: &str, name: &str, slug: &str, definition: &str) -> Result<Collection, RepositoryError>;
    async fn delete(&self, site_id: &str, slug: &str) -> Result<u64, RepositoryError>;
    async fn get_content_for_migration(&self, collection_id: &str) -> Result<Vec<Entry>, RepositoryError>;
    async fn migrate_content_field_renames(
        &self,
        content_items: &[Entry],
        rename_map: &std::collections::HashMap<String, String>,
    ) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait EntryRepository: Send + Sync {
    async fn get_by_id(&self, id: &str, site_id: &str, published_only: bool) -> Result<Option<Entry>, RepositoryError>;
    async fn get_by_id_any_site(&self, id: &str) -> Result<Option<Entry>, RepositoryError>;
    async fn list(&self, params: ListEntriesParams<'_>) -> Result<EntriesListResult, RepositoryError>;
    async fn get_by_collection_id(
        &self,
        collection_id: &str,
        status: Option<&str>,
        published_only: bool,
    ) -> Result<Vec<Entry>, RepositoryError>;
    /// Batched variant of [`get_by_collection_id`] for many collections in one
    /// query (used by the GraphQL DataLoader to avoid N+1). Each returned entry
    /// carries its `collection_id` so callers can group the flat result.
    async fn get_by_collection_ids(
        &self,
        collection_ids: &[String],
        status: Option<&str>,
        published_only: bool,
    ) -> Result<Vec<Entry>, RepositoryError>;
    async fn create(
        &self,
        id: &str,
        site_id: &str,
        collection_id: &str,
        data: &str,
        slug: &str,
        created_by: Option<&str>,
    ) -> Result<Entry, RepositoryError>;
    async fn get_singleton_entry(&self, site_id: &str, slug: &str) -> Result<Option<Entry>, RepositoryError>;
    async fn upsert_singleton_entry(
        &self,
        site_id: &str,
        collection_id: &str,
        slug: &str,
        data: &str,
        created_by: Option<&str>,
        change_summary: Option<&str>,
    ) -> Result<Entry, RepositoryError>;
    async fn update(
        &self,
        id: &str,
        site_id: &str,
        data: &str,
        slug: &str,
        status: &str,
        created_by: Option<&str>,
        change_summary: Option<&str>,
    ) -> Result<Entry, RepositoryError>;
    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError>;
    async fn publish(&self, id: &str, site_id: &str) -> Result<Entry, RepositoryError>;
    async fn unpublish(&self, id: &str, site_id: &str) -> Result<Entry, RepositoryError>;
    async fn sync_file_references(&self, entry_id: &str, site_id: &str, data: &Value) -> Result<(), RepositoryError>;

    // Revision methods
    async fn list_revisions(
        &self,
        entry_id: &str,
        page: i64,
        per_page: i64,
    ) -> Result<RevisionsListResult, RepositoryError>;
    async fn get_revision(
        &self,
        entry_id: &str,
        revision_number: i64,
    ) -> Result<Option<EntryRevision>, RepositoryError>;
    async fn restore_revision(
        &self,
        entry_id: &str,
        revision_number: i64,
        created_by: Option<&str>,
    ) -> Result<Entry, RepositoryError>;
    async fn migrate_singleton_field_renames(
        &self,
        site_id: &str,
        collection_id: &str,
        rename_map: &std::collections::HashMap<String, String>,
    ) -> Result<(), RepositoryError>;
}

#[derive(Clone)]
pub struct ListEntriesParams<'a> {
    pub site_id: &'a str,
    pub collection_slug: Option<&'a str>,
    pub collection_id: Option<&'a str>,
    pub status: Option<&'a str>,
    pub search: Option<&'a str>,
    pub published_only: bool,
    pub page: i64,
    pub per_page: i64,
}

pub struct EntriesListResult {
    pub items: Vec<Entry>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

pub struct RevisionsListResult {
    pub items: Vec<EntryRevision>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Clone)]
pub struct ListFilesParams<'a> {
    pub site_id: &'a str,
    pub trashed: bool,
    pub search: Option<&'a str>,
    pub file_type: Option<&'a str>,
    pub page: i64,
    pub per_page: i64,
}

pub struct FileListResult {
    pub items: Vec<File>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[async_trait]
pub trait FileRepository: Send + Sync {
    async fn get_by_id(&self, id: &str, site_id: &str) -> Result<Option<File>, RepositoryError>;
    async fn get_by_id_any(&self, id: &str) -> Result<Option<File>, RepositoryError>;
    async fn list(&self, params: ListFilesParams<'_>) -> Result<FileListResult, RepositoryError>;
    async fn create(
        &self,
        id: &str,
        site_id: &str,
        filename: &str,
        original_name: &str,
        mime_type: &str,
        size: i64,
        storage_provider: &str,
        storage_key: &str,
        thumbnail_key: Option<&str>,
        width: Option<i32>,
        height: Option<i32>,
        created_by: Option<&str>,
    ) -> Result<File, RepositoryError>;
    async fn soft_delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError>;
    async fn restore(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError>;
    async fn batch_soft_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, RepositoryError>;
    async fn batch_restore(&self, site_id: &str, ids: &[String]) -> Result<u64, RepositoryError>;
    async fn get_by_ids(&self, site_id: &str, ids: &[String]) -> Result<Vec<File>, RepositoryError>;
    async fn get_deleted_by_ids(&self, site_id: &str, ids: &[String]) -> Result<Vec<File>, RepositoryError>;
    async fn batch_permanent_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, RepositoryError>;
    async fn get_references(&self, file_id: &str) -> Result<Vec<FileReference>, RepositoryError>;
    async fn get_references_for_site(
        &self,
        file_id: &str,
        site_id: &str,
    ) -> Result<Vec<FileReference>, RepositoryError>;
    async fn get_storage_provider(&self, site_id: &str) -> Result<String, RepositoryError>;
}

pub type AccessTokenLookupRow = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

#[async_trait]
pub trait AccessTokenRepository: Send + Sync {
    async fn list(&self, site_id: &str) -> Result<Vec<AccessToken>, RepositoryError>;
    async fn create(
        &self,
        id: &str,
        site_id: &str,
        name: &str,
        token_hash: &str,
        token_prefix: &str,
        token_hmac: &str,
        permission: &str,
        created_by_user_id: Option<&str>,
    ) -> Result<(), RepositoryError>;
    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError>;
    async fn find_by_prefix(&self, prefix: &str) -> Result<Vec<AccessTokenLookupRow>, RepositoryError>;
    async fn update_last_used(&self, id: &str) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait WebhookRepository: Send + Sync {
    async fn list_for_site(&self, site_id: &str) -> Result<Vec<SiteWebhook>, RepositoryError>;
    async fn get_by_id(&self, id: &str, site_id: &str) -> Result<Option<SiteWebhook>, RepositoryError>;
    async fn create(
        &self,
        id: &str,
        site_id: &str,
        label: &str,
        url: &str,
        headers_encrypted: &str,
        created_by: Option<&str>,
    ) -> Result<SiteWebhook, RepositoryError>;
    async fn update(
        &self,
        id: &str,
        label: Option<&str>,
        url: Option<&str>,
        headers_encrypted: Option<&str>,
    ) -> Result<SiteWebhook, RepositoryError>;
    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError>;
    async fn create_delivery(
        &self,
        id: &str,
        webhook_id: &str,
        status: &str,
        status_code: Option<i32>,
        response_body: Option<&str>,
        duration_ms: Option<i64>,
        triggered_by: Option<&str>,
    ) -> Result<WebhookDelivery, RepositoryError>;
    async fn list_deliveries(
        &self,
        webhook_id: &str,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<WebhookDelivery>, i64), RepositoryError>;
}
