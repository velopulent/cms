use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::models::access_token::AccessToken;
use crate::models::collection::Collection;
use crate::models::entry::{Entry, EntryRevision};
use crate::models::file::{File, FileReference};
use crate::models::site::{Site, SiteMember, SiteWithRole};
use crate::models::user::User;
use crate::repository::error::RepositoryError;
use crate::repository::traits::{
    AccessTokenLookupRow, AccessTokenRepository, CollectionRepository, EntriesListResult, EntryRepository,
    FileListResult, FileRepository, ListEntriesParams, ListFilesParams, RevisionsListResult, SiteRepository,
    UserRepository,
};

pub fn now_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[derive(Clone)]
pub struct InMemoryUserRepository {
    users: Arc<Mutex<Vec<User>>>,
    by_username: Arc<Mutex<std::collections::HashMap<String, String>>>,
    by_id: Arc<Mutex<std::collections::HashMap<String, String>>>,
}

impl InMemoryUserRepository {
    pub fn new() -> Self {
        Self {
            users: Arc::new(Mutex::new(Vec::new())),
            by_username: Arc::new(Mutex::new(std::collections::HashMap::new())),
            by_id: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn add_user(&self, user: User) {
        let mut users = self.users.lock().unwrap();
        let mut by_username = self.by_username.lock().unwrap();
        let mut by_id = self.by_id.lock().unwrap();

        by_username.insert(user.username.clone(), user.id.clone());
        by_id.insert(user.id.clone(), user.id.clone());
        users.push(user);
    }

    pub fn with_user(&self, user: User) -> Self {
        self.add_user(user.clone());
        self.clone()
    }
}

impl Default for InMemoryUserRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, RepositoryError> {
        let users = self.users.lock().unwrap();
        Ok(users.iter().find(|u| u.username == username).cloned())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        let users = self.users.lock().unwrap();
        Ok(users.iter().find(|u| u.id == id).cloned())
    }

    async fn find_id_by_username(&self, username: &str) -> Result<Option<String>, RepositoryError> {
        let users = self.users.lock().unwrap();
        Ok(users.iter().find(|u| u.username == username).map(|u| u.id.clone()))
    }

    async fn create(&self, id: &str, username: &str, email: &str, password_hash: &str) -> Result<(), RepositoryError> {
        let mut users = self.users.lock().unwrap();
        let mut by_username = self.by_username.lock().unwrap();
        let mut by_id = self.by_id.lock().unwrap();

        if users.iter().any(|u| u.username == username) {
            return Err(RepositoryError::UniqueViolation("username".into()));
        }

        let user = User {
            id: id.to_string(),
            username: username.to_string(),
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
        };

        by_username.insert(username.to_string(), id.to_string());
        by_id.insert(id.to_string(), id.to_string());
        users.push(user);
        Ok(())
    }

    async fn exists(&self, username: &str) -> Result<bool, RepositoryError> {
        let users = self.users.lock().unwrap();
        Ok(users.iter().any(|u| u.username == username))
    }

    async fn get_role(&self, _user_id: &str, _site_id: &str) -> Result<Option<String>, RepositoryError> {
        Ok(Some("owner".to_string()))
    }
}

#[derive(Clone)]
pub struct InMemorySiteRepository {
    sites: Arc<Mutex<Vec<Site>>>,
    site_with_roles: Arc<Mutex<Vec<SiteWithRole>>>,
    members: Arc<Mutex<Vec<SiteMember>>>,
}

impl InMemorySiteRepository {
    pub fn new() -> Self {
        Self {
            sites: Arc::new(Mutex::new(Vec::new())),
            site_with_roles: Arc::new(Mutex::new(Vec::new())),
            members: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_site(&self, site: Site) {
        let mut sites = self.sites.lock().unwrap();
        sites.push(site);
    }

    pub fn add_site_with_role(&self, site: SiteWithRole) {
        let mut sites = self.site_with_roles.lock().unwrap();
        sites.push(site);
    }

    pub fn add_member(&self, member: SiteMember) {
        let mut members = self.members.lock().unwrap();
        members.push(member);
    }
}

impl Default for InMemorySiteRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SiteRepository for InMemorySiteRepository {
    async fn list_all(&self) -> Result<Vec<Site>, RepositoryError> {
        let sites = self.sites.lock().unwrap();
        Ok(sites.clone())
    }

    async fn list_for_user(&self, user_id: &str) -> Result<Vec<SiteWithRole>, RepositoryError> {
        let sites = self.site_with_roles.lock().unwrap();
        Ok(sites
            .iter()
            .filter(|s| s.created_by == user_id || s.role != "none")
            .cloned()
            .collect())
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Site>, RepositoryError> {
        let sites = self.sites.lock().unwrap();
        Ok(sites.iter().find(|s| s.id == id).cloned())
    }

    async fn create(
        &self,
        id: &str,
        name: &str,
        storage_provider: &str,
        created_by: &str,
    ) -> Result<Site, RepositoryError> {
        let mut sites = self.sites.lock().unwrap();
        let site = Site {
            id: id.to_string(),
            name: name.to_string(),
            storage_provider: storage_provider.to_string(),
            created_by: created_by.to_string(),
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
        };
        sites.push(site.clone());
        Ok(site)
    }

    async fn update(&self, id: &str, name: &str) -> Result<Site, RepositoryError> {
        let mut sites = self.sites.lock().unwrap();
        if let Some(site) = sites.iter_mut().find(|s| s.id == id) {
            site.name = name.to_string();
            site.updated_at = now_timestamp();
            return Ok(site.clone());
        }
        Err(RepositoryError::NotFound)
    }

    async fn delete(&self, id: &str) -> Result<u64, RepositoryError> {
        let mut sites = self.sites.lock().unwrap();
        let len = sites.len();
        sites.retain(|s| s.id != id);
        Ok((len - sites.len()) as u64)
    }

    async fn list_members(&self, site_id: &str) -> Result<Vec<SiteMember>, RepositoryError> {
        let members = self.members.lock().unwrap();
        Ok(members.iter().filter(|m| m.site_id == site_id).cloned().collect())
    }

    async fn add_member(
        &self,
        id: &str,
        site_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<SiteMember, RepositoryError> {
        let mut members = self.members.lock().unwrap();
        let member = SiteMember {
            id: id.to_string(),
            site_id: site_id.to_string(),
            user_id: user_id.to_string(),
            username: format!("user_{}", user_id),
            email: format!("{}@example.com", user_id),
            role: role.to_string(),
            created_at: now_timestamp(),
        };
        members.push(member.clone());
        Ok(member)
    }

    async fn update_member_role(
        &self,
        site_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<Option<SiteMember>, RepositoryError> {
        let mut members = self.members.lock().unwrap();
        if let Some(member) = members
            .iter_mut()
            .find(|m| m.site_id == site_id && m.user_id == user_id)
        {
            member.role = role.to_string();
            return Ok(Some(member.clone()));
        }
        Ok(None)
    }

    async fn remove_member(&self, site_id: &str, user_id: &str) -> Result<u64, RepositoryError> {
        let mut members = self.members.lock().unwrap();
        let len = members.len();
        members.retain(|m| !(m.site_id == site_id && m.user_id == user_id));
        Ok((len - members.len()) as u64)
    }
}

#[derive(Clone)]
pub struct InMemoryCollectionRepository {
    collections: Arc<Mutex<Vec<Collection>>>,
}

impl InMemoryCollectionRepository {
    pub fn new() -> Self {
        Self {
            collections: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_collection(&self, collection: Collection) {
        let mut collections = self.collections.lock().unwrap();
        collections.push(collection);
    }
}

impl Default for InMemoryCollectionRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CollectionRepository for InMemoryCollectionRepository {
    async fn list(&self, site_id: &str) -> Result<Vec<Collection>, RepositoryError> {
        let collections = self.collections.lock().unwrap();
        Ok(collections.iter().filter(|c| c.site_id == site_id).cloned().collect())
    }

    async fn list_singletons_only(&self, site_id: &str) -> Result<Vec<Collection>, RepositoryError> {
        let collections = self.collections.lock().unwrap();
        Ok(collections
            .iter()
            .filter(|c| c.site_id == site_id && c.is_singleton)
            .cloned()
            .collect())
    }

    async fn get_by_slug(&self, site_id: &str, slug: &str) -> Result<Option<Collection>, RepositoryError> {
        let collections = self.collections.lock().unwrap();
        Ok(collections
            .iter()
            .find(|c| c.site_id == site_id && c.slug == slug)
            .cloned())
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Collection>, RepositoryError> {
        let collections = self.collections.lock().unwrap();
        Ok(collections.iter().find(|c| c.id == id).cloned())
    }

    async fn create(
        &self,
        id: &str,
        site_id: &str,
        name: &str,
        slug: &str,
        definition: &str,
        is_singleton: bool,
    ) -> Result<Collection, RepositoryError> {
        let mut collections = self.collections.lock().unwrap();
        let collection = Collection {
            id: id.to_string(),
            site_id: site_id.to_string(),
            name: name.to_string(),
            slug: slug.to_string(),
            definition: definition.to_string(),
            is_singleton,
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
        };
        collections.push(collection.clone());
        Ok(collection)
    }

    async fn update(&self, id: &str, name: &str, slug: &str, definition: &str) -> Result<Collection, RepositoryError> {
        let mut collections = self.collections.lock().unwrap();
        if let Some(col) = collections.iter_mut().find(|c| c.id == id) {
            col.name = name.to_string();
            col.slug = slug.to_string();
            col.definition = definition.to_string();
            col.updated_at = now_timestamp();
            return Ok(col.clone());
        }
        Err(RepositoryError::NotFound)
    }

    async fn delete(&self, site_id: &str, slug: &str) -> Result<u64, RepositoryError> {
        let mut collections = self.collections.lock().unwrap();
        let len = collections.len();
        collections.retain(|c| !(c.site_id == site_id && c.slug == slug));
        Ok((len - collections.len()) as u64)
    }

    async fn get_content_for_migration(&self, _collection_id: &str) -> Result<Vec<Entry>, RepositoryError> {
        Ok(Vec::new())
    }

    async fn migrate_content_field_renames(
        &self,
        _content_items: &[Entry],
        _rename_map: &std::collections::HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct InMemoryEntryRepository {
    entries: Arc<Mutex<Vec<Entry>>>,
    revisions: Arc<Mutex<Vec<EntryRevision>>>,
}

impl InMemoryEntryRepository {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            revisions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_entry(&self, entry: Entry) {
        let mut entries = self.entries.lock().unwrap();
        entries.push(entry);
    }
}

impl Default for InMemoryEntryRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EntryRepository for InMemoryEntryRepository {
    async fn get_by_id(
        &self,
        id: &str,
        site_id: &str,
        _published_only: bool,
    ) -> Result<Option<Entry>, RepositoryError> {
        let entries = self.entries.lock().unwrap();
        Ok(entries.iter().find(|e| e.id == id && e.site_id == site_id).cloned())
    }

    async fn get_by_id_any_site(&self, id: &str) -> Result<Option<Entry>, RepositoryError> {
        let entries = self.entries.lock().unwrap();
        Ok(entries.iter().find(|e| e.id == id).cloned())
    }

    async fn list(&self, params: ListEntriesParams<'_>) -> Result<EntriesListResult, RepositoryError> {
        let entries = self.entries.lock().unwrap();
        let filtered: Vec<Entry> = entries
            .iter()
            .filter(|e| e.site_id == params.site_id)
            .filter(|_| params.collection_slug.is_none() || { true })
            .filter(|e| params.status.is_none() || e.status == params.status.unwrap())
            .filter(|e| {
                if params.published_only {
                    e.status == "published"
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        let total = filtered.len() as i64;
        Ok(EntriesListResult {
            items: filtered,
            total,
            page: params.page,
            per_page: params.per_page,
        })
    }

    async fn get_by_collection_id(
        &self,
        collection_id: &str,
        status: Option<&str>,
        published_only: bool,
    ) -> Result<Vec<Entry>, RepositoryError> {
        let entries = self.entries.lock().unwrap();
        Ok(entries
            .iter()
            .filter(|e| e.collection_id == collection_id)
            .filter(|e| status.is_none() || e.status == status.unwrap())
            .filter(|e| !published_only || e.status == "published")
            .cloned()
            .collect())
    }

    async fn create(
        &self,
        id: &str,
        site_id: &str,
        collection_id: &str,
        data: &str,
        slug: &str,
        created_by: Option<&str>,
    ) -> Result<Entry, RepositoryError> {
        let mut entries = self.entries.lock().unwrap();
        let entry = Entry {
            id: id.to_string(),
            site_id: site_id.to_string(),
            collection_id: collection_id.to_string(),
            data: data.to_string(),
            slug: slug.to_string(),
            status: "draft".to_string(),
            singleton_collection_id: None,
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
            published_at: None,
        };
        entries.push(entry.clone());

        let data_json: serde_json::Value = serde_json::from_str(data).unwrap_or(serde_json::Value::Null);
        let revision = EntryRevision {
            id: uuid::Uuid::now_v7().to_string(),
            entry_id: id.to_string(),
            revision_number: 1,
            data: sqlx::types::Json(data_json),
            created_by: created_by.map(|s| s.to_string()),
            created_at: now_timestamp(),
            change_summary: None,
        };
        self.revisions.lock().unwrap().push(revision);

        Ok(entry)
    }

    async fn update(
        &self,
        id: &str,
        _site_id: &str,
        data: &str,
        slug: &str,
        status: &str,
        created_by: Option<&str>,
        change_summary: Option<&str>,
    ) -> Result<Entry, RepositoryError> {
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
            entry.data = data.to_string();
            entry.slug = slug.to_string();
            entry.status = status.to_string();
            entry.updated_at = now_timestamp();

            let data_json: serde_json::Value = serde_json::from_str(data).unwrap_or(serde_json::Value::Null);
            let mut revisions = self.revisions.lock().unwrap();
            let next_number = revisions
                .iter()
                .filter(|r| r.entry_id == id)
                .map(|r| r.revision_number)
                .max()
                .unwrap_or(0)
                + 1;
            let revision = EntryRevision {
                id: uuid::Uuid::now_v7().to_string(),
                entry_id: id.to_string(),
                revision_number: next_number,
                data: sqlx::types::Json(data_json),
                created_by: created_by.map(|s| s.to_string()),
                created_at: now_timestamp(),
                change_summary: change_summary.map(|s| s.to_string()),
            };
            revisions.push(revision);

            return Ok(entry.clone());
        }
        Err(RepositoryError::NotFound)
    }

    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let mut entries = self.entries.lock().unwrap();
        let len = entries.len();
        entries.retain(|e| !(e.id == id && e.site_id == site_id));
        Ok((len - entries.len()) as u64)
    }

    async fn publish(&self, id: &str, site_id: &str) -> Result<Entry, RepositoryError> {
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id && e.site_id == site_id) {
            entry.status = "published".to_string();
            entry.published_at = Some(now_timestamp());
            entry.updated_at = now_timestamp();
            return Ok(entry.clone());
        }
        Err(RepositoryError::NotFound)
    }

    async fn unpublish(&self, id: &str, site_id: &str) -> Result<Entry, RepositoryError> {
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id && e.site_id == site_id) {
            entry.status = "draft".to_string();
            entry.updated_at = now_timestamp();
            return Ok(entry.clone());
        }
        Err(RepositoryError::NotFound)
    }

    async fn sync_file_references(
        &self,
        _entry_id: &str,
        _site_id: &str,
        _data: &serde_json::Value,
    ) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn list_revisions(
        &self,
        entry_id: &str,
        page: i64,
        per_page: i64,
    ) -> Result<RevisionsListResult, RepositoryError> {
        let revisions = self.revisions.lock().unwrap();
        let mut filtered: Vec<EntryRevision> = revisions.iter().filter(|r| r.entry_id == entry_id).cloned().collect();
        filtered.sort_by(|a, b| b.revision_number.cmp(&a.revision_number));

        let total = filtered.len() as i64;
        let offset = ((page - 1) * per_page) as usize;
        let items = filtered.into_iter().skip(offset).take(per_page as usize).collect();

        Ok(RevisionsListResult {
            items,
            total,
            page,
            per_page,
        })
    }

    async fn get_revision(
        &self,
        entry_id: &str,
        revision_number: i64,
    ) -> Result<Option<EntryRevision>, RepositoryError> {
        let revisions = self.revisions.lock().unwrap();
        Ok(revisions
            .iter()
            .find(|r| r.entry_id == entry_id && r.revision_number == revision_number)
            .cloned())
    }

    async fn restore_revision(
        &self,
        entry_id: &str,
        revision_number: i64,
        created_by: Option<&str>,
    ) -> Result<Entry, RepositoryError> {
        let revisions = self.revisions.lock().unwrap();
        let revision = revisions
            .iter()
            .find(|r| r.entry_id == entry_id && r.revision_number == revision_number)
            .cloned()
            .ok_or(RepositoryError::NotFound)?;
        drop(revisions);

        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter_mut().find(|e| e.id == entry_id) {
            entry.data = serde_json::to_string(&revision.data.0).unwrap_or_default();
            entry.updated_at = now_timestamp();

            let mut revisions = self.revisions.lock().unwrap();
            let next_number = revisions
                .iter()
                .filter(|r| r.entry_id == entry_id)
                .map(|r| r.revision_number)
                .max()
                .unwrap_or(0)
                + 1;
            let new_revision = EntryRevision {
                id: uuid::Uuid::now_v7().to_string(),
                entry_id: entry_id.to_string(),
                revision_number: next_number,
                data: revision.data,
                created_by: created_by.map(|s| s.to_string()),
                created_at: now_timestamp(),
                change_summary: Some(format!("Restored from revision {}", revision_number)),
            };
            revisions.push(new_revision);

            return Ok(entry.clone());
        }
        Err(RepositoryError::NotFound)
    }

    async fn get_singleton_entry(
        &self,
        site_id: &str,
        slug: &str,
    ) -> Result<Option<Entry>, RepositoryError> {
        let entries = self.entries.lock().unwrap();
        Ok(entries
            .iter()
            .find(|e| e.site_id == site_id && e.singleton_collection_id.is_some() && e.slug == slug)
            .cloned())
    }

    async fn upsert_singleton_entry(
        &self,
        site_id: &str,
        collection_id: &str,
        slug: &str,
        data: &str,
        created_by: Option<&str>,
        change_summary: Option<&str>,
    ) -> Result<Entry, RepositoryError> {
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries
            .iter_mut()
            .find(|e| e.singleton_collection_id.as_deref() == Some(collection_id))
        {
            entry.data = data.to_string();
            entry.updated_at = now_timestamp();

            let mut revisions = self.revisions.lock().unwrap();
            let next_number = revisions
                .iter()
                .filter(|r| r.entry_id == entry.id)
                .map(|r| r.revision_number)
                .max()
                .unwrap_or(0)
                + 1;
            let data_json: serde_json::Value = serde_json::from_str(data).unwrap_or(serde_json::Value::Null);
            let revision = EntryRevision {
                id: uuid::Uuid::now_v7().to_string(),
                entry_id: entry.id.clone(),
                revision_number: next_number,
                data: sqlx::types::Json(data_json),
                created_by: created_by.map(|s| s.to_string()),
                created_at: now_timestamp(),
                change_summary: change_summary.map(|s| s.to_string()),
            };
            revisions.push(revision);

            return Ok(entry.clone());
        }
        let id = uuid::Uuid::now_v7().to_string();
        let entry = Entry {
            id: id.clone(),
            site_id: site_id.to_string(),
            collection_id: collection_id.to_string(),
            data: data.to_string(),
            slug: slug.to_string(),
            status: "draft".to_string(),
            singleton_collection_id: Some(collection_id.to_string()),
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
            published_at: None,
        };
        entries.push(entry.clone());

        let data_json: serde_json::Value = serde_json::from_str(data).unwrap_or(serde_json::Value::Null);
        let revision = EntryRevision {
            id: uuid::Uuid::now_v7().to_string(),
            entry_id: id,
            revision_number: 1,
            data: sqlx::types::Json(data_json),
            created_by: created_by.map(|s| s.to_string()),
            created_at: now_timestamp(),
            change_summary: change_summary.map(|s| s.to_string()),
        };
        self.revisions.lock().unwrap().push(revision);

        Ok(entry)
    }

    async fn migrate_singleton_field_renames(
        &self,
        _site_id: &str,
        _collection_id: &str,
        _rename_map: &std::collections::HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct InMemoryFileRepository {
    files: Arc<Mutex<Vec<File>>>,
}

impl InMemoryFileRepository {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_file(&self, file: File) {
        let mut files = self.files.lock().unwrap();
        files.push(file);
    }
}

impl Default for InMemoryFileRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileRepository for InMemoryFileRepository {
    async fn get_by_id(&self, id: &str, site_id: &str) -> Result<Option<File>, RepositoryError> {
        let files = self.files.lock().unwrap();
        Ok(files
            .iter()
            .find(|f| f.id == id && f.site_id == site_id && f.deleted_at.is_none())
            .cloned())
    }

    async fn get_by_id_any(&self, id: &str) -> Result<Option<File>, RepositoryError> {
        let files = self.files.lock().unwrap();
        Ok(files.iter().find(|f| f.id == id).cloned())
    }

    async fn list(&self, params: ListFilesParams<'_>) -> Result<FileListResult, RepositoryError> {
        let files = self.files.lock().unwrap();
        let filtered: Vec<File> = files
            .iter()
            .filter(|f| f.site_id == params.site_id)
            .filter(|f| params.trashed == f.deleted_at.is_some())
            .filter(|f| params.search.is_none() || { f.filename.contains(params.search.unwrap()) })
            .cloned()
            .collect();

        let total = filtered.len() as i64;
        Ok(FileListResult {
            items: filtered,
            total,
            page: params.page,
            per_page: params.per_page,
        })
    }

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
    ) -> Result<File, RepositoryError> {
        let mut files = self.files.lock().unwrap();
        let file = File {
            id: id.to_string(),
            site_id: site_id.to_string(),
            filename: filename.to_string(),
            original_name: original_name.to_string(),
            mime_type: mime_type.to_string(),
            size,
            storage_provider: storage_provider.to_string(),
            storage_key: storage_key.to_string(),
            thumbnail_key: thumbnail_key.map(|s| s.to_string()),
            width,
            height,
            deleted_at: None,
            created_by: created_by.map(|s| s.to_string()),
            created_at: now_timestamp(),
        };
        files.push(file.clone());
        Ok(file)
    }

    async fn soft_delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let mut files = self.files.lock().unwrap();
        if let Some(file) = files.iter_mut().find(|f| f.id == id && f.site_id == site_id) {
            file.deleted_at = Some(now_timestamp());
            return Ok(1);
        }
        Ok(0)
    }

    async fn restore(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let mut files = self.files.lock().unwrap();
        if let Some(file) = files.iter_mut().find(|f| f.id == id && f.site_id == site_id) {
            file.deleted_at = None;
            return Ok(1);
        }
        Ok(0)
    }

    async fn batch_soft_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, RepositoryError> {
        let mut files = self.files.lock().unwrap();
        let mut count = 0u64;
        for id in ids {
            if let Some(file) = files.iter_mut().find(|f| f.id == *id && f.site_id == site_id) {
                file.deleted_at = Some(now_timestamp());
                count += 1;
            }
        }
        Ok(count)
    }

    async fn batch_restore(&self, site_id: &str, ids: &[String]) -> Result<u64, RepositoryError> {
        let mut files = self.files.lock().unwrap();
        let mut count = 0u64;
        for id in ids {
            if let Some(file) = files.iter_mut().find(|f| f.id == *id && f.site_id == site_id) {
                file.deleted_at = None;
                count += 1;
            }
        }
        Ok(count)
    }

    async fn get_by_ids(&self, site_id: &str, ids: &[String]) -> Result<Vec<File>, RepositoryError> {
        let files = self.files.lock().unwrap();
        Ok(files
            .iter()
            .filter(|f| f.site_id == site_id && ids.contains(&f.id) && f.deleted_at.is_none())
            .cloned()
            .collect())
    }

    async fn get_deleted_by_ids(&self, site_id: &str, ids: &[String]) -> Result<Vec<File>, RepositoryError> {
        let files = self.files.lock().unwrap();
        Ok(files
            .iter()
            .filter(|f| f.site_id == site_id && ids.contains(&f.id) && f.deleted_at.is_some())
            .cloned()
            .collect())
    }

    async fn batch_permanent_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, RepositoryError> {
        let mut files = self.files.lock().unwrap();
        let len = files.len();
        files.retain(|f| !(f.site_id == site_id && ids.contains(&f.id)));
        Ok((len - files.len()) as u64)
    }

    async fn get_references(&self, _file_id: &str) -> Result<Vec<FileReference>, RepositoryError> {
        Ok(Vec::new())
    }

    async fn get_references_for_site(
        &self,
        _file_id: &str,
        _site_id: &str,
    ) -> Result<Vec<FileReference>, RepositoryError> {
        Ok(Vec::new())
    }

    async fn get_storage_provider(&self, site_id: &str) -> Result<String, RepositoryError> {
        let files = self.files.lock().unwrap();
        Ok(files
            .iter()
            .find(|f| f.site_id == site_id)
            .map(|f| f.storage_provider.clone())
            .unwrap_or_else(|| "filesystem".to_string()))
    }
}

#[derive(Clone)]
pub struct InMemoryAccessTokenRepository {
    tokens: Arc<Mutex<Vec<AccessToken>>>,
}

impl InMemoryAccessTokenRepository {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_token(&self, token: AccessToken) {
        let mut tokens = self.tokens.lock().unwrap();
        tokens.push(token);
    }
}

impl Default for InMemoryAccessTokenRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AccessTokenRepository for InMemoryAccessTokenRepository {
    async fn list(&self, site_id: &str) -> Result<Vec<AccessToken>, RepositoryError> {
        let tokens = self.tokens.lock().unwrap();
        Ok(tokens
            .iter()
            .filter(|t| t.site_id == site_id)
            .cloned()
            .collect())
    }

    async fn create(
        &self,
        id: &str,
        site_id: &str,
        name: &str,
        _token_hash: &str,
        token_prefix: &str,
        token_hmac: &str,
        permission: &str,
        created_by_user_id: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let mut tokens = self.tokens.lock().unwrap();
        let token = AccessToken {
            id: id.to_string(),
            site_id: site_id.to_string(),
            name: name.to_string(),
            token_prefix: token_prefix.to_string(),
            permission: permission.to_string(),
            created_by_user_id: created_by_user_id.map(|s| s.to_string()),
            last_used_at: None,
            created_at: now_timestamp(),
            expires_at: None,
            revoked_at: None,
            token_hmac: Some(token_hmac.to_string()),
        };
        tokens.push(token);
        Ok(())
    }

    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let mut tokens = self.tokens.lock().unwrap();
        let len = tokens.len();
        tokens.retain(|t| !(t.id == id && t.site_id == site_id));
        Ok((len - tokens.len()) as u64)
    }

    async fn find_by_prefix(&self, prefix: &str) -> Result<Vec<AccessTokenLookupRow>, RepositoryError> {
        let tokens = self.tokens.lock().unwrap();
        Ok(tokens
            .iter()
            .filter(|t| t.token_prefix.starts_with(prefix))
            .map(|t| {
                (
                    t.id.clone(),
                    t.site_id.clone(),
                    String::new(),
                    t.token_hmac.clone(),
                    t.expires_at.clone(),
                    t.revoked_at.clone(),
                    t.permission.clone(),
                )
            })
            .collect())
    }

    async fn update_last_used(&self, _id: &str) -> Result<(), RepositoryError> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct InMemoryWebhookRepository {
    webhooks: Arc<Mutex<Vec<crate::models::webhook::SiteWebhook>>>,
    deliveries: Arc<Mutex<Vec<crate::models::webhook::WebhookDelivery>>>,
}

impl InMemoryWebhookRepository {
    pub fn new() -> Self {
        Self {
            webhooks: Arc::new(Mutex::new(Vec::new())),
            deliveries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_webhook(&self, webhook: crate::models::webhook::SiteWebhook) {
        let mut webhooks = self.webhooks.lock().unwrap();
        webhooks.push(webhook);
    }
}

impl Default for InMemoryWebhookRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::repository::traits::WebhookRepository for InMemoryWebhookRepository {
    async fn list_for_site(&self, site_id: &str) -> Result<Vec<crate::models::webhook::SiteWebhook>, RepositoryError> {
        let webhooks = self.webhooks.lock().unwrap();
        Ok(webhooks.iter().filter(|w| w.site_id == site_id).cloned().collect())
    }

    async fn get_by_id(&self, id: &str, site_id: &str) -> Result<Option<crate::models::webhook::SiteWebhook>, RepositoryError> {
        let webhooks = self.webhooks.lock().unwrap();
        Ok(webhooks.iter().find(|w| w.id == id && w.site_id == site_id).cloned())
    }

    async fn create(
        &self,
        id: &str,
        site_id: &str,
        label: &str,
        url: &str,
        headers_encrypted: &str,
        created_by: Option<&str>,
    ) -> Result<crate::models::webhook::SiteWebhook, RepositoryError> {
        let mut webhooks = self.webhooks.lock().unwrap();
        let webhook = crate::models::webhook::SiteWebhook {
            id: id.to_string(),
            site_id: site_id.to_string(),
            label: label.to_string(),
            url: url.to_string(),
            headers_encrypted: headers_encrypted.to_string(),
            created_by: created_by.map(|s| s.to_string()),
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
        };
        webhooks.push(webhook.clone());
        Ok(webhook)
    }

    async fn update(
        &self,
        id: &str,
        label: Option<&str>,
        url: Option<&str>,
        headers_encrypted: Option<&str>,
    ) -> Result<crate::models::webhook::SiteWebhook, RepositoryError> {
        let mut webhooks = self.webhooks.lock().unwrap();
        if let Some(webhook) = webhooks.iter_mut().find(|w| w.id == id) {
            if let Some(l) = label {
                webhook.label = l.to_string();
            }
            if let Some(u) = url {
                webhook.url = u.to_string();
            }
            if let Some(h) = headers_encrypted {
                webhook.headers_encrypted = h.to_string();
            }
            webhook.updated_at = now_timestamp();
            return Ok(webhook.clone());
        }
        Err(RepositoryError::NotFound)
    }

    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let mut webhooks = self.webhooks.lock().unwrap();
        let len = webhooks.len();
        webhooks.retain(|w| !(w.id == id && w.site_id == site_id));
        Ok((len - webhooks.len()) as u64)
    }

    async fn create_delivery(
        &self,
        id: &str,
        webhook_id: &str,
        status: &str,
        status_code: Option<i32>,
        response_body: Option<&str>,
        duration_ms: Option<i64>,
        triggered_by: Option<&str>,
    ) -> Result<crate::models::webhook::WebhookDelivery, RepositoryError> {
        let mut deliveries = self.deliveries.lock().unwrap();
        let delivery = crate::models::webhook::WebhookDelivery {
            id: id.to_string(),
            webhook_id: webhook_id.to_string(),
            status: status.to_string(),
            status_code,
            response_body: response_body.map(|s| s.to_string()),
            duration_ms,
            triggered_by: triggered_by.map(|s| s.to_string()),
            triggered_at: now_timestamp(),
        };
        deliveries.push(delivery.clone());
        Ok(delivery)
    }

    async fn list_deliveries(
        &self,
        webhook_id: &str,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<crate::models::webhook::WebhookDelivery>, i64), RepositoryError> {
        let deliveries = self.deliveries.lock().unwrap();
        let filtered: Vec<crate::models::webhook::WebhookDelivery> = deliveries
            .iter()
            .filter(|d| d.webhook_id == webhook_id)
            .cloned()
            .collect();
        let total = filtered.len() as i64;
        let offset = ((page - 1) * per_page) as usize;
        let items = filtered.into_iter().skip(offset).take(per_page as usize).collect();
        Ok((items, total))
    }
}
