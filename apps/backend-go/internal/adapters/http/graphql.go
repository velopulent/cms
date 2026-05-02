package httpadapter

import (
	"context"
	"net/http"

	"github.com/graphql-go/graphql"
	"github.com/labstack/echo/v4"

	"github.com/velopulent/cms/apps/backend-go/internal/application"
	"github.com/velopulent/cms/apps/backend-go/internal/middleware"
	"github.com/velopulent/cms/apps/backend-go/internal/ports"
)

type ctxKey string

func RegisterGraphQL(e *echo.Echo, s *application.Services) {
	siteType := graphql.NewObject(graphql.ObjectConfig{Name: "GraphQLSite", Fields: graphql.Fields{
		"id": {Type: graphql.String}, "name": {Type: graphql.String}, "storage_provider": {Type: graphql.String},
		"created_by": {Type: graphql.String}, "created_at": {Type: graphql.String}, "updated_at": {Type: graphql.String}, "role": {Type: graphql.String},
	}})
	collectionType := graphql.NewObject(graphql.ObjectConfig{Name: "GraphQLCollection", Fields: graphql.Fields{
		"id": {Type: graphql.String}, "site_id": {Type: graphql.String}, "name": {Type: graphql.String}, "slug": {Type: graphql.String}, "definition": {Type: graphql.String},
		"created_at": {Type: graphql.String}, "updated_at": {Type: graphql.String},
	}})
	entryType := graphql.NewObject(graphql.ObjectConfig{Name: "GraphQLEntry", Fields: graphql.Fields{
		"id": {Type: graphql.String}, "site_id": {Type: graphql.String}, "collection_id": {Type: graphql.String}, "data": {Type: graphql.String}, "slug": {Type: graphql.String}, "status": {Type: graphql.String},
	}})
	entriesListType := graphql.NewObject(graphql.ObjectConfig{Name: "GraphQLEntriesList", Fields: graphql.Fields{
		"items": {Type: graphql.NewList(entryType)}, "total": {Type: graphql.Int}, "page": {Type: graphql.Int}, "per_page": {Type: graphql.Int},
	}})
	schema, _ := graphql.NewSchema(graphql.SchemaConfig{Query: graphql.NewObject(graphql.ObjectConfig{Name: "Query", Fields: graphql.Fields{
		"sites": {Type: graphql.NewList(siteType), Resolve: func(p graphql.ResolveParams) (any, error) {
			pr := p.Context.Value(ctxKey("principal")).(ports.Principal)
			return s.Sites.ListForPrincipal(p.Context, pr)
		}},
		"collections": {Type: graphql.NewList(collectionType), Resolve: func(p graphql.ResolveParams) (any, error) {
			siteID, _ := p.Context.Value(ctxKey("site_id")).(string)
			return s.Collections.List(p.Context, siteID)
		}},
		"entries": {Type: entriesListType, Resolve: func(p graphql.ResolveParams) (any, error) {
			siteID, _ := p.Context.Value(ctxKey("site_id")).(string)
			return s.Entries.List(p.Context, ports.ListEntriesParams{SiteID: siteID, Page: 1, PerPage: 50})
		}},
	}})})
	handler := func(c echo.Context) error {
		pr, status, body := middleware.ResolvePrincipal(c, s)
		if status != 0 {
			return c.JSON(status, body)
		}
		ctx := context.WithValue(c.Request().Context(), ctxKey("principal"), pr)
		siteID := c.Request().Header.Get(application.HeaderSiteID)
		if pr.Kind == ports.PrincipalSite {
			siteID = pr.SiteID
		}
		ctx = context.WithValue(ctx, ctxKey("site_id"), siteID)
		query := c.QueryParam("query")
		if c.Request().Method == http.MethodPost {
			var req struct {
				Query string `json:"query"`
			}
			_ = c.Bind(&req)
			query = req.Query
		}
		return c.JSON(200, graphql.Do(graphql.Params{Schema: schema, RequestString: query, Context: ctx}))
	}
	e.GET("/api/graphql", handler)
	e.POST("/api/graphql", handler)
}
