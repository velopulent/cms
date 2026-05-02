package repository

import (
	"context"
	"database/sql"
	"errors"
	"testing"

	_ "github.com/mattn/go-sqlite3"

	"github.com/velopulent/cms/apps/backend-go/internal/application"
)

func testSQLiteDB(t *testing.T) *sql.DB {
	t.Helper()

	db, err := sql.Open("sqlite3", ":memory:")
	if err != nil {
		t.Fatalf("open sqlite: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	if _, err := db.Exec(sqliteSchema); err != nil {
		t.Fatalf("migrate: %v", err)
	}
	return db
}

func TestSQLiteUserRepositoryCreateFindAndDuplicate(t *testing.T) {
	ctx := context.Background()
	repo := NewSQLiteUserRepository(testSQLiteDB(t))

	if err := repo.Create(ctx, "user-123", "testuser", "test@example.com", "hash"); err != nil {
		t.Fatalf("create: %v", err)
	}

	user, err := repo.FindByUsername(ctx, "testuser")
	if err != nil {
		t.Fatalf("find by username: %v", err)
	}
	if user == nil || user.ID != "user-123" || user.PasswordHash != "hash" {
		t.Fatalf("unexpected user: %#v", user)
	}

	exists, err := repo.Exists(ctx, "testuser")
	if err != nil {
		t.Fatalf("exists: %v", err)
	}
	if !exists {
		t.Fatal("expected user to exist")
	}

	if err := repo.Create(ctx, "user-456", "testuser", "other@example.com", "hash"); !errors.Is(err, application.ErrDuplicateUser) {
		t.Fatalf("expected duplicate user error, got %v", err)
	}
}

func TestSeedAdmin(t *testing.T) {
	ctx := context.Background()
	repo := NewSQLiteUserRepository(testSQLiteDB(t))

	if err := SeedAdmin(ctx, repo); err != nil {
		t.Fatalf("seed admin: %v", err)
	}
	if err := SeedAdmin(ctx, repo); err != nil {
		t.Fatalf("seed admin second time: %v", err)
	}

	user, err := repo.FindByUsername(ctx, "admin")
	if err != nil {
		t.Fatalf("find admin: %v", err)
	}
	if user == nil || user.Email != "admin@cms.local" {
		t.Fatalf("unexpected admin user: %#v", user)
	}
}
