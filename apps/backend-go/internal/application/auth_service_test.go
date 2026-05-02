package application

import (
	"context"
	"errors"
	"testing"
	"time"

	"golang.org/x/crypto/bcrypt"

	"github.com/velopulent/cms/apps/backend-go/internal/domain"
)

type fakeUsers struct {
	byUsername map[string]domain.User
	byID       map[string]domain.User
	createErr  error
}

func newFakeUsers() *fakeUsers {
	return &fakeUsers{byUsername: map[string]domain.User{}, byID: map[string]domain.User{}}
}

func (f *fakeUsers) FindByUsername(_ context.Context, username string) (*domain.User, error) {
	user, ok := f.byUsername[username]
	if !ok {
		return nil, nil
	}
	return &user, nil
}

func (f *fakeUsers) FindByID(_ context.Context, id string) (*domain.User, error) {
	user, ok := f.byID[id]
	if !ok {
		return nil, nil
	}
	return &user, nil
}

func (f *fakeUsers) FindIDByUsername(_ context.Context, username string) (*string, error) {
	user, ok := f.byUsername[username]
	if !ok {
		return nil, nil
	}
	return &user.ID, nil
}

func (f *fakeUsers) Create(_ context.Context, id, username, email, passwordHash string) error {
	if f.createErr != nil {
		return f.createErr
	}
	user := domain.User{ID: id, Username: username, Email: email, PasswordHash: passwordHash}
	f.byUsername[username] = user
	f.byID[id] = user
	return nil
}

func (f *fakeUsers) Exists(_ context.Context, username string) (bool, error) {
	_, ok := f.byUsername[username]
	return ok, nil
}

func (f *fakeUsers) GetRole(_ context.Context, _, _ string) (*string, error) {
	return nil, nil
}

type fakeSigner struct {
	token string
}

func (f fakeSigner) Create(_ string, _ time.Time) (string, error) {
	return f.token, nil
}

func (f fakeSigner) Verify(token string) (domain.Claims, error) {
	if token != f.token {
		return domain.Claims{}, errors.New("bad token")
	}
	return domain.Claims{Sub: "user-123", Exp: time.Now().Add(time.Hour).Unix()}, nil
}

func TestRegisterValidationParity(t *testing.T) {
	tests := []struct {
		name     string
		username string
		email    string
		password string
		wantErr  error
	}{
		{"empty username", "", "test@example.com", "password123", ErrUsernameRequired},
		{"short username", "ab", "test@example.com", "password123", ErrUsernameTooShort},
		{"empty password", "validuser", "test@example.com", "", ErrPasswordRequired},
		{"short password", "validuser", "test@example.com", "short", ErrPasswordTooShort},
		{"invalid email", "validuser", "invalid-email", "password123", ErrInvalidEmailAddress},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			auth := NewAuthService(newFakeUsers(), fakeSigner{token: "jwt-token"})
			_, _, err := auth.Register(context.Background(), tt.username, tt.email, tt.password)
			if !errors.Is(err, tt.wantErr) {
				t.Fatalf("expected %v, got %v", tt.wantErr, err)
			}
		})
	}
}

func TestRegisterTrimsWhitespace(t *testing.T) {
	users := newFakeUsers()
	auth := NewAuthService(users, fakeSigner{token: "jwt-token"})

	user, token, err := auth.Register(context.Background(), "  newuser  ", "  new@example.com  ", "  password123  ")
	if err != nil {
		t.Fatalf("register: %v", err)
	}
	if token != "jwt-token" {
		t.Fatalf("token mismatch: %q", token)
	}
	if user.Username != "newuser" || user.Email != "new@example.com" {
		t.Fatalf("user not trimmed: %#v", user)
	}
}

func TestRegisterDuplicateUser(t *testing.T) {
	users := newFakeUsers()
	users.createErr = ErrDuplicateUser
	auth := NewAuthService(users, fakeSigner{token: "jwt-token"})

	_, _, err := auth.Register(context.Background(), "newuser", "new@example.com", "password123")
	var appErr AppError
	if !errors.As(err, &appErr) || appErr.Kind != KindConflict || appErr.Error() != "Username or email already exists" {
		t.Fatalf("expected duplicate user conflict, got %T %v", err, err)
	}
}

func TestLoginSuccessAndInvalidCredentials(t *testing.T) {
	passwordHash, err := bcrypt.GenerateFromPassword([]byte("password123"), bcrypt.DefaultCost)
	if err != nil {
		t.Fatalf("hash: %v", err)
	}
	users := newFakeUsers()
	users.byUsername["testuser"] = domain.User{
		ID:           "user-123",
		Username:     "testuser",
		Email:        "test@example.com",
		PasswordHash: string(passwordHash),
	}
	auth := NewAuthService(users, fakeSigner{token: "jwt-token"})

	user, token, err := auth.Login(context.Background(), "testuser", "password123")
	if err != nil {
		t.Fatalf("login: %v", err)
	}
	if user.Username != "testuser" || token != "jwt-token" {
		t.Fatalf("unexpected login response: %#v %q", user, token)
	}

	_, _, err = auth.Login(context.Background(), "testuser", "wrongpassword")
	if !errors.Is(err, ErrInvalidCredentials) {
		t.Fatalf("expected invalid credentials, got %v", err)
	}
}
