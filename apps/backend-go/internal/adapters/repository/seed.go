package repository

import (
	"context"
	"fmt"

	"github.com/google/uuid"
	"golang.org/x/crypto/bcrypt"

	"github.com/velopulent/cms/apps/backend-go/internal/ports"
)

func SeedAdmin(ctx context.Context, users ports.UserRepository) error {
	exists, err := users.Exists(ctx, "admin")
	if err != nil {
		return fmt.Errorf("check admin user: %w", err)
	}
	if exists {
		return nil
	}

	id, err := uuid.NewV7()
	if err != nil {
		return fmt.Errorf("generate admin id: %w", err)
	}
	passwordHash, err := bcrypt.GenerateFromPassword([]byte("admin"), bcrypt.DefaultCost)
	if err != nil {
		return fmt.Errorf("hash admin password: %w", err)
	}

	if err := users.Create(ctx, id.String(), "admin", "admin@cms.local", string(passwordHash)); err != nil {
		return fmt.Errorf("seed admin user: %w", err)
	}
	return nil
}
