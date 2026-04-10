use async_trait::async_trait;
use serde_json::Value;

use crate::models::{
    api_key::ApiKey,
    collection::Collection,
    entry::Entry,
    file::{File, FileReference},
    site::{Site, SiteMember, SiteWithRole},
    user::User,
};
use crate::repository::error::RepositoryError;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, RepositoryError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError>;
    async fn find_id_by_username(&self, username: &str) -> Result<Option<String>, RepositoryError>;
    async fn create(&self, id: &str, username: &str, email: &str, password_hash: &str) -> Result<(), RepositoryError>;
    async fn exists(&self, username: &str) -> Result<bool, RepositoryError>;
    async fn get_role(&self, user_id: &str, site_id: &str) -> Result<Option<String>, RepositoryError>;
}

#[async_trait]
pub trait SiteRepository: Send + Sync {
    async fn list_for_user(&self, user_id: &str) -> Result<Vec<SiteWithRole>, RepositoryError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Site>, RepositoryError>;
    async fn create(
        &self,
        id: &str,
        name: &str,
        storage_provider: &str,
        created_by: &str,
    ) -> Result<Site, RepositoryError>;
    async fn update(&self, id: &str, name: &str, storage_provider: &str) -> Result<Site, RepositoryError>;
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
    async fn update_singleton_data(&self, id: &str, data: &str) -> Result<Collection, RepositoryError>;
    async fn delete(&self, site_id: &str, slug: &str) -> Result<u64, RepositoryError>;
    async fn get_content_for_migration(&self, collection_id: &str) -> Result<Vec<Entry>, RepositoryError>;
    async fn migrate_content_field_renames(
        &self,
        content_items: &[Entry],
        rename_map: &std::collections::HashMap<String, String>,
    ) -> Result<(), RepositoryError>;
    async fn migrate_singleton_field_renames(
        &self,
        collection: &Collection,
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
    async fn create(
        &self,
        id: &str,
        site_id: &str,
        collection_id: &str,
        data: &str,
        slug: &str,
    ) -> Result<Entry, RepositoryError>;
    async fn update(&self, id: &str, data: &str, slug: &str, status: &str) -> Result<Entry, RepositoryError>;
    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError>;
    async fn publish(&self, id: &str, site_id: &str) -> Result<Entry, RepositoryError>;
    async fn unpublish(&self, id: &str, site_id: &str) -> Result<Entry, RepositoryError>;
    async fn sync_file_references(&self, entry_id: &str, site_id: &str, data: &Value) -> Result<(), RepositoryError>;
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

#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    async fn list(&self, site_id: &str) -> Result<Vec<ApiKey>, RepositoryError>;
    async fn create(
        &self,
        id: &str,
        site_id: &str,
        name: &str,
        key_hash: &str,
        key_prefix: &str,
        key_hmac: &str,
        permissions: &str,
    ) -> Result<(), RepositoryError>;
    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError>;
    async fn find_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, String, String, Option<String>, Option<String>, String)>, RepositoryError>;
    async fn update_last_used(&self, id: &str) -> Result<(), RepositoryError>;
}
