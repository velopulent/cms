package repository

import (
	"database/sql"
	"fmt"
	"strings"
	"time"

	_ "github.com/go-sql-driver/mysql"
	_ "github.com/jackc/pgx/v5/stdlib"
	_ "github.com/mattn/go-sqlite3"

	"github.com/velopulent/cms/apps/backend-go/internal/config"
)

type Database struct {
	DB      *sql.DB
	Driver  string
	DSN     string
	Backend string
}

func OpenDatabase(cfg config.Config) (*Database, error) {
	driver, dsn, backend, err := driverAndDSN(cfg.DatabaseURL)
	if err != nil {
		return nil, err
	}

	db, err := sql.Open(driver, dsn)
	if err != nil {
		return nil, fmt.Errorf("open database: %w", err)
	}

	db.SetMaxOpenConns(cfg.DBMaxConnections)
	db.SetMaxIdleConns(cfg.DBMinConnections)
	db.SetConnMaxIdleTime(time.Duration(cfg.DBIdleTimeoutSecs) * time.Second)

	if err := db.Ping(); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("ping database: %w", err)
	}

	return &Database{DB: db, Driver: driver, DSN: dsn, Backend: backend}, nil
}

func driverAndDSN(url string) (driver string, dsn string, backend string, err error) {
	switch {
	case strings.HasPrefix(url, "sqlite:"):
		return "sqlite3", sqliteDSN(url), "sqlite", nil
	case strings.HasPrefix(url, "postgres://"), strings.HasPrefix(url, "postgresql://"):
		return "pgx", url, "postgres", nil
	case strings.HasPrefix(url, "mysql://"):
		return "mysql", strings.TrimPrefix(url, "mysql://"), "mysql", nil
	default:
		return "", "", "", fmt.Errorf("unknown database URL scheme")
	}
}

func sqliteDSN(url string) string {
	path := strings.TrimPrefix(url, "sqlite:")
	if path == "" {
		path = "cms.db"
	}
	if path == ":memory:" || path == "memory:" {
		return ":memory:"
	}
	return path + "?_journal_mode=WAL&_busy_timeout=30000&_foreign_keys=on"
}

func RunMigrations(db *Database) error {
	if db.Backend != "sqlite" {
		return fmt.Errorf("%s migrations are planned after the SQLite auth slice", db.Backend)
	}
	if _, err := db.DB.Exec(sqliteSchema); err != nil {
		return fmt.Errorf("run sqlite migrations: %w", err)
	}
	return nil
}
