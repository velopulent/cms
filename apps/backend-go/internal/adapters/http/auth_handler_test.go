package httpadapter

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/velopulent/cms/apps/backend-go/internal/application"
	"github.com/velopulent/cms/apps/backend-go/internal/domain"
)

func newTestServer() *httptest.Server {
	users := newHTTPFakeUsers()
	auth := application.NewAuthService(users, httpFakeSigner{token: "jwt-token", userID: "user-123"})
	server := NewServer(auth, false)
	return httptest.NewServer(server)
}

type httpFakeUsers struct {
	users map[string]domain.User
}

func newHTTPFakeUsers() *httpFakeUsers {
	return &httpFakeUsers{users: map[string]domain.User{}}
}

func (f *httpFakeUsers) FindByUsername(_ context.Context, username string) (*domain.User, error) {
	user, ok := f.users[username]
	if !ok {
		return nil, nil
	}
	return &user, nil
}

func (f *httpFakeUsers) FindByID(_ context.Context, id string) (*domain.User, error) {
	for _, user := range f.users {
		if user.ID == id {
			return &user, nil
		}
	}
	return nil, nil
}

func (f *httpFakeUsers) FindIDByUsername(_ context.Context, username string) (*string, error) {
	user, ok := f.users[username]
	if !ok {
		return nil, nil
	}
	return &user.ID, nil
}

func (f *httpFakeUsers) Create(_ context.Context, id, username, email, passwordHash string) error {
	f.users[username] = domain.User{ID: id, Username: username, Email: email, PasswordHash: passwordHash}
	return nil
}

func (f *httpFakeUsers) Exists(_ context.Context, username string) (bool, error) {
	_, ok := f.users[username]
	return ok, nil
}

func (f *httpFakeUsers) GetRole(_ context.Context, _, _ string) (*string, error) {
	return nil, nil
}

type httpFakeSigner struct {
	token  string
	userID string
}

func (f httpFakeSigner) Create(_ string, _ time.Time) (string, error) {
	return f.token, nil
}

func (f httpFakeSigner) Verify(token string) (domain.Claims, error) {
	if token != f.token {
		return domain.Claims{}, application.ErrInvalidCredentials
	}
	return domain.Claims{Sub: f.userID, Exp: 9999999999}, nil
}

func TestRegisterHandlerParity(t *testing.T) {
	server := newTestServer()
	defer server.Close()

	resp, body := postJSON(t, server.URL+"/api/auth/register", `{"username":"newuser","email":"new@example.com","password":"password123"}`)
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusCreated {
		t.Fatalf("status = %d body = %s", resp.StatusCode, body)
	}
	var parsed map[string]map[string]string
	if err := json.Unmarshal([]byte(body), &parsed); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	if parsed["user"]["username"] != "newuser" {
		t.Fatalf("unexpected body: %s", body)
	}
	assertCookie(t, resp.Cookies(), "token")
	assertCookie(t, resp.Cookies(), "csrf")
}

func TestRegisterValidationErrorShape(t *testing.T) {
	server := newTestServer()
	defer server.Close()

	resp, body := postJSON(t, server.URL+"/api/auth/register", `{"username":"","email":"test@example.com","password":"password123"}`)
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusBadRequest {
		t.Fatalf("status = %d body = %s", resp.StatusCode, body)
	}
	if strings.TrimSpace(body) != `{"error":"Username is required"}` {
		t.Fatalf("unexpected body: %s", body)
	}
}

func postJSON(t *testing.T, url, body string) (*http.Response, string) {
	t.Helper()

	resp, err := http.Post(url, "application/json", strings.NewReader(body))
	if err != nil {
		t.Fatalf("post: %v", err)
	}
	bytes, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("read body: %v", err)
	}
	return resp, string(bytes)
}

func assertCookie(t *testing.T, cookies []*http.Cookie, name string) {
	t.Helper()

	for _, cookie := range cookies {
		if cookie.Name == name && cookie.Value != "" && cookie.Path == "/" && cookie.SameSite == http.SameSiteStrictMode {
			return
		}
	}
	t.Fatalf("cookie %q not found in %#v", name, cookies)
}
