use bytes::Bytes;
use cms::storage::{MockStorage, StorageProvider};

#[tokio::test]
async fn test_mock_storage_put_get_delete() {
    let storage = MockStorage::default();
    let data = Bytes::from("Hello, World!");

    storage.put("test.txt", data.clone(), "text/plain").await.unwrap();
    let retrieved = storage.get("test.txt").await.unwrap();
    assert_eq!(retrieved, data);

    storage.delete("test.txt").await.unwrap();
    assert!(storage.get("test.txt").await.is_err());
}

#[tokio::test]
async fn test_mock_storage_url_generation() {
    let storage = MockStorage::default();
    let url = storage.url("s_site/f_file/test.jpg", "file-abc");
    assert_eq!(url, "/mock/file-abc");
}

#[tokio::test]
async fn test_mock_storage_multiple_files() {
    let storage = MockStorage::default();
    let data1 = Bytes::from("File 1 content");
    let data2 = Bytes::from("File 2 content longer");

    storage.put("dir/file1.txt", data1.clone(), "text/plain").await.unwrap();
    storage.put("dir/file2.txt", data2.clone(), "text/plain").await.unwrap();

    let retrieved1 = storage.get("dir/file1.txt").await.unwrap();
    let retrieved2 = storage.get("dir/file2.txt").await.unwrap();

    assert_eq!(retrieved1, data1);
    assert_eq!(retrieved2, data2);
}

#[tokio::test]
async fn test_mock_storage_url_with_various_file_ids() {
    let storage = MockStorage::default();

    assert_eq!(storage.url("key", "abc-123"), "/mock/abc-123");
    assert_eq!(storage.url("s_site/f_file/img.png", "file-uuid"), "/mock/file-uuid");
    assert_eq!(storage.url("nested/path/file.jpg", "id"), "/mock/id");
}
