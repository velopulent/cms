package middleware

import (
	"net/http"
	"strings"

	"github.com/labstack/echo/v4"

	"github.com/velopulent/cms/apps/backend-go/internal/application"
	"github.com/velopulent/cms/apps/backend-go/internal/ports"
)

const (
	UserIDKey    = "user_id"
	PrincipalKey = "principal"
)

type AuthError struct {
	Error   string `json:"error"`
	Message string `json:"message"`
}

func Principal(s *application.Services) echo.MiddlewareFunc {
	return func(next echo.HandlerFunc) echo.HandlerFunc {
		return func(c echo.Context) error {
			p, status, body := ResolvePrincipal(c, s)
			if status != 0 {
				return c.JSON(status, body)
			}
			c.Set(PrincipalKey, p)
			if p.UserID != "" {
				c.Set(UserIDKey, p.UserID)
			}
			return next(c)
		}
	}
}

func UserSession(auth *application.AuthService) echo.MiddlewareFunc {
	return func(next echo.HandlerFunc) echo.HandlerFunc {
		return func(c echo.Context) error {
			token := bearerToken(c.Request())
			if token == "" {
				if cookie, err := c.Cookie("token"); err == nil {
					token = cookie.Value
				}
			}
			if token == "" {
				return c.JSON(http.StatusUnauthorized, AuthError{Error: "unauthorized", Message: "Missing authentication"})
			}
			claims, err := auth.VerifyToken(token)
			if err != nil {
				return c.JSON(http.StatusUnauthorized, AuthError{Error: "unauthorized", Message: "Invalid or expired token"})
			}
			c.Set(UserIDKey, claims.Sub)
			c.Set(PrincipalKey, ports.Principal{Kind: ports.PrincipalUser, UserID: claims.Sub})
			return next(c)
		}
	}
}

func ResolvePrincipal(c echo.Context, s *application.Services) (ports.Principal, int, AuthError) {
	token := bearerToken(c.Request())
	if token != "" && strings.HasPrefix(token, "cms_") {
		p, err := s.Tokens.Verify(c.Request().Context(), token)
		if err != nil {
			return ports.Principal{}, http.StatusUnauthorized, AuthError{Error: "unauthorized", Message: application.Message(err)}
		}
		return p, 0, AuthError{}
	}
	if token == "" {
		if cookie, err := c.Cookie("token"); err == nil {
			token = cookie.Value
		}
	}
	if token == "" {
		return ports.Principal{}, http.StatusUnauthorized, AuthError{Error: "unauthorized", Message: "Missing authentication"}
	}
	claims, err := s.Auth.VerifyToken(token)
	if err != nil {
		return ports.Principal{}, http.StatusUnauthorized, AuthError{Error: "unauthorized", Message: "Invalid or expired token"}
	}
	return ports.Principal{Kind: ports.PrincipalUser, UserID: claims.Sub}, 0, AuthError{}
}

func bearerToken(r *http.Request) string {
	header := r.Header.Get("Authorization")
	token, ok := strings.CutPrefix(header, "Bearer ")
	if !ok {
		return ""
	}
	return strings.TrimSpace(token)
}

func GetPrincipal(c echo.Context) ports.Principal {
	p, _ := c.Get(PrincipalKey).(ports.Principal)
	return p
}

func RequireAdminScope(c echo.Context, s *application.Services, siteID *string, scope string) error {
	p := GetPrincipal(c)
	switch p.Kind {
	case ports.PrincipalInstance:
		if p.Scopes[scope] {
			return nil
		}
		return c.JSON(http.StatusForbidden, AuthError{Error: "insufficient_scope", Message: "Token is missing required scope: " + scope + "."})
	case ports.PrincipalUser:
		if scope == application.ScopeSitesRead || scope == application.ScopeSitesWrite {
			return nil
		}
		min := "viewer"
		if scope == application.ScopeSitesDelete { min = "owner" }
		if scope == application.ScopeMembersWrite || scope == application.ScopeTokensRead || scope == application.ScopeTokensWrite { min = "admin" }
		if siteID == nil { return c.JSON(http.StatusForbidden, AuthError{Error:"forbidden", Message:"Site id is required"}) }
		return RequireRole(c, s, p.UserID, *siteID, min)
	default:
		return c.JSON(http.StatusForbidden, AuthError{Error: "site_token_denied", Message: "Site tokens cannot access this endpoint."})
	}
}

func ResolveSite(c echo.Context, s *application.Services, scope, minRole string) (string, error) {
	p := GetPrincipal(c)
	headerSite := c.Request().Header.Get(application.HeaderSiteID)
	switch p.Kind {
	case ports.PrincipalSite:
		if headerSite != "" && headerSite != p.SiteID {
			return "", c.JSON(http.StatusForbidden, AuthError{Error:"forbidden", Message:"Site token does not have access to this site"})
		}
		if !p.Scopes[scope] {
			return "", c.JSON(http.StatusForbidden, AuthError{Error:"insufficient_scope", Message:"Token is missing required scope: "+scope+"."})
		}
		return p.SiteID, nil
	case ports.PrincipalUser:
		if headerSite == "" {
			return "", c.JSON(http.StatusForbidden, AuthError{Error:"forbidden", Message:"Missing site context"})
		}
		if err := RequireRole(c, s, p.UserID, headerSite, minRole); err != nil { return "", err }
		return headerSite, nil
	default:
		return "", c.JSON(http.StatusForbidden, AuthError{Error:"instance_token_denied", Message:"Instance tokens cannot access this endpoint."})
	}
}

func RequireRole(c echo.Context, s *application.Services, userID, siteID, minRole string) error {
	role, err := s.Sites.Role(c.Request().Context(), userID, siteID)
	if err != nil {
		return c.JSON(http.StatusInternalServerError, AuthError{Error:"internal_error", Message:err.Error()})
	}
	if role == nil {
		return c.JSON(http.StatusNotFound, AuthError{Error:"not_found", Message:"Site not found"})
	}
	if roleLevel(*role) < roleLevel(minRole) {
		return c.JSON(http.StatusForbidden, AuthError{Error:"insufficient_role", Message:"This action requires the '"+minRole+"' role or higher."})
	}
	return nil
}

func roleLevel(role string) int {
	switch role {
	case "owner": return 4
	case "admin": return 3
	case "editor": return 2
	case "viewer": return 1
	default: return 0
	}
}
