package ports

import (
	"time"

	"github.com/velopulent/cms/apps/backend-go/internal/domain"
)

type TokenSigner interface {
	Create(userID string, now time.Time) (string, error)
	Verify(token string) (domain.Claims, error)
}
