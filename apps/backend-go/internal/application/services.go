package application

import (
	"context"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"mime"
	"path/filepath"
	"regexp"
	"slices"
	"strings"
	"time"

	"github.com/google/uuid"
	"golang.org/x/crypto/bcrypt"

	"github.com/velopulent/cms/apps/backend-go/internal/config"
	"github.com/velopulent/cms/apps/backend-go/internal/domain"
	"github.com/velopulent/cms/apps/backend-go/internal/ports"
)

const (
	HeaderSiteID = "x-cms-site-id"

	ScopeSitesRead    = "sites:read"
	ScopeSitesWrite   = "sites:write"
	ScopeSitesDelete  = "sites:delete"
	ScopeMembersRead  = "members:read"
	ScopeMembersWrite = "members:write"
	ScopeTokensRead   = "tokens:read"
	ScopeTokensWrite  = "tokens:write"
	ScopeSiteRead     = "site:read"
	ScopeSchemaRead   = "schema:read"
	ScopeSchemaWrite  = "schema:write"
	ScopeContentRead  = "content:read"
	ScopeContentWrite = "content:write"
	ScopeAssetsRead   = "assets:read"
	ScopeAssetsWrite  = "assets:write"
)

type Services struct {
	Auth       *AuthService
	Sites      *SiteService
	Collections *CollectionService
	Entries    *EntryService
	Files      *FileService
	Tokens     *TokenService
	Config     config.Config
}

func NewServices(cfg config.Config, user ports.UserRepository, sites ports.SiteRepository, collections ports.CollectionRepository, entries ports.EntryRepository, files ports.FileRepository, tokens ports.AccessTokenRepository, signer ports.TokenSigner, storage map[string]ports.StorageProvider) *Services {
	auth := NewAuthService(user, signer)
	return &Services{
		Auth:        auth,
		Sites:       &SiteService{users: user, sites: sites},
		Collections: &CollectionService{repo: collections},
		Entries:     &EntryService{repo: entries, files: files, storage: storage},
		Files:       &FileService{repo: files, cfg: cfg, storage: storage},
		Tokens:      &TokenService{repo: tokens, hmacSecret: cfg.HMACSecret},
		Config:      cfg,
	}
}

type SiteService struct {
	users ports.UserRepository
	sites ports.SiteRepository
}

var validRoles = []string{"owner", "admin", "editor", "viewer"}

func (s *SiteService) ListForPrincipal(ctx context.Context, p ports.Principal) ([]map[string]any, error) {
	if p.Kind == ports.PrincipalInstance {
		items, err := s.sites.ListAll(ctx)
		if err != nil { return nil, Internal(err) }
		out := make([]map[string]any, 0, len(items))
		for _, site := range items {
			out = append(out, map[string]any{"id": site.ID, "name": site.Name, "storage_provider": site.StorageProvider, "created_by": site.CreatedBy, "created_at": site.CreatedAt, "updated_at": site.UpdatedAt, "role": "instance_admin"})
		}
		return out, nil
	}
	items, err := s.sites.ListForUser(ctx, p.UserID)
	if err != nil { return nil, Internal(err) }
	out := make([]map[string]any, 0, len(items))
	for _, site := range items {
		out = append(out, map[string]any{"id": site.ID, "name": site.Name, "storage_provider": site.StorageProvider, "created_by": site.CreatedBy, "created_at": site.CreatedAt, "updated_at": site.UpdatedAt, "role": site.Role})
	}
	return out, nil
}

func (s *SiteService) Get(ctx context.Context, id string) (*domain.Site, error) {
	site, err := s.sites.GetByID(ctx, id)
	if err != nil { return nil, Internal(err) }
	return site, nil
}

func (s *SiteService) Create(ctx context.Context, name, storageProvider, createdBy string) (*domain.Site, error) {
	name = strings.TrimSpace(name)
	if name == "" { return nil, Validation(errors.New("Name is required")) }
	if storageProvider == "" { storageProvider = "filesystem" }
	if storageProvider != "filesystem" && storageProvider != "s3" {
		return nil, Validation(errors.New("Invalid storage provider. Must be 'filesystem' or 's3'"))
	}
	id := mustUUIDv7()
	site, err := s.sites.Create(ctx, id, name, storageProvider, createdBy)
	if err != nil { return nil, Internal(err) }
	return site, nil
}

func (s *SiteService) Update(ctx context.Context, id string, name *string) (*domain.Site, error) {
	existing, err := s.sites.GetByID(ctx, id)
	if err != nil { return nil, Internal(err) }
	if existing == nil { return nil, NotFound(errors.New("Site not found")) }
	nextName := existing.Name
	if name != nil { nextName = *name }
	site, err := s.sites.Update(ctx, id, nextName)
	if err != nil { return nil, Internal(err) }
	return site, nil
}

func (s *SiteService) Delete(ctx context.Context, id string) (int64, error) {
	n, err := s.sites.Delete(ctx, id)
	if err != nil { return 0, Internal(err) }
	return n, nil
}

func (s *SiteService) ListMembers(ctx context.Context, siteID string) ([]domain.SiteMember, error) {
	items, err := s.sites.ListMembers(ctx, siteID)
	if err != nil { return nil, Internal(err) }
	return items, nil
}

func (s *SiteService) InviteMember(ctx context.Context, siteID, username, role string) (*domain.SiteMember, error) {
	if !slices.Contains(validRoles, role) { return nil, Validation(errors.New("Invalid role. Must be owner, admin, editor, or viewer")) }
	userID, err := s.users.FindIDByUsername(ctx, username)
	if err != nil { return nil, Internal(err) }
	if userID == nil { return nil, NotFound(errors.New("User not found")) }
	member, err := s.sites.AddMember(ctx, mustUUIDv7(), siteID, *userID, role)
	if err != nil {
		if errors.Is(err, ErrDuplicateUser) { return nil, Conflict(errors.New("User is already a member of this site")) }
		return nil, Internal(err)
	}
	return member, nil
}

func (s *SiteService) UpdateMemberRole(ctx context.Context, siteID, userID, role string) (*domain.SiteMember, error) {
	if !slices.Contains(validRoles, role) { return nil, Validation(errors.New("Invalid role")) }
	member, err := s.sites.UpdateMemberRole(ctx, siteID, userID, role)
	if err != nil { return nil, Internal(err) }
	return member, nil
}

func (s *SiteService) RemoveMember(ctx context.Context, siteID, userID, byUserID string) (int64, error) {
	if userID == byUserID { return 0, Validation(errors.New("Cannot remove yourself from the site")) }
	n, err := s.sites.RemoveMember(ctx, siteID, userID)
	if err != nil { return 0, Internal(err) }
	return n, nil
}

func (s *SiteService) Role(ctx context.Context, userID, siteID string) (*string, error) {
	role, err := s.users.GetRole(ctx, userID, siteID)
	if err != nil { return nil, err }
	return role, nil
}

type CollectionService struct { repo ports.CollectionRepository }

func (s *CollectionService) List(ctx context.Context, siteID string) ([]domain.Collection, error) { v, err := s.repo.List(ctx, siteID); if err != nil { return nil, Internal(err) }; return v, nil }
func (s *CollectionService) ListSingletons(ctx context.Context, siteID string) ([]domain.SingletonResponse, error) {
	items, err := s.repo.ListSingletons(ctx, siteID); if err != nil { return nil, Internal(err) }
	out := make([]domain.SingletonResponse, 0, len(items))
	for _, c := range items { out = append(out, singletonResponse(c)) }
	return out, nil
}
func (s *CollectionService) Get(ctx context.Context, siteID, slug string) (*domain.Collection, error) { v, err := s.repo.GetBySlug(ctx, siteID, slug); if err != nil { return nil, Internal(err) }; return v, nil }
func (s *CollectionService) Create(ctx context.Context, siteID, name, slug string, definition any, singleton bool) (*domain.Collection, error) {
	b, _ := json.Marshal(definition)
	v, err := s.repo.Create(ctx, mustUUIDv7(), siteID, name, slug, string(b), singleton)
	if err != nil { if errors.Is(err, ErrDuplicateUser) { return nil, Conflict(errors.New("Collection with this name or slug already exists")) }; return nil, Internal(err) }
	return v, nil
}
func (s *CollectionService) Update(ctx context.Context, siteID, slug string, name, newSlug *string, definition any) (*domain.Collection, error) {
	existing, err := s.repo.GetBySlug(ctx, siteID, slug); if err != nil { return nil, Internal(err) }
	if existing == nil { return nil, NotFound(errors.New("Collection not found")) }
	n, sl, def := existing.Name, existing.Slug, existing.Definition
	if name != nil { n = *name }; if newSlug != nil { sl = *newSlug }; if definition != nil { b, _ := json.Marshal(definition); def = string(b) }
	v, err := s.repo.Update(ctx, existing.ID, n, sl, def); if err != nil { return nil, Internal(err) }; return v, nil
}
func (s *CollectionService) Delete(ctx context.Context, siteID, slug string) (int64, error) { n, err := s.repo.Delete(ctx, siteID, slug); if err != nil { return 0, Internal(err) }; return n, nil }
func (s *CollectionService) GetSingleton(ctx context.Context, siteID, slug string) (*domain.SingletonResponse, error) {
	c, err := s.repo.GetBySlug(ctx, siteID, slug); if err != nil { return nil, Internal(err) }
	if c == nil || !c.IsSingleton { return nil, NotFound(errors.New("Singleton not found")) }
	r := singletonResponse(*c); return &r, nil
}
func (s *CollectionService) UpdateSingleton(ctx context.Context, siteID, slug string, data any) (*domain.SingletonResponse, error) {
	c, err := s.repo.GetBySlug(ctx, siteID, slug); if err != nil { return nil, Internal(err) }
	if c == nil || !c.IsSingleton { return nil, NotFound(errors.New("Singleton not found")) }
	b, _ := json.Marshal(data)
	updated, err := s.repo.UpdateSingletonData(ctx, c.ID, string(b)); if err != nil { return nil, Internal(err) }
	r := singletonResponse(*updated); return &r, nil
}

func singletonResponse(c domain.Collection) domain.SingletonResponse {
	var def any = map[string]any{"fields": []any{}}
	_ = json.Unmarshal([]byte(c.Definition), &def)
	var data any
	if c.SingletonData != nil { _ = json.Unmarshal([]byte(*c.SingletonData), &data) }
	return domain.SingletonResponse{ID:c.ID, SiteID:c.SiteID, Name:c.Name, Slug:c.Slug, Definition:def, Data:data, CreatedAt:c.CreatedAt, UpdatedAt:c.UpdatedAt}
}

type EntryService struct { repo ports.EntryRepository; files ports.FileRepository; storage map[string]ports.StorageProvider }

func (s *EntryService) List(ctx context.Context, p ports.ListEntriesParams) (domain.EntriesListResult, error) { v, err := s.repo.List(ctx, p); if err != nil { return domain.EntriesListResult{}, Internal(err) }; return v, nil }
func (s *EntryService) Get(ctx context.Context, id, siteID string, publishedOnly bool) (*domain.Entry, error) { v, err := s.repo.GetByID(ctx, id, siteID, publishedOnly); if err != nil { return nil, Internal(err) }; return v, nil }
func (s *EntryService) Create(ctx context.Context, siteID, collectionID string, data any, slug string, createdBy *string) (*domain.Entry, error) {
	b, _ := json.Marshal(data); v, err := s.repo.Create(ctx, mustUUIDv7(), siteID, collectionID, string(b), slug, createdBy)
	if err != nil { if errors.Is(err, ErrDuplicateUser) { return nil, Conflict(errors.New("Entry with this slug already exists for this collection")) }; return nil, Internal(err) }
	_ = s.repo.SyncFileReferences(ctx, v.ID, siteID, b); return v, nil
}
func (s *EntryService) Update(ctx context.Context, id, siteID string, data any, slug, status *string, createdBy, summary *string) (*domain.Entry, error) {
	ex, err := s.repo.GetByID(ctx, id, siteID, false); if err != nil { return nil, Internal(err) }; if ex == nil { return nil, NotFound(errors.New("Entry not found")) }
	dataStr := ex.Data; if data != nil { b, _ := json.Marshal(data); dataStr = string(b) }
	nextSlug, nextStatus := ex.Slug, ex.Status; if slug != nil { nextSlug = *slug }; if status != nil { nextStatus = *status }
	v, err := s.repo.Update(ctx, id, siteID, dataStr, nextSlug, nextStatus, createdBy, summary)
	if err != nil { if errors.Is(err, ErrDuplicateUser) { return nil, Conflict(errors.New("Entry with this slug already exists for this collection")) }; return nil, Internal(err) }
	_ = s.repo.SyncFileReferences(ctx, id, siteID, []byte(dataStr)); return v, nil
}
func (s *EntryService) Delete(ctx context.Context, id, siteID string) (int64, error) { n, err := s.repo.Delete(ctx, id, siteID); if err != nil { return 0, Internal(err) }; return n, nil }
func (s *EntryService) Publish(ctx context.Context, id, siteID string) (*domain.Entry, error) { v, err := s.repo.Publish(ctx, id, siteID); if err != nil { return nil, NotFound(errors.New("Entry not found")) }; return v, nil }
func (s *EntryService) Unpublish(ctx context.Context, id, siteID string) (*domain.Entry, error) { v, err := s.repo.Unpublish(ctx, id, siteID); if err != nil { return nil, NotFound(errors.New("Entry not found")) }; return v, nil }
func (s *EntryService) ListRevisions(ctx context.Context, entryID, siteID string, page, perPage int64) (domain.RevisionsListResult, error) { if e, _ := s.repo.GetByID(ctx, entryID, siteID, false); e == nil { return domain.RevisionsListResult{}, NotFound(errors.New("Entry not found")) }; v, err := s.repo.ListRevisions(ctx, entryID, page, perPage); if err != nil { return domain.RevisionsListResult{}, Internal(err) }; return v, nil }
func (s *EntryService) GetRevision(ctx context.Context, entryID, siteID string, number int64) (*domain.EntryRevision, error) { if e, _ := s.repo.GetByID(ctx, entryID, siteID, false); e == nil { return nil, NotFound(errors.New("Entry not found")) }; v, err := s.repo.GetRevision(ctx, entryID, number); if err != nil { return nil, Internal(err) }; return v, nil }
func (s *EntryService) RestoreRevision(ctx context.Context, entryID, siteID string, number int64, createdBy *string) (*domain.Entry, error) { if e, _ := s.repo.GetByID(ctx, entryID, siteID, false); e == nil { return nil, NotFound(errors.New("Entry not found")) }; v, err := s.repo.RestoreRevision(ctx, entryID, number, createdBy); if err != nil { return nil, NotFound(errors.New("Revision not found")) }; return v, nil }
func RevisionResponse(r domain.EntryRevision) domain.EntryRevisionResponse { var data any; _ = json.Unmarshal([]byte(r.Data), &data); return domain.EntryRevisionResponse{ID:r.ID, EntryID:r.EntryID, RevisionNumber:r.RevisionNumber, Data:data, CreatedBy:r.CreatedBy, CreatedAt:r.CreatedAt, ChangeSummary:r.ChangeSummary} }

type FileService struct { repo ports.FileRepository; cfg config.Config; storage map[string]ports.StorageProvider }
func (s *FileService) List(ctx context.Context, p ports.ListFilesParams) (domain.FileListResult, error) { v, err := s.repo.List(ctx, p); if err != nil { return domain.FileListResult{}, Internal(err) }; return v, nil }
func (s *FileService) Get(ctx context.Context, id, siteID string) (*domain.File, error) { v, err := s.repo.GetByID(ctx, id, siteID); if err != nil { return nil, Internal(err) }; return v, nil }
func (s *FileService) GetAny(ctx context.Context, id string) (*domain.File, error) { v, err := s.repo.GetByIDAny(ctx, id); if err != nil { return nil, Internal(err) }; return v, nil }
func (s *FileService) WithURL(f domain.File) domain.FileWithURL {
	st := s.storage[f.StorageProvider]; url := "/api/files/"+f.ID; if st != nil { url = st.URL(f.StorageKey, f.ID) }
	var thumb *string; if f.ThumbnailKey != nil { v := "/api/files/"+f.ID+"/thumbnail"; thumb = &v }
	return domain.FileWithURL{ID:f.ID, SiteID:f.SiteID, Filename:f.Filename, OriginalName:f.OriginalName, MimeType:f.MimeType, Size:f.Size, StorageProvider:f.StorageProvider, StorageKey:f.StorageKey, ThumbnailKey:f.ThumbnailKey, Width:f.Width, Height:f.Height, DeletedAt:f.DeletedAt, CreatedBy:f.CreatedBy, CreatedAt:f.CreatedAt, URL:url, ThumbnailURL:thumb}
}
func (s *FileService) Upload(ctx context.Context, siteID string, data []byte, originalName, contentType string, createdBy *string) (*domain.FileWithURL, error) {
	if len(data) > s.cfg.MaxUploadSizeBytes { return nil, AppError{Kind:KindValidation, Err:fmt.Errorf("File too large. Maximum size is %dMB", s.cfg.MaxUploadSizeBytes/(1024*1024))} }
	provider, _ := s.repo.GetStorageProvider(ctx, siteID); if provider == "" { provider = "filesystem" }
	st := s.storage[provider]; if st == nil { return nil, Internal(errors.New("Storage not configured")) }
	id := mustUUIDv7(); ext := strings.TrimPrefix(filepath.Ext(originalName), "."); if ext == "" { exts, _ := mime.ExtensionsByType(contentType); if len(exts) > 0 { ext = strings.TrimPrefix(exts[0], ".") } else { ext = "bin" } }
	filename := id[:8]+"."+ext; key := fmt.Sprintf("s_%s/f_%s/%s", siteID, id, filename)
	if err := st.Put(ctx, key, strings.NewReader(string(data)), contentType); err != nil { return nil, Internal(fmt.Errorf("Failed to store file: %w", err)) }
	file := domain.File{ID:id, SiteID:siteID, Filename:filename, OriginalName:originalName, MimeType:contentType, Size:int64(len(data)), StorageProvider:provider, StorageKey:key, CreatedBy:createdBy}
	created, err := s.repo.Create(ctx, file); if err != nil { return nil, Internal(err) }
	w := s.WithURL(*created); return &w, nil
}
func (s *FileService) SoftDelete(ctx context.Context, id, siteID string) (int64, error) { n, err := s.repo.SoftDelete(ctx, id, siteID); if err != nil { return 0, Internal(err) }; return n, nil }
func (s *FileService) Restore(ctx context.Context, id, siteID string) (int64, error) { n, err := s.repo.Restore(ctx, id, siteID); if err != nil { return 0, Internal(err) }; return n, nil }
func (s *FileService) BatchSoftDelete(ctx context.Context, siteID string, ids []string) (int64, error) { n, err := s.repo.BatchSoftDelete(ctx, siteID, ids); if err != nil { return 0, Internal(err) }; return n, nil }
func (s *FileService) BatchRestore(ctx context.Context, siteID string, ids []string) (int64, error) { n, err := s.repo.BatchRestore(ctx, siteID, ids); if err != nil { return 0, Internal(err) }; return n, nil }
func (s *FileService) BatchPermanentDelete(ctx context.Context, siteID string, ids []string) (int64, error) { n, err := s.repo.BatchPermanentDelete(ctx, siteID, ids); if err != nil { return 0, Internal(err) }; return n, nil }
func (s *FileService) References(ctx context.Context, fileID, siteID string) ([]domain.FileReference, error) { v, err := s.repo.GetReferencesForSite(ctx, fileID, siteID); if err != nil { return nil, Internal(err) }; return v, nil }
func (s *FileService) Serve(ctx context.Context, id string, thumb bool) ([]byte, string, string, error) {
	f, err := s.repo.GetByIDAny(ctx, id); if err != nil { return nil, "", "", Internal(err) }; if f == nil || f.DeletedAt != nil { return nil, "", "", NotFound(errors.New("File not found")) }
	key, ctype := f.StorageKey, f.MimeType; if thumb { if f.ThumbnailKey == nil { return nil, "", "", NotFound(errors.New("File not found")) }; key = *f.ThumbnailKey; ctype = "image/avif" }
	st := s.storage[f.StorageProvider]; if st == nil { return nil, "", "", Internal(errors.New("Storage not configured")) }
	b, err := st.Get(ctx, key); if err != nil { return nil, "", "", Internal(err) }; return b, ctype, f.OriginalName, nil
}

type TokenService struct { repo ports.AccessTokenRepository; hmacSecret string }
func (s *TokenService) List(ctx context.Context, kind string, siteID *string) ([]domain.AccessToken, error) { v, err := s.repo.List(ctx, kind, siteID); if err != nil { return nil, Internal(err) }; return v, nil }
func (s *TokenService) Create(ctx context.Context, kind string, siteID *string, name string, scopes []string, createdBy *string) (*domain.AccessTokenResponse, error) {
	name = strings.TrimSpace(name); if name == "" { return nil, Validation(errors.New("Name is required")) }
	defaults := defaultScopes(kind); if len(scopes)==0 { scopes = defaults }
	for _, sc := range scopes { if !slices.Contains(defaults, sc) { return nil, Validation(fmt.Errorf("Unsupported scope '%s'", sc)) } }
	raw := kindPrefix(kind)+randomHex(16); prefix := raw; if len(prefix)>24 { prefix = prefix[:24] }
	hash, _ := bcrypt.GenerateFromPassword([]byte(raw), bcrypt.DefaultCost); id := mustUUIDv7(); now := nowString()
	token := domain.AccessToken{ID:id, Kind:kind, SiteID:siteID, Name:name, TokenPrefix:prefix, Scopes:strings.Join(scopes, ","), CreatedByUserID:createdBy, CreatedAt:now}
	h := computeHMAC(raw, s.hmacSecret); token.TokenHMAC = &h
	if err := s.repo.Create(ctx, token, string(hash)); err != nil { return nil, Internal(err) }
	return &domain.AccessTokenResponse{ID:id, Kind:kind, SiteID:siteID, Name:name, Token:raw, TokenPrefix:prefix, Scopes:scopes, CreatedAt:now}, nil
}
func (s *TokenService) Delete(ctx context.Context, id, kind string, siteID *string) (int64, error) { n, err := s.repo.Delete(ctx, id, kind, siteID); if err != nil { return 0, Internal(err) }; return n, nil }
func (s *TokenService) Verify(ctx context.Context, raw string) (ports.Principal, error) {
	prefix := raw; if len(prefix)>24 { prefix = prefix[:24] }
	rows, err := s.repo.FindByPrefix(ctx, prefix); if err != nil { return ports.Principal{}, Unauthorized(errors.New("Internal server error")) }
	h := computeHMAC(raw, s.hmacSecret)
	for _, t := range rows {
		if t.TokenHMAC != nil && *t.TokenHMAC != h { continue }
		if t.RevokedAt != nil { return ports.Principal{}, Unauthorized(errors.New("Access token has been revoked")) }
		scopes := map[string]bool{}; for _, sc := range strings.Split(t.Scopes, ",") { if strings.TrimSpace(sc)!="" { scopes[strings.TrimSpace(sc)] = true } }
		_ = s.repo.UpdateLastUsed(ctx, t.ID)
		if t.Kind=="instance" { return ports.Principal{Kind:ports.PrincipalInstance, TokenID:t.ID, Scopes:scopes}, nil }
		if t.SiteID == nil { return ports.Principal{}, Unauthorized(errors.New("Site token missing site binding")) }
		return ports.Principal{Kind:ports.PrincipalSite, TokenID:t.ID, SiteID:*t.SiteID, Scopes:scopes}, nil
	}
	return ports.Principal{}, Unauthorized(errors.New("Invalid access token"))
}

func defaultScopes(kind string) []string { if kind=="instance" { return []string{ScopeSitesRead,ScopeSitesWrite,ScopeSitesDelete,ScopeMembersRead,ScopeMembersWrite,ScopeTokensRead,ScopeTokensWrite} }; return []string{ScopeSiteRead,ScopeSchemaRead,ScopeSchemaWrite,ScopeContentRead,ScopeContentWrite,ScopeAssetsRead,ScopeAssetsWrite,ScopeTokensRead,ScopeTokensWrite} }
func kindPrefix(kind string) string { if kind=="instance" { return "cms_inst_" }; return "cms_site_" }
func computeHMAC(key, secret string) string { mac := hmac.New(sha256.New, []byte(secret)); mac.Write([]byte(key)); return hex.EncodeToString(mac.Sum(nil)) }
func randomHex(n int) string { b := make([]byte, n); _, _ = rand.Read(b); return hex.EncodeToString(b) }
func mustUUIDv7() string { id, err := uuid.NewV7(); if err != nil { return uuid.NewString() }; return id.String() }
func nowString() string { return time.Now().UTC().Format("2006-01-02 15:04:05") }

var fileIDRE = regexp.MustCompile(`/api/files/([a-f0-9-]+)`)
func ExtractFileIDs(data []byte) []string { ms := fileIDRE.FindAllSubmatch(data, -1); out := []string{}; for _, m := range ms { out = append(out, string(m[1])) }; return out }
var _ = io.EOF
