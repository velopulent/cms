pub mod filesystem;
pub mod s3;

use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub use filesystem::FileSystemStorage;
pub use s3::S3Storage;

pub const STORAGE_KIND_FILESYSTEM: &str = "filesystem";
pub const STORAGE_KIND_S3: &str = "s3";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StorageKind {
    #[default]
    Filesystem,
    S3,
}

impl StorageKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            STORAGE_KIND_FILESYSTEM => Some(StorageKind::Filesystem),
            STORAGE_KIND_S3 => Some(StorageKind::S3),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            StorageKind::Filesystem => STORAGE_KIND_FILESYSTEM,
            StorageKind::S3 => STORAGE_KIND_S3,
        }
    }
}

pub struct StorageRegistry {
    providers: RwLock<HashMap<String, Arc<dyn StorageProvider>>>,
}

impl StorageRegistry {
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, name: &str, provider: Arc<dyn StorageProvider>) {
        self.providers
            .write()
            .expect("storage registry poisoned")
            .insert(name.to_string(), provider);
    }

    pub fn remove(&self, name: &str) {
        self.providers.write().expect("storage registry poisoned").remove(name);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn StorageProvider>> {
        self.providers
            .read()
            .expect("storage registry poisoned")
            .get(name)
            .cloned()
    }

    pub fn get_by_kind(&self, kind: StorageKind) -> Option<Arc<dyn StorageProvider>> {
        self.get(kind.as_str())
    }
}

impl Default for StorageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn put(
        &self,
        key: &str,
        data: Bytes,
        content_type: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn get(&self, key: &str) -> Result<Bytes, Box<dyn std::error::Error + Send + Sync>>;
    async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn url(&self, key: &str, file_id: &str) -> String;
    /// Begin a streaming (multipart) upload to `key`. Drive it with
    /// [`object_store::WriteMultipart`]; `abort()` cleans up partial state.
    async fn start_multipart(
        &self,
        key: &str,
    ) -> Result<Box<dyn object_store::MultipartUpload>, Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Default)]
pub struct MockStorage {
    pub files: Arc<std::sync::Mutex<std::collections::HashMap<String, Bytes>>>,
}

/// In-memory [`object_store::MultipartUpload`] so `MockStorage` supports the
/// streaming path: parts accumulate in a buffer, `complete` publishes to the map.
#[derive(Debug)]
struct MockMultipartUpload {
    files: Arc<std::sync::Mutex<std::collections::HashMap<String, Bytes>>>,
    key: String,
    buf: Vec<u8>,
}

#[async_trait]
impl object_store::MultipartUpload for MockMultipartUpload {
    fn put_part(&mut self, data: object_store::PutPayload) -> object_store::UploadPart {
        for chunk in data.iter() {
            self.buf.extend_from_slice(chunk);
        }
        Box::pin(futures_util::future::ready(Ok(())))
    }

    async fn complete(&mut self) -> object_store::Result<object_store::PutResult> {
        let data = Bytes::from(std::mem::take(&mut self.buf));
        self.files.lock().unwrap().insert(self.key.clone(), data);
        Ok(object_store::PutResult {
            e_tag: None,
            version: None,
            extensions: Default::default(),
        })
    }

    async fn abort(&mut self) -> object_store::Result<()> {
        self.buf.clear();
        Ok(())
    }
}

#[async_trait]
impl StorageProvider for MockStorage {
    async fn put(&self, key: &str, data: Bytes, _: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.files.lock().unwrap().insert(key.to_string(), data);
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes, Box<dyn std::error::Error + Send + Sync>> {
        self.files
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or_else(|| "MockStorage: key not found".into())
    }

    async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.files.lock().unwrap().remove(key);
        Ok(())
    }

    fn url(&self, _key: &str, file_id: &str) -> String {
        format!("/mock/{}", file_id)
    }

    async fn start_multipart(
        &self,
        key: &str,
    ) -> Result<Box<dyn object_store::MultipartUpload>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(MockMultipartUpload {
            files: self.files.clone(),
            key: key.to_string(),
            buf: Vec::new(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_storage_put_and_get() {
        let storage = MockStorage::default();
        let data = Bytes::from("Hello, World!");

        storage.put("test.txt", data.clone(), "text/plain").await.unwrap();
        let retrieved = storage.get("test.txt").await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_mock_storage_get_nonexistent() {
        let storage = MockStorage::default();
        let result = storage.get("nonexistent.txt").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_storage_delete() {
        let storage = MockStorage::default();
        let data = Bytes::from("Hello, World!");

        storage.put("test.txt", data, "text/plain").await.unwrap();
        storage.delete("test.txt").await.unwrap();
        assert!(storage.get("test.txt").await.is_err());
    }

    #[tokio::test]
    async fn test_mock_storage_url() {
        let storage = MockStorage::default();
        assert_eq!(storage.url("test.txt", "file123"), "/mock/file123");
    }

    #[test]
    fn test_storage_kind_from_str() {
        assert_eq!(StorageKind::parse("filesystem"), Some(StorageKind::Filesystem));
        assert_eq!(StorageKind::parse("s3"), Some(StorageKind::S3));
        assert_eq!(StorageKind::parse("unknown"), None);
    }

    #[test]
    fn test_storage_kind_as_str() {
        assert_eq!(StorageKind::Filesystem.as_str(), "filesystem");
        assert_eq!(StorageKind::S3.as_str(), "s3");
    }

    #[test]
    fn test_storage_registry_register_and_get() {
        let registry = StorageRegistry::new();
        let storage: Arc<dyn StorageProvider> = Arc::new(MockStorage::default());

        registry.register("filesystem", storage.clone());
        assert!(registry.get("filesystem").is_some());
        assert!(registry.get("s3").is_none());
    }

    #[test]
    fn test_storage_registry_get_by_kind() {
        let registry = StorageRegistry::new();
        let storage: Arc<dyn StorageProvider> = Arc::new(MockStorage::default());

        registry.register("filesystem", storage);
        assert!(registry.get_by_kind(StorageKind::Filesystem).is_some());
        assert!(registry.get_by_kind(StorageKind::S3).is_none());
    }
}
