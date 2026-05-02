package httpadapter

import (
	"embed"
	"io/fs"
	"net/http"
	"path"
	"strings"

	"github.com/labstack/echo/v4"
	echomiddleware "github.com/labstack/echo/v4/middleware"

	"github.com/velopulent/cms/apps/backend-go/internal/application"
	"github.com/velopulent/cms/apps/backend-go/internal/middleware"
)

//go:embed dashboard/dist
var dashboardFS embed.FS

func NewServer(services *application.Services) *echo.Echo {
	e := echo.New()
	e.HideBanner = true
	e.HidePort = true
	e.Use(echomiddleware.Recover())
	e.Use(echomiddleware.Logger())
	e.Use(echomiddleware.CORS())

	authHandler := NewAuthHandler(services.Auth, services.Config.CookieSecure)
	e.POST("/api/auth/register", authHandler.Register)
	e.POST("/api/auth/login", authHandler.Login)
	e.POST("/api/auth/logout", authHandler.Logout)
	e.GET("/api/auth/me", authHandler.Me, middleware.UserSession(services.Auth))

	protected := e.Group("", middleware.Principal(services))
	rest := NewRESTHandler(services)
	protected.GET("/api/v1/sites", rest.listSites)
	protected.POST("/api/v1/sites", rest.createSite)
	protected.GET("/api/v1/sites/:site_id", rest.getSite)
	protected.PUT("/api/v1/sites/:site_id", rest.updateSite)
	protected.DELETE("/api/v1/sites/:site_id", rest.deleteSite)
	protected.GET("/api/v1/sites/:site_id/members", rest.listMembers)
	protected.POST("/api/v1/sites/:site_id/members", rest.inviteMember)
	protected.PUT("/api/v1/sites/:site_id/members/:user_id", rest.updateMember)
	protected.DELETE("/api/v1/sites/:site_id/members/:user_id", rest.removeMember)

	protected.GET("/api/v1/collections", rest.listCollections)
	protected.POST("/api/v1/collections", rest.createCollection)
	protected.GET("/api/v1/collections/:collection_slug", rest.getCollection)
	protected.PUT("/api/v1/collections/:collection_slug", rest.updateCollection)
	protected.DELETE("/api/v1/collections/:collection_slug", rest.deleteCollection)

	protected.GET("/api/v1/singletons", rest.listSingletons)
	protected.GET("/api/v1/singletons/:slug", rest.getSingleton)
	protected.PUT("/api/v1/singletons/:slug", rest.updateSingleton)

	protected.GET("/api/v1/entries", rest.listEntries)
	protected.POST("/api/v1/entries", rest.createEntry)
	protected.GET("/api/v1/entries/:id", rest.getEntry)
	protected.PUT("/api/v1/entries/:id", rest.updateEntry)
	protected.DELETE("/api/v1/entries/:id", rest.deleteEntry)
	protected.POST("/api/v1/entries/:id/publish", rest.publishEntry)
	protected.POST("/api/v1/entries/:id/unpublish", rest.unpublishEntry)
	protected.GET("/api/v1/entries/:id/revisions", rest.listRevisions)
	protected.GET("/api/v1/entries/:id/revisions/:number", rest.getRevision)
	protected.POST("/api/v1/entries/:id/revisions/:number/restore", rest.restoreRevision)

	protected.GET("/api/v1/files", rest.listFiles)
	protected.POST("/api/v1/files", rest.uploadFile)
	protected.GET("/api/v1/files/:id", rest.getFile)
	protected.DELETE("/api/v1/files/:id", rest.deleteFile)
	protected.GET("/api/v1/files/:id/references", rest.fileReferences)
	protected.POST("/api/v1/files/:id/restore", rest.restoreFile)
	protected.POST("/api/v1/files/batch-delete", rest.batchDelete)
	protected.POST("/api/v1/files/batch-restore", rest.batchRestore)
	protected.POST("/api/v1/files/batch-permanent-delete", rest.batchPermanentDelete)

	protected.GET("/api/v1/tokens", rest.listInstanceTokens)
	protected.POST("/api/v1/tokens", rest.createInstanceToken)
	protected.DELETE("/api/v1/tokens/:token_id", rest.deleteInstanceToken)
	protected.GET("/api/v1/site-tokens", rest.listSiteTokens)
	protected.POST("/api/v1/site-tokens", rest.createSiteToken)
	protected.DELETE("/api/v1/site-tokens/:token_id", rest.deleteSiteToken)

	e.GET("/api/files/:id", rest.serveFile)
	e.GET("/api/files/:id/thumbnail", rest.serveFile)
	e.GET("/healthz", func(c echo.Context) error { return c.JSON(http.StatusOK, map[string]string{"status": "ok"}) })

	RegisterGraphQL(e, services)
	e.GET("/api/v1/docs", func(c echo.Context) error { return c.HTML(200, "OpenAPI docs are not yet generated for backend-go") })
	e.GET("/dashboard", dashboard)
	e.GET("/dashboard/*", dashboard)
	e.GET("/", dashboard)

	return e
}

func dashboard(c echo.Context) error {
	file := strings.TrimPrefix(c.Request().URL.Path, "/dashboard/")
	if file == "" || file == "/dashboard" || strings.HasSuffix(file, "/") {
		file = "index.html"
	}
	file = path.Clean(file)
	sub, _ := fs.Sub(dashboardFS, "dashboard/dist")
	data, err := fs.ReadFile(sub, file)
	if err != nil {
		data, err = fs.ReadFile(sub, "index.html")
		if err != nil {
			return c.NoContent(http.StatusNotFound)
		}
		file = "index.html"
	}
	ctype := mimeByExt(file)
	return c.Blob(http.StatusOK, ctype, data)
}

func mimeByExt(name string) string {
	switch path.Ext(name) {
	case ".html": return "text/html"
	case ".js": return "text/javascript"
	case ".css": return "text/css"
	case ".svg": return "image/svg+xml"
	case ".png": return "image/png"
	case ".jpg", ".jpeg": return "image/jpeg"
	case ".avif": return "image/avif"
	case ".webp": return "image/webp"
	default: return "application/octet-stream"
	}
}
