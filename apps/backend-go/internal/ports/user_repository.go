package ports

import (
	"context"

	"github.com/velopulent/cms/apps/backend-go/internal/domain"
)

type UserRepository interface {
	FindByUsername(ctx context.Context, username string) (*domain.User, error)
	FindByID(ctx context.Context, id string) (*domain.User, error)
	FindIDByUsername(ctx context.Context, username string) (*string, error)
	Create(ctx context.Context, id, username, email, passwordHash string) error
	Exists(ctx context.Context, username string) (bool, error)
	GetRole(ctx context.Context, userID, siteID string) (*string, error)
}
