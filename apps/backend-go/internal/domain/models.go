package domain

type Site struct {
	ID              string `json:"id"`
	Name            string `json:"name"`
	StorageProvider string `json:"storage_provider"`
	CreatedBy       string `json:"created_by"`
	CreatedAt       string `json:"created_at"`
	UpdatedAt       string `json:"updated_at"`
}

type SiteWithRole struct {
	ID              string `json:"id"`
	Name            string `json:"name"`
	StorageProvider string `json:"storage_provider"`
	CreatedBy       string `json:"created_by"`
	CreatedAt       string `json:"created_at"`
	UpdatedAt       string `json:"updated_at"`
	Role            string `json:"role"`
}

type SiteMember struct {
	ID        string `json:"id"`
	SiteID    string `json:"site_id"`
	UserID    string `json:"user_id"`
	Username  string `json:"username"`
	Email     string `json:"email"`
	Role      string `json:"role"`
	CreatedAt string `json:"created_at"`
}

type Collection struct {
	ID            string  `json:"id"`
	SiteID        string  `json:"site_id"`
	Name          string  `json:"name"`
	Slug          string  `json:"slug"`
	Definition    string  `json:"definition"`
	IsSingleton   bool    `json:"is_singleton"`
	SingletonData *string `json:"singleton_data"`
	CreatedAt     string  `json:"created_at"`
	UpdatedAt     string  `json:"updated_at"`
}

type SingletonResponse struct {
	ID         string      `json:"id"`
	SiteID     string      `json:"site_id"`
	Name       string      `json:"name"`
	Slug       string      `json:"slug"`
	Definition interface{} `json:"definition"`
	Data       interface{} `json:"data,omitempty"`
	CreatedAt  string      `json:"created_at"`
	UpdatedAt  string      `json:"updated_at"`
}

type Entry struct {
	ID           string  `json:"id"`
	SiteID       string  `json:"site_id"`
	CollectionID string  `json:"collection_id"`
	Data         string  `json:"data"`
	Slug         string  `json:"slug"`
	Status       string  `json:"status"`
	CreatedAt    string  `json:"created_at"`
	UpdatedAt    string  `json:"updated_at"`
	PublishedAt  *string `json:"published_at"`
}

type EntryRevision struct {
	ID             string  `json:"id"`
	EntryID        string  `json:"entry_id"`
	RevisionNumber int64   `json:"revision_number"`
	Data           string  `json:"data"`
	CreatedBy      *string `json:"created_by"`
	CreatedAt      string  `json:"created_at"`
	ChangeSummary  *string `json:"change_summary"`
}

type EntryRevisionResponse struct {
	ID               string      `json:"id"`
	EntryID          string      `json:"entry_id"`
	RevisionNumber   int64       `json:"revision_number"`
	Data             interface{} `json:"data"`
	CreatedBy        *string     `json:"created_by"`
	CreatedAt        string      `json:"created_at"`
	ChangeSummary    *string     `json:"change_summary"`
	DiffFromPrevious  interface{} `json:"diff_from_previous"`
}

type File struct {
	ID              string  `json:"id"`
	SiteID          string  `json:"site_id"`
	Filename        string  `json:"filename"`
	OriginalName    string  `json:"original_name"`
	MimeType        string  `json:"mime_type"`
	Size            int64   `json:"size"`
	StorageProvider string  `json:"storage_provider"`
	StorageKey      string  `json:"storage_key"`
	ThumbnailKey    *string `json:"thumbnail_key"`
	Width           *int    `json:"width"`
	Height          *int    `json:"height"`
	DeletedAt       *string `json:"deleted_at"`
	CreatedBy       *string `json:"created_by"`
	CreatedAt       string  `json:"created_at"`
}

type FileWithURL struct {
	ID              string  `json:"id"`
	SiteID          string  `json:"site_id"`
	Filename        string  `json:"filename"`
	OriginalName    string  `json:"original_name"`
	MimeType        string  `json:"mime_type"`
	Size            int64   `json:"size"`
	StorageProvider string  `json:"storage_provider"`
	StorageKey      string  `json:"storage_key"`
	ThumbnailKey    *string `json:"thumbnail_key"`
	Width           *int    `json:"width"`
	Height          *int    `json:"height"`
	DeletedAt       *string `json:"deleted_at"`
	CreatedBy       *string `json:"created_by"`
	CreatedAt       string  `json:"created_at"`
	URL             string  `json:"url"`
	ThumbnailURL    *string `json:"thumbnail_url"`
}

type FileReference struct {
	EntryID        string `json:"entry_id"`
	CollectionName string `json:"collection_name"`
	FieldName      string `json:"field_name"`
}

type AccessToken struct {
	ID              string  `json:"id"`
	Kind            string  `json:"kind"`
	SiteID          *string `json:"site_id"`
	Name            string  `json:"name"`
	TokenPrefix     string  `json:"token_prefix"`
	Scopes          string  `json:"scopes"`
	CreatedByUserID *string `json:"created_by_user_id"`
	LastUsedAt      *string `json:"last_used_at"`
	CreatedAt       string  `json:"created_at"`
	ExpiresAt       *string `json:"expires_at"`
	RevokedAt       *string `json:"revoked_at"`
	TokenHMAC       *string `json:"-"`
}

type AccessTokenResponse struct {
	ID          string   `json:"id"`
	Kind        string   `json:"kind"`
	SiteID      *string  `json:"site_id"`
	Name        string   `json:"name"`
	Token       string   `json:"token"`
	TokenPrefix string   `json:"token_prefix"`
	Scopes      []string `json:"scopes"`
	CreatedAt   string   `json:"created_at"`
}

type EntriesListResult struct {
	Items   []Entry `json:"items"`
	Total   int64   `json:"total"`
	Page    int64   `json:"page"`
	PerPage int64   `json:"per_page"`
}

type RevisionsListResult struct {
	Items   []EntryRevision `json:"items"`
	Total   int64           `json:"total"`
	Page    int64           `json:"page"`
	PerPage int64           `json:"per_page"`
}

type FileListResult struct {
	Items   []File `json:"items"`
	Total   int64  `json:"total"`
	Page    int64  `json:"page"`
	PerPage int64  `json:"per_page"`
}
