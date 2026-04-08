use bytes::Bytes;
use object_store::ObjectStoreExt;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct FileSystemStorage {
    store: Arc<LocalFileSystem>,
    root_path: String,
}

impl FileSystemStorage {
    pub fn new(root_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(root_path)?;
        let store = LocalFileSystem::new_with_prefix(root_path)?;
        Ok(Self {
            store: Arc::new(store),
            root_path: root_path.to_string(),
        })
    }

    pub async fn put(
        &self,
        key: &str,
        data: Bytes,
        _content_type: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = ObjectPath::from(key);
        let payload = object_store::PutPayload::from_bytes(data);
        self.store.put(&path, payload).await?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Bytes, Box<dyn std::error::Error>> {
        let path = ObjectPath::from(key);
        let result = self.store.get(&path).await?;
        let bytes = result.bytes().await?;
        Ok(bytes)
    }

    pub async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = ObjectPath::from(key);
        self.store.delete(&path).await?;

        // Clean up empty parent directory (the f_{file_id} level)
        if let Some(parent) = Path::new(key).parent() {
            let _ = std::fs::remove_dir(Path::new(&self.root_path).join(parent));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_filesystem_storage_put_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSystemStorage::new(temp_dir.path().to_str().unwrap()).unwrap();

        let data = Bytes::from("Hello, World!");
        storage.put("test.txt", data.clone(), "text/plain").await.unwrap();

        let retrieved = storage.get("test.txt").await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_filesystem_storage_put_and_delete() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSystemStorage::new(temp_dir.path().to_str().unwrap()).unwrap();

        let data = Bytes::from("Hello, World!");
        storage.put("test.txt", data, "text/plain").await.unwrap();

        storage.delete("test.txt").await.unwrap();

        let result = storage.get("test.txt").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_filesystem_storage_get_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSystemStorage::new(temp_dir.path().to_str().unwrap()).unwrap();

        let result = storage.get("nonexistent.txt").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_filesystem_storage_nested_path() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSystemStorage::new(temp_dir.path().to_str().unwrap()).unwrap();

        let data = Bytes::from("Nested content");
        storage.put("dir1/dir2/test.txt", data.clone(), "text/plain").await.unwrap();

        let retrieved = storage.get("dir1/dir2/test.txt").await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_filesystem_storage_overwrite() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSystemStorage::new(temp_dir.path().to_str().unwrap()).unwrap();

        let data1 = Bytes::from("First content");
        let data2 = Bytes::from("Second content");

        storage.put("test.txt", data1, "text/plain").await.unwrap();
        storage.put("test.txt", data2.clone(), "text/plain").await.unwrap();

        let retrieved = storage.get("test.txt").await.unwrap();
        assert_eq!(retrieved, data2);
    }

    #[tokio::test]
    async fn test_filesystem_storage_empty_content() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSystemStorage::new(temp_dir.path().to_str().unwrap()).unwrap();

        let data = Bytes::from("");
        storage.put("empty.txt", data, "text/plain").await.unwrap();

        let retrieved = storage.get("empty.txt").await.unwrap();
        assert!(retrieved.is_empty());
    }

    #[tokio::test]
    async fn test_filesystem_storage_binary_content() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSystemStorage::new(temp_dir.path().to_str().unwrap()).unwrap();

        let data = Bytes::from(vec![0u8, 1, 2, 3, 255]);
        storage.put("binary.bin", data.clone(), "application/octet-stream").await.unwrap();

        let retrieved = storage.get("binary.bin").await.unwrap();
        assert_eq!(retrieved, data);
    }
}
