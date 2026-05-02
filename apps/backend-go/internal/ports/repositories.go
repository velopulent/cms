package ports

import (
	"context"
	"io"

	"github.com/velopulent/cms/apps/backend-go/internal/domain"
)

type PrincipalKind string

const (
	PrincipalUser     PrincipalKind = "user"
	PrincipalInstance PrincipalKind = "instance"
	PrincipalSite     PrincipalKind = "site"
)

type Principal struct {
	Kind    PrincipalKind
	UserID  string
	TokenID string
	SiteID  string
	Scopes  map[string]bool
}

type ListEntriesParams struct {
	SiteID         string
	CollectionSlug string
	CollectionID   string
	Status         string
	Search         string
	PublishedOnly  bool
	Page           int64
	PerPage        int64
}

type ListFilesParams struct {
	SiteID   string
	Trashed  bool
	Search   string
	FileType string
	Page     int64
	PerPage  int64
}

type SiteRepository interface {
	ListAll(ctx context.Context) ([]domain.Site, error)
	ListForUser(ctx context.Context, userID string) ([]domain.SiteWithRole, error)
	GetByID(ctx context.Context, id string) (*domain.Site, error)
	Create(ctx context.Context, id, name, storageProvider, createdBy string) (*domain.Site, error)
	Update(ctx context.Context, id, name string) (*domain.Site, error)
	Delete(ctx context.Context, id string) (int64, error)
	ListMembers(ctx context.Context, siteID string) ([]domain.SiteMember, error)
	AddMember(ctx context.Context, id, siteID, userID, role string) (*domain.SiteMember, error)
	UpdateMemberRole(ctx context.Context, siteID, userID, role string) (*domain.SiteMember, error)
	RemoveMember(ctx context.Context, siteID, userID string) (int64, error)
}

type CollectionRepository interface {
	List(ctx context.Context, siteID string) ([]domain.Collection, error)
	ListSingletons(ctx context.Context, siteID string) ([]domain.Collection, error)
	GetBySlug(ctx context.Context, siteID, slug string) (*domain.Collection, error)
	GetByID(ctx context.Context, id string) (*domain.Collection, error)
	Create(ctx context.Context, id, siteID, name, slug, definition string, singleton bool) (*domain.Collection, error)
	Update(ctx context.Context, id, name, slug, definition string) (*domain.Collection, error)
	UpdateSingletonData(ctx context.Context, id, data string) (*domain.Collection, error)
	Delete(ctx context.Context, siteID, slug string) (int64, error)
}

type EntryRepository interface {
	GetByID(ctx context.Context, id, siteID string, publishedOnly bool) (*domain.Entry, error)
	GetByIDAnySite(ctx context.Context, id string) (*domain.Entry, error)
	List(ctx context.Context, params ListEntriesParams) (domain.EntriesListResult, error)
	Create(ctx context.Context, id, siteID, collectionID, data, slug string, createdBy *string) (*domain.Entry, error)
	Update(ctx context.Context, id, siteID, data, slug, status string, createdBy *string, summary *string) (*domain.Entry, error)
	Delete(ctx context.Context, id, siteID string) (int64, error)
	Publish(ctx context.Context, id, siteID string) (*domain.Entry, error)
	Unpublish(ctx context.Context, id, siteID string) (*domain.Entry, error)
	SyncFileReferences(ctx context.Context, entryID, siteID string, data []byte) error
	ListRevisions(ctx context.Context, entryID string, page, perPage int64) (domain.RevisionsListResult, error)
	GetRevision(ctx context.Context, entryID string, number int64) (*domain.EntryRevision, error)
	RestoreRevision(ctx context.Context, entryID string, number int64, createdBy *string) (*domain.Entry, error)
}

type FileRepository interface {
	GetByID(ctx context.Context, id, siteID string) (*domain.File, error)
	GetByIDAny(ctx context.Context, id string) (*domain.File, error)
	List(ctx context.Context, params ListFilesParams) (domain.FileListResult, error)
	Create(ctx context.Context, file domain.File) (*domain.File, error)
	SoftDelete(ctx context.Context, id, siteID string) (int64, error)
	Restore(ctx context.Context, id, siteID string) (int64, error)
	BatchSoftDelete(ctx context.Context, siteID string, ids []string) (int64, error)
	BatchRestore(ctx context.Context, siteID string, ids []string) (int64, error)
	GetByIDs(ctx context.Context, siteID string, ids []string, deletedOnly bool) ([]domain.File, error)
	BatchPermanentDelete(ctx context.Context, siteID string, ids []string) (int64, error)
	GetReferencesForSite(ctx context.Context, fileID, siteID string) ([]domain.FileReference, error)
	GetStorageProvider(ctx context.Context, siteID string) (string, error)
}

type AccessTokenRepository interface {
	List(ctx context.Context, kind string, siteID *string) ([]domain.AccessToken, error)
	Create(ctx context.Context, token domain.AccessToken, tokenHash string) error
	Delete(ctx context.Context, id, kind string, siteID *string) (int64, error)
	FindByPrefix(ctx context.Context, prefix string) ([]domain.AccessToken, error)
	UpdateLastUsed(ctx context.Context, id string) error
}

type StorageProvider interface {
	Put(ctx context.Context, key string, r io.Reader, contentType string) error
	Get(ctx context.Context, key string) ([]byte, error)
	Delete(ctx context.Context, key string) error
	URL(key, fileID string) string
	Name() string
}
