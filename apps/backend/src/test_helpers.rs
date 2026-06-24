use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::models::access_token::AccessToken;
use crate::models::collection::Collection;
use crate::models::entry::{Entry, EntryRevision};
use crate::models::file::{File, FileReference};
use crate::models::session::Session;
use crate::models::site::{Site, SiteMember, SiteWithRole};
use crate::models::user::User;
use crate::repository::error::RepositoryError;
use crate::repository::traits::{
    AccessTokenLookupRow, AccessTokenRepository, CollectionRepository, EntriesListResult, EntryRepository,
    FileListResult, FileRepository, ListEntriesParams, ListFilesParams, NewAccessToken, NewFile, NewWebhookDelivery,
    RevisionsListResult, SessionRepository, SiteRepository, UpdateEntryParams, UserRepository,
};

pub fn now_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Synchronize tests that mutate `VCMS_HOME` (process-global env var).
/// The single static `Mutex` here is shared by all callers across modules,
/// preventing races between `paths::tests` and `secrets::tests` etc.
pub fn with_home<T>(value: &Path, f: impl FnOnce() -> T) -> T {
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let previous = std::env::var_os(crate::paths::CMS_HOME_ENV);
    // SAFETY: guarded by LOCK so no other test reads/writes the env concurrently.
    unsafe { std::env::set_var(crate::paths::CMS_HOME_ENV, value) };
    let result = f();
    match previous {
        Some(v) => unsafe { std::env::set_var(crate::paths::CMS_HOME_ENV, v) },
        None => unsafe { std::env::remove_var(crate::paths::CMS_HOME_ENV) },
    }
    result
}

#[derive(Clone)]
pub struct InMemoryUserRepository {
    users: Arc<Mutex<Vec<User>>>,
    by_name: Arc<Mutex<std::collections::HashMap<String, String>>>,
    by_id: Arc<Mutex<std::collections::HashMap<String, String>>>,
}

impl InMemoryUserRepository {
    pub fn new() -> Self {
        Self {
            users: Arc::new(Mutex::new(Vec::new())),
            by_name: Arc::new(Mutex::new(std::collections::HashMap::new())),
            by_id: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn add_user(&self, user: User) {
        let mut users = self.users.lock().unwrap();
        let mut by_name = self.by_name.lock().unwrap();
        let mut by_id = self.by_id.lock().unwrap();

        by_name.insert(user.name.clone(), user.id.clone());
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
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError> {
        let users = self.users.lock().unwrap();
        Ok(users.iter().find(|u| u.email == email).cloned())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        let users = self.users.lock().unwrap();
        Ok(users.iter().find(|u| u.id == id).cloned())
    }

    async fn list(&self) -> Result<Vec<User>, RepositoryError> {
        Ok(self.users.lock().unwrap().clone())
    }

    async fn create(&self, id: &str, name: &str, email: &str, password_hash: &str) -> Result<(), RepositoryError> {
        let mut users = self.users.lock().unwrap();
        let mut by_name = self.by_name.lock().unwrap();
        let mut by_id = self.by_id.lock().unwrap();

        if users.iter().any(|u| u.email == email) {
            return Err(RepositoryError::UniqueViolation("email".into()));
        }

        let user = User {
            id: id.to_string(),
            name: name.to_string(),
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            instance_role: None,
            must_change_password: false,
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
        };

        by_name.insert(name.to_string(), id.to_string());
        by_id.insert(id.to_string(), id.to_string());
        users.push(user);
        Ok(())
    }

    async fn exists(&self, email: &str) -> Result<bool, RepositoryError> {
        let users = self.users.lock().unwrap();
        Ok(users.iter().any(|u| u.email == email))
    }

    async fn get_role(&self, _user_id: &str, _site_id: &str) -> Result<Option<String>, RepositoryError> {
        Ok(Some("editor".to_string()))
    }

    async fn count(&self) -> Result<i64, RepositoryError> {
        Ok(self.users.lock().unwrap().len() as i64)
    }

    async fn count_instance_owners(&self) -> Result<i64, RepositoryError> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .filter(|user| user.instance_role.as_deref() == Some("instance_owner"))
            .count() as i64)
    }

    async fn set_instance_role(&self, user_id: &str, role: Option<&str>) -> Result<u64, RepositoryError> {
        let mut users = self.users.lock().unwrap();
        if let Some(user) = users.iter_mut().find(|user| user.id == user_id) {
            user.instance_role = role.map(ToString::to_string);
            return Ok(1);
        }
        Ok(0)
    }

    async fn update_password(
        &self,
        user_id: &str,
        password_hash: &str,
        must_change: bool,
    ) -> Result<u64, RepositoryError> {
        let mut users = self.users.lock().unwrap();
        if let Some(user) = users.iter_mut().find(|user| user.id == user_id) {
            user.password_hash = password_hash.to_string();
            user.must_change_password = must_change;
            return Ok(1);
        }
        Ok(0)
    }

    async fn update_profile(&self, user_id: &str, name: &str, email: &str) -> Result<u64, RepositoryError> {
        let mut users = self.users.lock().unwrap();
        if users.iter().any(|u| u.email == email && u.id != user_id) {
            return Err(RepositoryError::UniqueViolation("email".into()));
        }
        if let Some(user) = users.iter_mut().find(|user| user.id == user_id) {
            user.name = name.to_string();
            user.email = email.to_string();
            return Ok(1);
        }
        Ok(0)
    }

    async fn delete(&self, user_id: &str) -> Result<u64, RepositoryError> {
        let mut users = self.users.lock().unwrap();
        let before = users.len();
        users.retain(|user| user.id != user_id);
        Ok((before - users.len()) as u64)
    }
}

#[derive(Clone, Default)]
pub struct InMemorySessionRepository {
    sessions: Arc<Mutex<Vec<Session>>>,
}

impl InMemorySessionRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionRepository for InMemorySessionRepository {
    async fn create(
        &self,
        id: &str,
        user_id: &str,
        token_hash: &str,
        csrf_token_hash: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError> {
        self.sessions.lock().unwrap().push(Session {
            id: id.to_string(),
            user_id: user_id.to_string(),
            token_hash: token_hash.to_string(),
            csrf_token_hash: csrf_token_hash.to_string(),
            created_at: now_timestamp(),
            expires_at: expires_at.to_string(),
            last_seen_at: now_timestamp(),
            revoked_at: None,
        });
        Ok(())
    }

    async fn find_active_by_hash(&self, token_hash: &str) -> Result<Option<Session>, RepositoryError> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .find(|session| session.token_hash == token_hash && session.revoked_at.is_none())
            .cloned())
    }

    async fn touch(&self, id: &str) -> Result<(), RepositoryError> {
        if let Some(session) = self
            .sessions
            .lock()
            .unwrap()
            .iter_mut()
            .find(|session| session.id == id)
        {
            session.last_seen_at = now_timestamp();
        }
        Ok(())
    }

    async fn revoke(&self, id: &str, user_id: &str) -> Result<u64, RepositoryError> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions
            .iter_mut()
            .find(|session| session.id == id && session.user_id == user_id && session.revoked_at.is_none())
        {
            session.revoked_at = Some(now_timestamp());
            return Ok(1);
        }
        Ok(0)
    }

    async fn revoke_all(&self, user_id: &str) -> Result<u64, RepositoryError> {
        let mut sessions = self.sessions.lock().unwrap();
        let mut count = 0;
        for session in sessions
            .iter_mut()
            .filter(|session| session.user_id == user_id && session.revoked_at.is_none())
        {
            session.revoked_at = Some(now_timestamp());
            count += 1;
        }
        Ok(count)
    }

    async fn list(&self, user_id: &str) -> Result<Vec<Session>, RepositoryError> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|session| session.user_id == user_id && session.revoked_at.is_none())
            .cloned()
            .collect())
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
            name: format!("user_{}", user_id),
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

    async fn get_by_collection_ids(
        &self,
        collection_ids: &[String],
        status: Option<&str>,
        published_only: bool,
    ) -> Result<Vec<Entry>, RepositoryError> {
        let entries = self.entries.lock().unwrap();
        Ok(entries
            .iter()
            .filter(|e| collection_ids.iter().any(|c| c == &e.collection_id))
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

    async fn update(&self, params: UpdateEntryParams<'_>) -> Result<Entry, RepositoryError> {
        let UpdateEntryParams {
            id,
            site_id,
            data,
            slug,
            status,
            created_by,
            change_summary,
        } = params;
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id && e.site_id == site_id) {
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
        let mut entries = self.entries.lock().unwrap();
        let entry = match entries.iter_mut().find(|e| e.id == entry_id) {
            Some(entry) => entry,
            None => return Err(RepositoryError::NotFound),
        };

        let mut revisions = self.revisions.lock().unwrap();
        let revision = revisions
            .iter()
            .find(|r| r.entry_id == entry_id && r.revision_number == revision_number)
            .cloned()
            .ok_or(RepositoryError::NotFound)?;

        entry.data = serde_json::to_string(&revision.data.0).unwrap_or_default();
        entry.updated_at = now_timestamp();

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

        Ok(entry.clone())
    }

    async fn get_singleton_entry(&self, site_id: &str, slug: &str) -> Result<Option<Entry>, RepositoryError> {
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

    async fn create(&self, file: NewFile<'_>) -> Result<File, RepositoryError> {
        let NewFile {
            id,
            site_id,
            filename,
            original_name,
            mime_type,
            size,
            storage_provider,
            storage_key,
            thumbnail_key,
            width,
            height,
            created_by,
        } = file;
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
        Ok(tokens.iter().filter(|t| t.site_id == site_id).cloned().collect())
    }

    async fn create(&self, token: NewAccessToken<'_>) -> Result<(), RepositoryError> {
        let NewAccessToken {
            id,
            site_id,
            name,
            token_hash: _,
            token_prefix,
            token_hmac,
            permission,
            created_by_user_id,
        } = token;
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

    async fn get_by_id(
        &self,
        id: &str,
        site_id: &str,
    ) -> Result<Option<crate::models::webhook::SiteWebhook>, RepositoryError> {
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
        delivery: NewWebhookDelivery<'_>,
    ) -> Result<crate::models::webhook::WebhookDelivery, RepositoryError> {
        let NewWebhookDelivery {
            id,
            webhook_id,
            status,
            status_code,
            response_body,
            duration_ms,
            triggered_by,
        } = delivery;
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
