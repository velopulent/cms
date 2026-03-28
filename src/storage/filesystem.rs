use bytes::Bytes;
use object_store::ObjectStoreExt;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use std::sync::Arc;

#[derive(Clone)]
pub struct FileSystemStorage {
    store: Arc<LocalFileSystem>,
    base_url: String,
}

impl FileSystemStorage {
    pub fn new(root_path: &str, base_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(root_path)?;
        let store = LocalFileSystem::new_with_prefix(root_path)?;
        Ok(Self {
            store: Arc::new(store),
            base_url: base_url.to_string(),
        })
    }

    pub async fn put(&self, key: &str, data: &[u8], _content_type: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = ObjectPath::from(key);
        let payload = object_store::PutPayload::from_bytes(Bytes::copy_from_slice(data));
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

    pub fn url(&self, key: &str) -> String {
        format!("{}/{}", self.base_url, key)
    }
}
