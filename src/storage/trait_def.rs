use async_trait::async_trait;
use bytes::Bytes;

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn put(&self, key: &str, data: &[u8], content_type: &str) -> Result<(), Box<dyn std::error::Error>>;
    async fn get(&self, key: &str) -> Result<Bytes, Box<dyn std::error::Error>>;
    async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn url(&self, key: &str) -> String;
    fn provider_name(&self) -> &str;
}
