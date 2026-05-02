package application

import (
	"context"
	"errors"
	"fmt"
	"regexp"
	"strings"
	"time"

	"github.com/google/uuid"
	"golang.org/x/crypto/bcrypt"

	"github.com/velopulent/cms/apps/backend-go/internal/domain"
	"github.com/velopulent/cms/apps/backend-go/internal/ports"
)

var emailRE = regexp.MustCompile(`^[^@\s]+@[^@\s]+\.[^@\s]+$`)

type AuthService struct {
	users ports.UserRepository
	jwt   ports.TokenSigner
	now   func() time.Time
}

func NewAuthService(users ports.UserRepository, jwt ports.TokenSigner) *AuthService {
	return &AuthService{
		users: users,
		jwt:   jwt,
		now:   time.Now,
	}
}

func (s *AuthService) Register(ctx context.Context, username, email, password string) (domain.UserPublic, string, error) {
	username = strings.TrimSpace(username)
	email = strings.TrimSpace(email)
	password = strings.TrimSpace(password)

	if username == "" {
		return domain.UserPublic{}, "", Validation(ErrUsernameRequired)
	}
	if len(username) < 3 {
		return domain.UserPublic{}, "", Validation(ErrUsernameTooShort)
	}
	if password == "" {
		return domain.UserPublic{}, "", Validation(ErrPasswordRequired)
	}
	if len(password) < 8 {
		return domain.UserPublic{}, "", Validation(ErrPasswordTooShort)
	}
	if !emailRE.MatchString(email) {
		return domain.UserPublic{}, "", Validation(ErrInvalidEmailAddress)
	}

	passwordHash, err := bcrypt.GenerateFromPassword([]byte(password), bcrypt.DefaultCost)
	if err != nil {
		return domain.UserPublic{}, "", Internal(fmt.Errorf("hash password: %w", err))
	}

	id, err := uuid.NewV7()
	if err != nil {
		return domain.UserPublic{}, "", Internal(fmt.Errorf("generate user id: %w", err))
	}

	if err := s.users.Create(ctx, id.String(), username, email, string(passwordHash)); err != nil {
		if errors.Is(err, ErrDuplicateUser) {
			return domain.UserPublic{}, "", Conflict(errors.New("Username or email already exists"))
		}
		return domain.UserPublic{}, "", Internal(fmt.Errorf("create user: %w", err))
	}

	user := domain.UserPublic{ID: id.String(), Username: username, Email: email}
	token, err := s.jwt.Create(user.ID, s.now())
	if err != nil {
		return domain.UserPublic{}, "", Internal(fmt.Errorf("create token: %w", err))
	}

	return user, token, nil
}

func (s *AuthService) Login(ctx context.Context, username, password string) (domain.UserPublic, string, error) {
	user, err := s.users.FindByUsername(ctx, username)
	if err != nil {
		return domain.UserPublic{}, "", Internal(fmt.Errorf("find user: %w", err))
	}
	if user == nil {
		return domain.UserPublic{}, "", Unauthorized(ErrInvalidCredentials)
	}

	if err := bcrypt.CompareHashAndPassword([]byte(user.PasswordHash), []byte(password)); err != nil {
		return domain.UserPublic{}, "", Unauthorized(ErrInvalidCredentials)
	}

	token, err := s.jwt.Create(user.ID, s.now())
	if err != nil {
		return domain.UserPublic{}, "", Internal(fmt.Errorf("create token: %w", err))
	}

	return domain.UserPublic{ID: user.ID, Username: user.Username, Email: user.Email}, token, nil
}

func (s *AuthService) GetUser(ctx context.Context, userID string) (*domain.UserPublic, error) {
	user, err := s.users.FindByID(ctx, userID)
	if err != nil {
		return nil, Internal(fmt.Errorf("find user by id: %w", err))
	}
	if user == nil {
		return nil, nil
	}

	return &domain.UserPublic{ID: user.ID, Username: user.Username, Email: user.Email}, nil
}

func (s *AuthService) VerifyToken(token string) (domain.Claims, error) {
	claims, err := s.jwt.Verify(token)
	if err != nil {
		return domain.Claims{}, Unauthorized(errors.New("Invalid or expired token"))
	}
	return claims, nil
}
