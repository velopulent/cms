use bytes::Bytes;
use object_store::aws::AmazonS3Builder;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, ObjectStoreExt};
use std::sync::Arc;

#[derive(Clone)]
pub struct S3Storage {
    store: Arc<dyn ObjectStore>,
    public_url: Option<String>,
}

impl S3Storage {
    pub fn new(
        access_key_id: &str,
        secret_access_key: &str,
        bucket: &str,
        region: &str,
        endpoint: Option<&str>,
        public_url: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(bucket)
            .with_region(region)
            .with_access_key_id(access_key_id)
            .with_secret_access_key(secret_access_key);

        if let Some(ep) = endpoint {
            builder = builder.with_endpoint(ep);
        }

        let store = builder.build()?;

        Ok(Self {
            store: Arc::new(store),
            public_url: public_url.map(String::from),
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
        Ok(())
    }

    pub fn url(&self, key: &str) -> String {
        match &self.public_url {
            Some(base) => format!("{}/{}", base.trim_end_matches('/'), key),
            None => format!("/api/files?key={}", key),
        }
    }

}
