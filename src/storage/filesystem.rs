use bytes::Bytes;
use object_store::ObjectStoreExt;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use std::sync::Arc;

#[derive(Clone)]
pub struct FileSystemStorage {
    store: Arc<LocalFileSystem>,
}

impl FileSystemStorage {
    pub fn new(root_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(root_path)?;
        let store = LocalFileSystem::new_with_prefix(root_path)?;
        Ok(Self {
            store: Arc::new(store),
        })
    }

    pub async fn put(&self, key: &str, data: Bytes, _content_type: &str) -> Result<(), Box<dyn std::error::Error>> {
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
        Ok(())
    }

    /// Store a file by reading directly from a filesystem path.
    /// Avoids loading the entire file into a Bytes allocation.
    pub async fn put_from_path(&self, path: &std::path::Path, key: &str, _content_type: &str) -> Result<(), Box<dyn std::error::Error>> {
        eprintln!("[FileSystemStorage::put_from_path] Reading file from {:?}", path);
        let data = tokio::fs::read(path).await?;
        eprintln!("[FileSystemStorage::put_from_path] Read {} bytes, putting to key={}", data.len(), key);
        let object_path = ObjectPath::from(key);
        let payload = object_store::PutPayload::from_bytes(Bytes::from(data));
        self.store.put(&object_path, payload).await?;
        eprintln!("[FileSystemStorage::put_from_path] Done");
        Ok(())
    }
}
