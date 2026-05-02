package httpadapter

import (
	"net/http"
	"time"

	"github.com/google/uuid"
	"github.com/labstack/echo/v4"

	"github.com/velopulent/cms/apps/backend-go/internal/application"
	"github.com/velopulent/cms/apps/backend-go/internal/domain"
	"github.com/velopulent/cms/apps/backend-go/internal/middleware"
)

type AuthHandler struct {
	auth         *application.AuthService
	cookieSecure bool
}

func NewAuthHandler(auth *application.AuthService, cookieSecure bool) AuthHandler {
	return AuthHandler{auth: auth, cookieSecure: cookieSecure}
}

type createUserRequest struct {
	Username string `json:"username"`
	Email    string `json:"email"`
	Password string `json:"password"`
}

type loginRequest struct {
	Username string `json:"username"`
	Password string `json:"password"`
}

func (h AuthHandler) Register(c echo.Context) error {
	var req createUserRequest
	if err := c.Bind(&req); err != nil {
		return c.JSON(http.StatusBadRequest, errorBody{Error: "Invalid input"})
	}

	user, token, err := h.auth.Register(c.Request().Context(), req.Username, req.Email, req.Password)
	if err != nil {
		return writeAppError(c, err)
	}

	h.setAuthCookies(c, token)
	return c.JSON(http.StatusCreated, domain.AuthResponse{User: user})
}

func (h AuthHandler) Login(c echo.Context) error {
	var req loginRequest
	if err := c.Bind(&req); err != nil {
		return c.JSON(http.StatusBadRequest, errorBody{Error: "Invalid input"})
	}

	user, token, err := h.auth.Login(c.Request().Context(), req.Username, req.Password)
	if err != nil {
		return writeAppError(c, err)
	}

	h.setAuthCookies(c, token)
	return c.JSON(http.StatusOK, domain.AuthResponse{User: user})
}

func (h AuthHandler) Logout(c echo.Context) error {
	h.clearAuthCookies(c)
	return c.JSON(http.StatusOK, map[string]string{"message": "Logged out"})
}

func (h AuthHandler) Me(c echo.Context) error {
	userID, _ := c.Get(middleware.UserIDKey).(string)
	user, err := h.auth.GetUser(c.Request().Context(), userID)
	if err != nil {
		return writeAppError(c, err)
	}
	if user == nil {
		return c.JSON(http.StatusNotFound, errorBody{Error: "User not found"})
	}
	return c.JSON(http.StatusOK, user)
}

func (h AuthHandler) setAuthCookies(c echo.Context, token string) {
	maxAge := int((24 * time.Hour).Seconds())
	c.SetCookie(&http.Cookie{
		Name:     "token",
		Value:    token,
		Path:     "/",
		MaxAge:   maxAge,
		HttpOnly: true,
		Secure:   h.cookieSecure,
		SameSite: http.SameSiteStrictMode,
	})
	c.SetCookie(&http.Cookie{
		Name:     "csrf",
		Value:    uuid.Must(uuid.NewV7()).String(),
		Path:     "/",
		MaxAge:   maxAge,
		HttpOnly: false,
		Secure:   h.cookieSecure,
		SameSite: http.SameSiteStrictMode,
	})
}

func (h AuthHandler) clearAuthCookies(c echo.Context) {
	c.SetCookie(&http.Cookie{
		Name:     "token",
		Value:    "",
		Path:     "/",
		MaxAge:   -1,
		HttpOnly: true,
		SameSite: http.SameSiteDefaultMode,
	})
	c.SetCookie(&http.Cookie{
		Name:     "csrf",
		Value:    "",
		Path:     "/",
		MaxAge:   -1,
		HttpOnly: false,
		SameSite: http.SameSiteDefaultMode,
	})
}
