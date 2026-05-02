package main

import (
	"context"
	"errors"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	cmsv1 "github.com/velopulent/cms/apps/backend-go/internal/adapters/grpc"
	httpadapter "github.com/velopulent/cms/apps/backend-go/internal/adapters/http"
	"github.com/velopulent/cms/apps/backend-go/internal/adapters/repository"
	"github.com/velopulent/cms/apps/backend-go/internal/application"
	"github.com/velopulent/cms/apps/backend-go/internal/config"
	"github.com/velopulent/cms/apps/backend-go/internal/ports"
	"github.com/velopulent/cms/apps/backend-go/internal/utils"
)

func main() {
	cfg := config.FromEnv()

	db, err := repository.OpenDatabase(cfg)
	if err != nil {
		log.Fatalf("failed to initialize database: %v", err)
	}
	defer db.DB.Close()

	if err := repository.RunMigrations(db); err != nil {
		log.Fatalf("failed to run migrations: %v", err)
	}

	userRepo := repository.NewSQLiteUserRepository(db.DB)
	if err := repository.SeedAdmin(context.Background(), userRepo); err != nil {
		log.Fatalf("failed to seed admin user: %v", err)
	}

	storage := map[string]ports.StorageProvider{}
	if cfg.StorageFSPath != "" {
		fs, err := repository.NewFileSystemStorage(cfg.StorageFSPath)
		if err != nil {
			log.Printf("failed to initialize filesystem storage: %v", err)
		} else {
			storage["filesystem"] = fs
		}
	}
	if cfg.HasS3() {
		storage["s3"] = repository.NewS3Storage(cfg.S3PublicURL)
	}
	if len(storage) == 0 {
		fs, _ := repository.NewFileSystemStorage("uploads")
		storage["filesystem"] = fs
	}

	services := application.NewServices(
		cfg,
		userRepo,
		repository.NewSQLiteSiteRepository(db.DB),
		repository.NewSQLiteCollectionRepository(db.DB),
		repository.NewSQLiteEntryRepository(db.DB),
		repository.NewSQLiteFileRepository(db.DB),
		repository.NewSQLiteAccessTokenRepository(db.DB),
		utils.NewJWTSigner(cfg.JWTSecret),
		storage,
	)
	server := httpadapter.NewServer(services)

	go func() {
		log.Printf("Go REST API server running on %s", cfg.BindAddress)
		if err := server.Start(cfg.BindAddress); err != nil && !errors.Is(err, http.ErrServerClosed) {
			log.Fatalf("server error: %v", err)
		}
	}()
	ctx, stop := context.WithCancel(context.Background())
	defer stop()
	go func() {
		log.Printf("Go gRPC server running on %s", cfg.GRPCBindAddress)
		if err := cmsv1.Start(ctx, cfg.GRPCBindAddress, services); err != nil {
			log.Printf("gRPC server error: %v", err)
		}
	}()

	quit := make(chan os.Signal, 1)
	signal.Notify(quit, os.Interrupt, syscall.SIGTERM)
	<-quit
	stop()

	shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	if err := server.Shutdown(shutdownCtx); err != nil {
		log.Fatalf("server shutdown failed: %v", err)
	}
}
