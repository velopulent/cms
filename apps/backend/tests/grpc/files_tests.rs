use cms::grpc::cms::v1::file_service_client::FileServiceClient;
use cms::grpc::cms::v1::{
    BatchDeleteFilesRequest, BatchRestoreFilesRequest, DeleteFileRequest, GetFileRequest, ListFilesRequest,
    RestoreFileRequest,
};

use crate::common::{GrpcTestContext, auth_interceptor, seed_access_token, seed_site};

async fn setup() -> (GrpcTestContext, String, String) {
    let ctx = GrpcTestContext::start().await;
    let site_id = seed_site(&ctx.repository, "Test Site", &ctx.admin_user_id).await;
    let token = seed_access_token(&ctx.repository, &site_id, "write").await;
    (ctx, site_id, token)
}

async fn seed_file(repo: &cms::repository::Repository, site_id: &str, name: &str) -> String {
    let id = uuid::Uuid::now_v7().to_string();
    repo.file
        .create(
            &id,
            site_id,
            &format!("{}.txt", name),
            &format!("{}.txt", name),
            "text/plain",
            100,
            "filesystem",
            &format!("s_{}/f_{}/{}.txt", site_id, id, name),
            None,
            None,
            None,
            None,
        )
        .await
        .expect("Failed to seed file");
    id
}

#[tokio::test]
async fn test_list_files() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    seed_file(&ctx.repository, &site_id, "file1").await;
    seed_file(&ctx.repository, &site_id, "file2").await;

    let resp = client
        .list_files(tonic::Request::new(ListFilesRequest {
            search: None,
            file_type: None,
            page: 1,
            per_page: 10,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.files.len(), 2);
    assert_eq!(resp.total, 2);
}

#[tokio::test]
async fn test_get_file() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let file_id = seed_file(&ctx.repository, &site_id, "get-me").await;

    let resp = client
        .get_file(tonic::Request::new(GetFileRequest { id: file_id.clone() }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.id, file_id);
    assert_eq!(resp.original_name, "get-me.txt");
    assert_eq!(resp.mime_type, "text/plain");
}

#[tokio::test]
async fn test_delete_file() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let file_id = seed_file(&ctx.repository, &site_id, "delete-me").await;

    let resp = client
        .delete_file(tonic::Request::new(DeleteFileRequest { id: file_id.clone() }))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.success);
}

#[tokio::test]
async fn test_restore_file() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let file_id = seed_file(&ctx.repository, &site_id, "restore-me").await;

    client
        .delete_file(tonic::Request::new(DeleteFileRequest { id: file_id.clone() }))
        .await
        .unwrap();

    let resp = client
        .restore_file(tonic::Request::new(RestoreFileRequest { id: file_id.clone() }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.id, file_id);
    assert_eq!(resp.original_name, "restore-me.txt");
}

#[tokio::test]
async fn test_batch_delete_files() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let id1 = seed_file(&ctx.repository, &site_id, "batch1").await;
    let id2 = seed_file(&ctx.repository, &site_id, "batch2").await;

    let resp = client
        .batch_delete_files(tonic::Request::new(BatchDeleteFilesRequest { ids: vec![id1, id2] }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.affected, 2);
}

#[tokio::test]
async fn test_batch_restore_files() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let id1 = seed_file(&ctx.repository, &site_id, "batch-r1").await;
    let id2 = seed_file(&ctx.repository, &site_id, "batch-r2").await;

    client
        .batch_delete_files(tonic::Request::new(BatchDeleteFilesRequest {
            ids: vec![id1.clone(), id2.clone()],
        }))
        .await
        .unwrap();

    let resp = client
        .batch_restore_files(tonic::Request::new(BatchRestoreFilesRequest { ids: vec![id1, id2] }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.affected, 2);
}
