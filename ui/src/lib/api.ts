const BASE_URL = "/api/v1";
const AUTH_URL = "/api/auth";

async function getToken(): Promise<string | null> {
  return localStorage.getItem("cms_token");
}

export async function api<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const token = await getToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string>),
  };

  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const res = await fetch(`${BASE_URL}${path}`, {
    ...options,
    headers,
  });

  if (res.status === 401) {
    localStorage.removeItem("cms_token");
    localStorage.removeItem("cms_user");
    window.location.href = "/login";
    throw new Error("Unauthorized");
  }

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Request failed: ${res.status}`);
  }

  if (res.status === 204) {
    return undefined as T;
  }

  return res.json();
}

// Auth API uses unversioned path
export async function authApi<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string>),
  };

  const res = await fetch(`${AUTH_URL}${path}`, {
    ...options,
    headers,
  });

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Request failed: ${res.status}`);
  }

  if (res.status === 204) {
    return undefined as T;
  }

  return res.json();
}

export function setToken(token: string) {
  localStorage.setItem("cms_token", token);
}

export function clearToken() {
  localStorage.removeItem("cms_token");
  localStorage.removeItem("cms_user");
}

// --- Types ---

export interface UserPublic {
  id: string;
  username: string;
  email: string;
}

export interface AuthResponse {
  token: string;
  user: UserPublic;
}

export interface Site {
  id: string;
  name: string;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface SiteWithRole extends Site {
  role: "owner" | "admin" | "editor" | "viewer";
}

export interface SiteMember {
  id: string;
  site_id: string;
  user_id: string;
  username: string;
  email: string;
  role: string;
  created_at: string;
}

export interface Collection {
  id: string;
  site_id: string;
  name: string;
  slug: string;
  definition: string;
  created_at: string;
  updated_at: string;
}

export interface SchemaDefinition {
  fields: ContentField[];
}

export interface ContentField {
  name: string;
  type: string;
  required?: boolean;
  options?: string[];
}

export interface Content {
  id: string;
  site_id: string;
  collection_id: string;
  data: string;
  slug: string;
  status: string;
  created_at: string;
  updated_at: string;
  published_at: string | null;
}

export interface ApiKey {
  id: string;
  site_id: string;
  name: string;
  key_prefix: string;
  permissions: string;
  last_used_at: string | null;
  created_at: string;
  expires_at: string | null;
}

export interface ApiKeyResponse {
  id: string;
  site_id: string;
  name: string;
  key: string;
  key_prefix: string;
  permissions: string;
  created_at: string;
}

export interface Media {
  id: string;
  site_id: string;
  filename: string;
  original_name: string;
  mime_type: string;
  size: number;
  storage_provider: string;
  storage_key: string;
  thumbnail_key: string | null;
  width: number | null;
  height: number | null;
  deleted_at: string | null;
  created_by: string;
  created_at: string;
  url: string;
  thumbnail_url: string | null;
}

export interface MediaListResponse {
  items: Media[];
  total: number;
  page: number;
  per_page: number;
}

export interface MediaReference {
  content_id: string;
  collection_name: string;
  field_name: string;
}

// --- Auth API ---

export async function login(username: string, password: string) {
  return authApi<AuthResponse>("/login", {
    method: "POST",
    body: JSON.stringify({ username, password }),
  });
}

export async function register(
  username: string,
  email: string,
  password: string,
) {
  return authApi<AuthResponse>("/register", {
    method: "POST",
    body: JSON.stringify({ username, email, password }),
  });
}

export async function getMe() {
  return authApi<UserPublic>("/me");
}

// --- Sites API ---

export async function getSites() {
  return api<SiteWithRole[]>("/sites");
}

export async function createSite(data: { name: string }) {
  return api<Site>("/sites", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function getSite(id: string) {
  return api<Site>(`/sites/${id}`);
}

export async function updateSite(id: string, data: { name?: string }) {
  return api<Site>(`/sites/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteSite(id: string) {
  return api<void>(`/sites/${id}`, { method: "DELETE" });
}

export async function getSiteMembers(id: string) {
  return api<SiteMember[]>(`/sites/${id}/members`);
}

export async function inviteMember(
  siteId: string,
  data: { username: string; role: string },
) {
  return api<SiteMember>(`/sites/${siteId}/members`, {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateMemberRole(
  siteId: string,
  userId: string,
  role: string,
) {
  return api<SiteMember>(`/sites/${siteId}/members/${userId}`, {
    method: "PUT",
    body: JSON.stringify({ role }),
  });
}

export async function removeMember(siteId: string, userId: string) {
  return api<void>(`/sites/${siteId}/members/${userId}`, {
    method: "DELETE",
  });
}

// --- API Keys API ---

export async function getApiKeys(siteId: string) {
  return api<ApiKey[]>(`/sites/${siteId}/api-keys`);
}

export async function createApiKey(siteId: string, name: string) {
  return api<ApiKeyResponse>(`/sites/${siteId}/api-keys`, {
    method: "POST",
    body: JSON.stringify({ name }),
  });
}

export async function deleteApiKey(siteId: string, keyId: string) {
  return api<void>(`/sites/${siteId}/api-keys/${keyId}`, {
    method: "DELETE",
  });
}

// --- Collections API (site-scoped) ---

export async function getCollections(siteId: string) {
  return api<Collection[]>(`/sites/${siteId}/collections`);
}

export async function getCollection(siteId: string, slug: string) {
  return api<Collection>(`/sites/${siteId}/collections/${slug}`);
}

export async function createCollection(
  siteId: string,
  data: {
    name: string;
    slug: string;
    definition: SchemaDefinition;
  },
) {
  return api<Collection>(`/sites/${siteId}/collections`, {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateCollection(
  siteId: string,
  slug: string,
  data: { name?: string; slug?: string; definition?: SchemaDefinition },
) {
  return api<Collection>(`/sites/${siteId}/collections/${slug}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteCollection(siteId: string, slug: string) {
  return api<void>(`/sites/${siteId}/collections/${slug}`, {
    method: "DELETE",
  });
}

// --- Content API (site-scoped) ---

export async function getContent(
  siteId: string,
  params: {
    type?: string;
    status?: string;
    search?: string;
  },
) {
  const query = new URLSearchParams();
  if (params.type) query.set("type", params.type);
  if (params.status) query.set("status", params.status);
  if (params.search) query.set("search", params.search);
  const qs = query.toString();
  return api<Content[]>(`/sites/${siteId}/content${qs ? `?${qs}` : ""}`);
}

export async function getContentById(siteId: string, id: string) {
  return api<Content>(`/sites/${siteId}/content/${id}`);
}

export async function createContent(
  siteId: string,
  data: {
    collection_id: string;
    data: Record<string, unknown>;
    slug: string;
  },
) {
  return api<Content>(`/sites/${siteId}/content`, {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateContent(
  siteId: string,
  id: string,
  data: {
    data?: Record<string, unknown>;
    slug?: string;
    status?: string;
  },
) {
  return api<Content>(`/sites/${siteId}/content/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteContent(siteId: string, id: string) {
  return api<void>(`/sites/${siteId}/content/${id}`, { method: "DELETE" });
}

export async function publishContent(siteId: string, id: string) {
  return api<Content>(`/sites/${siteId}/content/${id}/publish`, {
    method: "POST",
  });
}

export async function unpublishContent(siteId: string, id: string) {
  return api<Content>(`/sites/${siteId}/content/${id}/unpublish`, {
    method: "POST",
  });
}

// --- Media API (site-scoped) ---

export async function getMedia(
  siteId: string,
  params: {
    page?: number;
    search?: string;
    type?: string;
  },
) {
  const query = new URLSearchParams();
  if (params.page) query.set("page", String(params.page));
  if (params.search) query.set("search", params.search);
  if (params.type) query.set("type", params.type);
  const qs = query.toString();
  return api<MediaListResponse>(`/sites/${siteId}/media${qs ? `?${qs}` : ""}`);
}

export async function uploadMedia(
  siteId: string,
  file: File,
  provider: "filesystem" | "s3",
): Promise<Media> {
  const token = await getToken();
  const formData = new FormData();
  formData.append("file", file);
  formData.append("storage_provider", provider);

  const res = await fetch(`${BASE_URL}/sites/${siteId}/media`, {
    method: "POST",
    headers: token ? { Authorization: `Bearer ${token}` } : {},
    body: formData,
  });

  if (res.status === 401) {
    localStorage.removeItem("cms_token");
    localStorage.removeItem("cms_user");
    window.location.href = "/login";
    throw new Error("Unauthorized");
  }

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Upload failed: ${res.status}`);
  }

  return res.json();
}

export async function deleteMedia(siteId: string, mediaId: string) {
  return api<{ message: string }>(`/sites/${siteId}/media/${mediaId}`, {
    method: "DELETE",
  });
}

export async function getMediaReferences(siteId: string, mediaId: string) {
  return api<MediaReference[]>(`/sites/${siteId}/media/${mediaId}/references`);
}

export async function restoreMedia(siteId: string, mediaId: string) {
  return api<{ message: string }>(`/sites/${siteId}/media/${mediaId}/restore`, {
    method: "POST",
  });
}
