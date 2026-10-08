use cms::grpc::cms::v1::file_service_client::FileServiceClient;
use cms::grpc::cms::v1::{DeleteFileRequest, GetFileRequest, ListFilesRequest, RestoreFileRequest};

use crate::common::{GrpcTestContext, grpc::auth_interceptor};

async fn setup() -> (GrpcTestContext, String, String) {
    let ctx = GrpcTestContext::start().await;
    let (site_id, token) = ctx.setup_site_and_token().await;
    (ctx, site_id, token)
}

async fn seed_file(ctx: &GrpcTestContext, site_id: &str, name: &str) -> String {
    let body = ctx
        .upload_file(site_id, &format!("{}.txt", name), b"test content", "text/plain")
        .await;
    body["id"].as_str().unwrap().to_string()
}

/// Minimal bytes carrying the right magic signature — uploads are now sniffed
/// against the declared mime type, so plain text can't pose as png/jpeg/pdf.
fn body_for_mime(mime_type: &str) -> Vec<u8> {
    match mime_type {
        "image/png" => vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0],
        "image/jpeg" => vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0],
        "application/pdf" => b"%PDF-1.4 test".to_vec(),
        _ => b"test content".to_vec(),
    }
}

async fn seed_file_with_mime(ctx: &GrpcTestContext, site_id: &str, name: &str, mime_type: &str) -> String {
    let ext = mime_type.split('/').next_back().unwrap_or("bin");
    let body = ctx
        .upload_file(
            site_id,
            &format!("{}.{}", name, ext),
            &body_for_mime(mime_type),
            mime_type,
        )
        .await;
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_list_files() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    seed_file(&ctx, &site_id, "file1").await;
    seed_file(&ctx, &site_id, "file2").await;

    let resp = client
        .list_files(tonic::Request::new(ListFilesRequest {
            search: None,
            file_type: None,
            page: 1,
            per_page: 10,
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.files.len(), 2);
    assert_eq!(resp.total, 2);
}

#[tokio::test]
async fn test_list_files_default_pagination() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    seed_file(&ctx, &site_id, "file1").await;
    seed_file(&ctx, &site_id, "file2").await;

    let resp = client
        .list_files(tonic::Request::new(ListFilesRequest {
            search: None,
            file_type: None,
            page: 0,
            per_page: 0,
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.files.len(), 2);
    assert_eq!(resp.total, 2);
    assert!(resp.page >= 1);
    assert!(resp.per_page >= 1);
}

#[tokio::test]
async fn test_list_files_filter_by_category() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    seed_file_with_mime(&ctx, &site_id, "photo", "image/png").await;
    seed_file_with_mime(&ctx, &site_id, "banner", "image/jpeg").await;
    seed_file_with_mime(&ctx, &site_id, "doc", "application/pdf").await;

    let resp = client
        .list_files(tonic::Request::new(ListFilesRequest {
            search: None,
            file_type: Some("image".into()),
            page: 1,
            per_page: 10,
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.files.len(), 2);
    assert_eq!(resp.total, 2);
    let types: Vec<&str> = resp.files.iter().map(|f| f.mime_type.as_str()).collect();
    assert!(types.iter().all(|t| t.starts_with("image/")));
}

#[tokio::test]
async fn test_list_files_filter_by_exact_mime_type() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    seed_file_with_mime(&ctx, &site_id, "photo", "image/png").await;
    seed_file_with_mime(&ctx, &site_id, "banner", "image/jpeg").await;
    seed_file_with_mime(&ctx, &site_id, "doc", "application/pdf").await;

    let resp = client
        .list_files(tonic::Request::new(ListFilesRequest {
            search: None,
            file_type: Some("image/png".into()),
            page: 1,
            per_page: 10,
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.files.len(), 1);
    assert_eq!(resp.total, 1);
    assert_eq!(resp.files[0].mime_type, "image/png");
}

#[tokio::test]
async fn test_get_file() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let file_id = seed_file(&ctx, &site_id, "get-me").await;

    let resp = client
        .get_file(tonic::Request::new(GetFileRequest {
            id: file_id.clone(),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.id, file_id);
    assert!(resp.original_name.contains("get-me"));
    assert_eq!(resp.mime_type, "text/plain");
}

#[tokio::test]
async fn test_delete_file() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let file_id = seed_file(&ctx, &site_id, "delete-me").await;

    let resp = client
        .delete_file(tonic::Request::new(DeleteFileRequest {
            id: file_id.clone(),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.success);

    let list_resp = client
        .list_files(tonic::Request::new(ListFilesRequest {
            search: None,
            file_type: None,
            page: 1,
            per_page: 100,
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    let remaining: Vec<&str> = list_resp.files.iter().map(|f| f.id.as_str()).collect();
    assert!(
        !remaining.contains(&file_id.as_str()),
        "Deleted file should not appear in list"
    );
}

#[tokio::test]
async fn test_restore_file() {
    let (ctx, site_id, token) = setup().await;
    let channel = ctx.connect().await;
    let mut client = FileServiceClient::with_interceptor(channel, auth_interceptor(&token));

    let file_id = seed_file(&ctx, &site_id, "restore-me").await;

    let _deleted = client
        .delete_file(tonic::Request::new(DeleteFileRequest {
            id: file_id.clone(),
            ..Default::default()
        }))
        .await
        .unwrap();

    let resp = client
        .restore_file(tonic::Request::new(RestoreFileRequest {
            id: file_id.clone(),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.id, file_id);
    assert!(resp.original_name.contains("restore-me"));
}

#[tokio::test]
async fn streaming_upload_preserves_bytes_and_rejects_changed_metadata() {
    use cms::grpc::cms::v1::UploadFileRequest;
    let (ctx, site_id, token) = setup().await;
    let mut client = FileServiceClient::with_interceptor(ctx.connect().await, auth_interceptor(&token));
    let metadata = UploadFileRequest {
        site_id: site_id.clone(),
        filename: "test.txt".into(),
        content_type: "text/plain".into(),
        chunk: b"hello ".to_vec(),
    };
    let file = client
        .upload_file(tokio_stream::iter(vec![
            metadata.clone(),
            UploadFileRequest {
                chunk: b"world".to_vec(),
                ..Default::default()
            },
        ]))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(file.size, 11);
    let response = reqwest::get(format!("{}/api/files/{}", ctx.rest_base_url, file.id))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "hello world");
    let error = client
        .upload_file(tokio_stream::iter(vec![
            metadata,
            UploadFileRequest {
                site_id: "another-site".into(),
                chunk: b"bad".to_vec(),
                ..Default::default()
            },
        ]))
        .await
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::InvalidArgument);
    let files = client
        .list_files(ListFilesRequest {
            site_id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(files.total, 1, "Rejected upload left a file record");
}
