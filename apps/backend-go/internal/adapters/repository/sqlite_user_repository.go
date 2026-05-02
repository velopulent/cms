package repository

import (
	"context"
	"database/sql"
	"errors"
	"strings"

	"github.com/mattn/go-sqlite3"

	"github.com/velopulent/cms/apps/backend-go/internal/application"
	"github.com/velopulent/cms/apps/backend-go/internal/domain"
)

type SQLiteUserRepository struct {
	db *sql.DB
}

func NewSQLiteUserRepository(db *sql.DB) *SQLiteUserRepository {
	return &SQLiteUserRepository{db: db}
}

func (r *SQLiteUserRepository) FindByUsername(ctx context.Context, username string) (*domain.User, error) {
	return r.findOne(ctx, "SELECT id, username, email, password_hash, created_at, updated_at FROM users WHERE username = ?", username)
}

func (r *SQLiteUserRepository) FindByID(ctx context.Context, id string) (*domain.User, error) {
	return r.findOne(ctx, "SELECT id, username, email, password_hash, created_at, updated_at FROM users WHERE id = ?", id)
}

func (r *SQLiteUserRepository) FindIDByUsername(ctx context.Context, username string) (*string, error) {
	var id string
	err := r.db.QueryRowContext(ctx, "SELECT id FROM users WHERE username = ?", username).Scan(&id)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &id, nil
}

func (r *SQLiteUserRepository) Create(ctx context.Context, id, username, email, passwordHash string) error {
	_, err := r.db.ExecContext(
		ctx,
		"INSERT INTO users (id, username, email, password_hash) VALUES (?, ?, ?, ?)",
		id,
		username,
		email,
		passwordHash,
	)
	if err != nil {
		if isSQLiteUniqueViolation(err) {
			return application.ErrDuplicateUser
		}
		return err
	}
	return nil
}

func (r *SQLiteUserRepository) Exists(ctx context.Context, username string) (bool, error) {
	var id string
	err := r.db.QueryRowContext(ctx, "SELECT id FROM users WHERE username = ?", username).Scan(&id)
	if errors.Is(err, sql.ErrNoRows) {
		return false, nil
	}
	if err != nil {
		return false, err
	}
	return true, nil
}

func (r *SQLiteUserRepository) GetRole(ctx context.Context, userID, siteID string) (*string, error) {
	var role string
	err := r.db.QueryRowContext(ctx, "SELECT sm.role FROM site_members sm WHERE sm.user_id = ? AND sm.site_id = ?", userID, siteID).Scan(&role)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &role, nil
}

func (r *SQLiteUserRepository) findOne(ctx context.Context, query string, arg string) (*domain.User, error) {
	var user domain.User
	err := r.db.QueryRowContext(ctx, query, arg).Scan(
		&user.ID,
		&user.Username,
		&user.Email,
		&user.PasswordHash,
		&user.CreatedAt,
		&user.UpdatedAt,
	)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &user, nil
}

func isSQLiteUniqueViolation(err error) bool {
	var sqliteErr sqlite3.Error
	if errors.As(err, &sqliteErr) && sqliteErr.ExtendedCode == sqlite3.ErrConstraintUnique {
		return true
	}
	return strings.Contains(strings.ToLower(err.Error()), "unique constraint failed")
}
