pub mod filesystem;
pub mod s3;

use async_trait::async_trait;
use bytes::Bytes;

pub use filesystem::FileSystemStorage;
pub use s3::S3Storage;

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn put(&self, key: &str, data: Bytes, content_type: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn get(&self, key: &str) -> Result<Bytes, Box<dyn std::error::Error + Send + Sync>>;
    async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn url(&self, key: &str, file_id: &str) -> String;
}

#[derive(Default)]
pub struct MockStorage {
    pub files: std::sync::Mutex<std::collections::HashMap<String, Bytes>>,
}

#[async_trait]
impl StorageProvider for MockStorage {
    async fn put(&self, key: &str, data: Bytes, _: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.files.lock().unwrap().insert(key.to_string(), data);
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes, Box<dyn std::error::Error + Send + Sync>> {
        self.files.lock().unwrap()
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
}