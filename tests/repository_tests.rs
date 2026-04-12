use cms::database::pool::DbPool;
use cms::models::access_token::AccessTokenKind;
use cms::repository::Repository;
use cms::repository::error::RepositoryError;
use cms::repository::traits::ListEntriesParams;
use sqlx::sqlite::SqlitePoolOptions;

async fn setup_test_db() -> (DbPool, Repository) {
    let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await.unwrap();
    let db_pool = DbPool::Sqlite(pool);
    let schema = include_str!("../src/database/schema/sqlite.sql");
    for statement in schema.split(';').filter(|s| !s.trim().is_empty()) {
        sqlx::query(statement)
            .execute(match &db_pool {
                DbPool::Sqlite(p) => p,
                _ => unreachable!(),
            })
            .await
            .unwrap();
    }
    let repo = Repository::new(&db_pool);
    (db_pool, repo)
}

#[tokio::test]
async fn test_user_crud() {
    let (_pool, repo) = setup_test_db().await;

    repo.user
        .create("u1", "alice", "alice@test.com", "hash1")
        .await
        .unwrap();
    repo.user.create("u2", "bob", "bob@test.com", "hash2").await.unwrap();

    let found = repo.user.find_by_username("alice").await.unwrap().unwrap();
    assert_eq!(found.username, "alice");
    assert_eq!(found.email, "alice@test.com");

    let by_id = repo.user.find_by_id("u1").await.unwrap().unwrap();
    assert_eq!(by_id.username, "alice");

    assert!(repo.user.exists("alice").await.unwrap());
    assert!(!repo.user.exists("nobody").await.unwrap());

    let none = repo.user.find_by_username("nobody").await.unwrap();
    assert!(none.is_none());

    let id_for_username = repo.user.find_id_by_username("bob").await.unwrap();
    assert_eq!(id_for_username, Some("u2".to_string()));
}

#[tokio::test]
async fn test_user_unique_constraints() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "hash").await.unwrap();

    let dup_username = repo.user.create("u2", "alice", "other@test.com", "hash").await;
    assert!(matches!(dup_username, Err(RepositoryError::UniqueViolation(_))));

    let dup_email = repo.user.create("u3", "other", "alice@test.com", "hash").await;
    assert!(matches!(dup_email, Err(RepositoryError::UniqueViolation(_))));
}

#[tokio::test]
async fn test_user_roles() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "hash").await.unwrap();
    repo.site.create("s1", "Test Site", "filesystem", "u1").await.unwrap();

    let role = repo.user.get_role("u1", "s1").await.unwrap();
    assert_eq!(role, Some("owner".to_string()));

    let no_role = repo.user.get_role("u1", "nonexistent").await.unwrap();
    assert!(no_role.is_none());
}

#[tokio::test]
async fn test_site_crud() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "hash").await.unwrap();

    let site = repo.site.create("s1", "My Site", "filesystem", "u1").await.unwrap();
    assert_eq!(site.name, "My Site");
    assert_eq!(site.default_storage_provider, "filesystem");

    let got = repo.site.get_by_id("s1").await.unwrap().unwrap();
    assert_eq!(got.name, "My Site");

    let updated = repo.site.update("s1", "Updated", "s3").await.unwrap();
    assert_eq!(updated.name, "Updated");
    assert_eq!(updated.default_storage_provider, "s3");

    assert_eq!(repo.site.delete("s1").await.unwrap(), 1);
    assert!(repo.site.get_by_id("s1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_site_memberships() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "owner", "owner@test.com", "h").await.unwrap();
    repo.user.create("u2", "editor", "editor@test.com", "h").await.unwrap();

    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();
    repo.site.add_member("m2", "s1", "u2", "editor").await.unwrap();

    let members = repo.site.list_members("s1").await.unwrap();
    assert_eq!(members.len(), 2);

    let dup = repo.site.add_member("m3", "s1", "u2", "viewer").await;
    assert!(matches!(dup, Err(RepositoryError::UniqueViolation(_))));

    repo.site.update_member_role("s1", "u2", "admin").await.unwrap();
    let members = repo.site.list_members("s1").await.unwrap();
    assert!(members.iter().any(|m| m.user_id == "u2" && m.role == "admin"));

    assert_eq!(repo.site.remove_member("s1", "u2").await.unwrap(), 1);
    assert_eq!(repo.site.list_members("s1").await.unwrap().len(), 1);
}

#[tokio::test]
async fn test_site_list_for_user() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site A", "filesystem", "u1").await.unwrap();

    let sites = repo.site.list_for_user("u1").await.unwrap();
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].role, "owner");

    let empty = repo.site.list_for_user("nobody").await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_collection_crud() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    let col = repo
        .collection
        .create("c1", "s1", "Posts", "posts", r#"{"fields":[]}"#, false)
        .await
        .unwrap();
    assert_eq!(col.name, "Posts");
    assert!(!col.is_singleton);

    let by_slug = repo.collection.get_by_slug("s1", "posts").await.unwrap().unwrap();
    assert_eq!(by_slug.id, "c1");

    let list = repo.collection.list("s1").await.unwrap();
    assert_eq!(list.len(), 1);

    let updated = repo
        .collection
        .update("c1", "Articles", "articles", r#"{"fields":[{"name":"title"}]}"#)
        .await
        .unwrap();
    assert_eq!(updated.name, "Articles");
    assert_eq!(updated.slug, "articles");

    assert_eq!(repo.collection.delete("s1", "articles").await.unwrap(), 1);
    assert!(repo.collection.get_by_slug("s1", "articles").await.unwrap().is_none());
}

#[tokio::test]
async fn test_collection_unique_slug() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    repo.collection
        .create("c1", "s1", "Posts", "posts", "{}", false)
        .await
        .unwrap();
    let dup = repo.collection.create("c2", "s1", "Other", "posts", "{}", false).await;
    assert!(matches!(dup, Err(RepositoryError::UniqueViolation(_))));
}

#[tokio::test]
async fn test_singleton_create_and_update_data() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    repo.collection
        .create("c1", "s1", "Settings", "settings", r#"{"fields":[]}"#, true)
        .await
        .unwrap();

    let singletons = repo.collection.list_singletons_only("s1").await.unwrap();
    assert_eq!(singletons.len(), 1);

    repo.collection
        .update_singleton_data("c1", r#"{"title":"Hello"}"#)
        .await
        .unwrap();
    let updated = repo.collection.get_by_id("c1").await.unwrap().unwrap();
    assert!(updated.singleton_data.is_some());
}

#[tokio::test]
async fn test_entry_crud() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();
    repo.collection
        .create("c1", "s1", "Posts", "posts", "{}", false)
        .await
        .unwrap();

    let entry = repo
        .entry
        .create("ct1", "s1", "c1", r#"{"title":"Hello"}"#, "hello")
        .await
        .unwrap();
    assert_eq!(entry.slug, "hello");
    assert_eq!(entry.status, "draft");

    let got = repo.entry.get_by_id("ct1", "s1", false).await.unwrap().unwrap();
    assert_eq!(got.slug, "hello");

    let updated = repo
        .entry
        .update("ct1", r#"{"title":"Updated"}"#, "hello-updated", "draft")
        .await
        .unwrap();
    assert_eq!(updated.slug, "hello-updated");

    assert_eq!(repo.entry.delete("ct1", "s1").await.unwrap(), 1);
    assert!(repo.entry.get_by_id("ct1", "s1", false).await.unwrap().is_none());
}

#[tokio::test]
async fn test_entry_publish_unpublish() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();
    repo.collection
        .create("c1", "s1", "Posts", "posts", "{}", false)
        .await
        .unwrap();
    repo.entry.create("ct1", "s1", "c1", "{}", "hello").await.unwrap();

    let published = repo.entry.publish("ct1", "s1").await.unwrap();
    assert_eq!(published.status, "published");
    assert!(published.published_at.is_some());

    let unpublished = repo.entry.unpublish("ct1", "s1").await.unwrap();
    assert_eq!(unpublished.status, "draft");
}

#[tokio::test]
async fn test_entry_unique_slug_per_collection() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();
    repo.collection
        .create("c1", "s1", "Posts", "posts", "{}", false)
        .await
        .unwrap();

    repo.entry.create("ct1", "s1", "c1", "{}", "hello").await.unwrap();
    let dup = repo.entry.create("ct2", "s1", "c1", "{}", "hello").await;
    assert!(matches!(dup, Err(RepositoryError::UniqueViolation(_))));
}

#[tokio::test]
async fn test_entry_not_found_errors() {
    let (_pool, repo) = setup_test_db().await;

    assert!(
        repo.entry
            .get_by_id("nonexistent", "s1", false)
            .await
            .unwrap()
            .is_none()
    );

    let publish_err = repo.entry.publish("nonexistent", "s1").await;
    assert!(matches!(publish_err, Err(RepositoryError::NotFound)));

    let unpublish_err = repo.entry.unpublish("nonexistent", "s1").await;
    assert!(matches!(unpublish_err, Err(RepositoryError::NotFound)));

    let update_err = repo.entry.update("nonexistent", "{}", "slug", "draft").await;
    assert!(matches!(update_err, Err(RepositoryError::NotFound)));
}

#[tokio::test]
async fn test_entry_list_with_pagination() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();
    repo.collection
        .create("c1", "s1", "Posts", "posts", "{}", false)
        .await
        .unwrap();

    for i in 0..5 {
        repo.entry
            .create(
                &format!("ct{}", i),
                "s1",
                "c1",
                &format!(r#"{{"title":"Post {}"}}"#, i),
                &format!("post-{}", i),
            )
            .await
            .unwrap();
    }

    let params = ListEntriesParams {
        site_id: "s1",
        collection_slug: None,
        collection_id: None,
        status: None,
        search: None,
        published_only: false,
        page: 1,
        per_page: 3,
    };
    let result = repo.entry.list(params).await.unwrap();
    assert_eq!(result.items.len(), 3);
    assert_eq!(result.total, 5);
    assert_eq!(result.page, 1);
    assert_eq!(result.per_page, 3);

    let params_page2 = ListEntriesParams {
        site_id: "s1",
        collection_slug: None,
        collection_id: None,
        status: None,
        search: None,
        published_only: false,
        page: 2,
        per_page: 3,
    };
    let result2 = repo.entry.list(params_page2).await.unwrap();
    assert_eq!(result2.items.len(), 2);
}

#[tokio::test]
async fn test_file_crud() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    let file = repo
        .file
        .create(
            "f1",
            "s1",
            "img.jpg",
            "photo.jpg",
            "image/jpeg",
            2048,
            "filesystem",
            "key1",
            Some("thumb1"),
            Some(800),
            Some(600),
            Some("u1"),
        )
        .await
        .unwrap();
    assert_eq!(file.filename, "img.jpg");
    assert_eq!(file.size, 2048);
    assert_eq!(file.width, Some(800));

    let got = repo.file.get_by_id("f1", "s1").await.unwrap().unwrap();
    assert_eq!(got.original_name, "photo.jpg");

    assert!(repo.file.get_by_id("nonexistent", "s1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_file_soft_delete_restore() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();
    repo.file
        .create(
            "f1",
            "s1",
            "img.jpg",
            "photo.jpg",
            "image/jpeg",
            1024,
            "filesystem",
            "key1",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    assert_eq!(repo.file.soft_delete("f1", "s1").await.unwrap(), 1);
    let deleted_file = repo.file.get_by_id("f1", "s1").await.unwrap();
    assert!(deleted_file.is_none() || deleted_file.unwrap().deleted_at.is_some());

    assert_eq!(repo.file.restore("f1", "s1").await.unwrap(), 1);
}

#[tokio::test]
async fn test_file_batch_operations() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();
    repo.file
        .create(
            "f1",
            "s1",
            "a.jpg",
            "a.jpg",
            "image/jpeg",
            100,
            "filesystem",
            "k1",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    repo.file
        .create(
            "f2",
            "s1",
            "b.jpg",
            "b.jpg",
            "image/jpeg",
            200,
            "filesystem",
            "k2",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let deleted = repo
        .file
        .batch_soft_delete("s1", &["f1".into(), "f2".into()])
        .await
        .unwrap();
    assert_eq!(deleted, 2);

    let restored = repo
        .file
        .batch_restore("s1", &["f1".into(), "f2".into()])
        .await
        .unwrap();
    assert_eq!(restored, 2);

    repo.file
        .batch_soft_delete("s1", &["f1".into(), "f2".into()])
        .await
        .unwrap();
    let perm_deleted = repo
        .file
        .batch_permanent_delete("s1", &["f1".into(), "f2".into()])
        .await
        .unwrap();
    assert_eq!(perm_deleted, 2);
}

#[tokio::test]
async fn test_file_references() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    let refs = repo.file.get_references("nonexistent").await.unwrap();
    assert!(refs.is_empty());

    let storage = repo.file.get_storage_provider("s1").await.unwrap();
    assert_eq!(storage, "filesystem");
}

#[tokio::test]
async fn test_access_token_crud() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    repo.access_token
        .create(
            "t1",
            AccessTokenKind::Site,
            Some("s1"),
            "My Token",
            "hashed_key",
            "cms_site_abc1234567890123",
            "hmac_hash_value",
            "content:read,content:write",
            Some("u1"),
        )
        .await
        .unwrap();

    let tokens = repo
        .access_token
        .list(AccessTokenKind::Site, Some("s1"))
        .await
        .unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].name, "My Token");
    assert_eq!(tokens[0].token_prefix, "cms_site_abc1234567890123");
    assert_eq!(tokens[0].kind, "site");

    assert_eq!(
        repo.access_token
            .delete("t1", AccessTokenKind::Site, Some("s1"))
            .await
            .unwrap(),
        1
    );
    assert!(repo
        .access_token
        .list(AccessTokenKind::Site, Some("s1"))
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn test_access_token_find_by_prefix() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    repo.access_token
        .create(
            "t1",
            AccessTokenKind::Site,
            Some("s1"),
            "Token1",
            "hashed1",
            "cms_site_abc1234567890123",
            "hmac1",
            "content:read",
            Some("u1"),
        )
        .await
        .unwrap();

    let found = repo
        .access_token
        .find_by_prefix("cms_site_abc1234567890123")
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, "t1");
    assert_eq!(found[0].1, "site");
    assert_eq!(found[0].2, Some("s1".to_string()));
    assert_eq!(found[0].3, "hashed1");
    assert_eq!(found[0].4, Some("hmac1".to_string()));

    let empty = repo.access_token.find_by_prefix("cms_nonexist").await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_entry_sync_file_references() {
    let (_pool, repo) = setup_test_db().await;

    repo.user.create("u1", "alice", "alice@test.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();
    repo.collection
        .create("c1", "s1", "Posts", "posts", "{}", false)
        .await
        .unwrap();
    repo.file
        .create(
            "f1",
            "s1",
            "img.jpg",
            "photo.jpg",
            "image/jpeg",
            2048,
            "filesystem",
            "key1",
            None,
            None,
            None,
            Some("u1"),
        )
        .await
        .unwrap();
    repo.entry
        .create("ct1", "s1", "c1", r#"{"image":"/api/files/f1"}"#, "hello")
        .await
        .unwrap();

    let data = serde_json::json!({"image": "/api/files/f1"});
    let result = repo.entry.sync_file_references("ct1", "s1", &data).await;
    assert!(result.is_ok(), "sync_file_references failed: {:?}", result.err());
}
