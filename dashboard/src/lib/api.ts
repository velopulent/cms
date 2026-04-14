const BASE_URL = "/api/v1";
const AUTH_URL = "/api/auth";
const SITE_HEADER = "x-cms-site-id";

export async function api<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string>),
  };

  const res = await fetch(`${BASE_URL}${path}`, {
    ...options,
    headers,
    credentials: "include",
  });

  if (res.status === 401) {
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

export async function siteApi<T>(
  siteId: string,
  path: string,
  options: RequestInit = {},
): Promise<T> {
  return api<T>(path, {
    ...options,
    headers: {
      [SITE_HEADER]: siteId,
      ...(options.headers as Record<string, string>),
    },
  });
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
    credentials: "include",
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

// --- Types ---

export interface UserPublic {
  id: string;
  username: string;
  email: string;
}

export interface AuthResponse {
  user: UserPublic;
}

export interface Site {
  id: string;
  name: string;
  default_storage_provider: string;
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
  is_singleton: boolean;
  singleton_data: string | null;
  created_at: string;
  updated_at: string;
}

export interface SingletonResponse {
  id: string;
  site_id: string;
  name: string;
  slug: string;
  definition: SchemaDefinition;
  data: Record<string, unknown> | null;
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

export interface Entry {
  id: string;
  site_id: string;
  collection_id: string;
  data: string | Record<string, unknown>;
  slug: string;
  status: string;
  created_at: string;
  updated_at: string;
  published_at: string | null;
}

export interface EntryListResponse {
  items: Entry[];
  total: number;
  page: number;
  per_page: number;
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

interface AccessToken {
  id: string;
  kind: string;
  site_id: string | null;
  name: string;
  token_prefix: string;
  scopes: string;
  last_used_at: string | null;
  created_at: string;
  expires_at: string | null;
}

interface AccessTokenResponse {
  id: string;
  kind: string;
  site_id: string | null;
  name: string;
  token: string;
  token_prefix: string;
  scopes: string[];
  created_at: string;
}

function scopesFromPermission(permission?: string) {
  if (permission === "write") {
    return [
      "site:read",
      "schema:read",
      "schema:write",
      "content:read",
      "content:write",
      "assets:read",
      "assets:write",
      "tokens:read",
      "tokens:write",
    ];
  }

  return [
    "site:read",
    "schema:read",
    "content:read",
    "assets:read",
    "tokens:read",
  ];
}

function permissionFromScopes(scopes: string[] | string) {
  const list = Array.isArray(scopes)
    ? scopes
    : scopes.split(",").map((scope) => scope.trim()).filter(Boolean);
  return list.some((scope) => scope.endsWith(":write")) ? "write" : "read";
}

function mapAccessToken(token: AccessToken): ApiKey {
  return {
    id: token.id,
    site_id: token.site_id ?? "",
    name: token.name,
    key_prefix: token.token_prefix,
    permissions: permissionFromScopes(token.scopes),
    last_used_at: token.last_used_at,
    created_at: token.created_at,
    expires_at: token.expires_at,
  };
}

function mapCreatedAccessToken(token: AccessTokenResponse): ApiKeyResponse {
  return {
    id: token.id,
    site_id: token.site_id ?? "",
    name: token.name,
    key: token.token,
    key_prefix: token.token_prefix,
    permissions: permissionFromScopes(token.scopes),
    created_at: token.created_at,
  };
}

export interface FileItem {
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

export interface FileListResponse {
  items: FileItem[];
  total: number;
  page: number;
  per_page: number;
}

export interface FileReference {
  entry_id: string;
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

export async function logoutApi() {
  return authApi<void>("/logout", { method: "POST" });
}

// --- Sites API ---

export async function getSites() {
  return api<SiteWithRole[]>("/sites");
}

export async function createSite(data: {
  name: string;
  default_storage_provider?: string;
}) {
  return api<Site>("/sites", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function getSite(id: string) {
  return api<Site>(`/sites/${id}`);
}

export async function updateSite(
  id: string,
  data: { name?: string; default_storage_provider?: string },
) {
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

// --- API Keys API (site-scoped) ---

export async function getApiKeys(siteId: string) {
  const tokens = await siteApi<AccessToken[]>(siteId, "/site-tokens");
  return tokens.map(mapAccessToken);
}

export async function createApiKey(siteId: string, name: string, permissions?: string) {
  const token = await siteApi<AccessTokenResponse>(siteId, "/site-tokens", {
    method: "POST",
    body: JSON.stringify({ name, scopes: scopesFromPermission(permissions) }),
  });
  return mapCreatedAccessToken(token);
}

export async function deleteApiKey(siteId: string, keyId: string) {
  return siteApi<void>(siteId, `/site-tokens/${keyId}`, {
    method: "DELETE",
  });
}

// --- Collections API (site-scoped) ---

export async function getCollections(siteId: string) {
  return siteApi<Collection[]>(siteId, "/collections");
}

export async function getCollection(siteId: string, slug: string) {
  return siteApi<Collection>(siteId, `/collections/${slug}`);
}

export async function createCollection(
  siteId: string,
  data: {
    name: string;
    slug: string;
    definition: SchemaDefinition;
    is_singleton?: boolean;
  },
) {
  return siteApi<Collection>(siteId, "/collections", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateCollection(
  siteId: string,
  slug: string,
  data: { name?: string; slug?: string; definition?: SchemaDefinition },
) {
  return siteApi<Collection>(siteId, `/collections/${slug}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteCollection(siteId: string, slug: string) {
  return siteApi<void>(siteId, `/collections/${slug}`, {
    method: "DELETE",
  });
}

// --- Entry API (site-scoped) ---

export async function getEntries(
  siteId: string,
  params: {
    type?: string;
    status?: string;
    search?: string;
    page?: number;
    pageSize?: number;
  },
) {
  const query = new URLSearchParams();
  if (params.type) query.set("type", params.type);
  if (params.status) query.set("status", params.status);
  if (params.search) query.set("search", params.search);
  if (params.page) query.set("page", String(params.page));
  if (params.pageSize) query.set("per_page", String(params.pageSize));
  const qs = query.toString();
  return siteApi<EntryListResponse>(siteId, `/entries${qs ? `?${qs}` : ""}`);
}

export async function getEntryById(siteId: string, id: string) {
  return siteApi<Entry>(siteId, `/entries/${id}`);
}

export async function createEntry(
  siteId: string,
  data: {
    collection_id: string;
    data: Record<string, unknown>;
    slug: string;
  },
) {
  return siteApi<Entry>(siteId, "/entries", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateEntry(
  siteId: string,
  id: string,
  data: {
    data?: Record<string, unknown>;
    slug?: string;
    status?: string;
  },
) {
  return siteApi<Entry>(siteId, `/entries/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteEntry(siteId: string, id: string) {
  return siteApi<void>(siteId, `/entries/${id}`, { method: "DELETE" });
}

export async function publishEntry(siteId: string, id: string) {
  return siteApi<Entry>(siteId, `/entries/${id}/publish`, {
    method: "POST",
  });
}

export async function unpublishEntry(siteId: string, id: string) {
  return siteApi<Entry>(siteId, `/entries/${id}/unpublish`, {
    method: "POST",
  });
}

// --- Files API (site-scoped) ---

export async function getFiles(
  siteId: string,
  params: {
    page?: number;
    search?: string;
    type?: string;
    trashed?: boolean;
  },
) {
  const query = new URLSearchParams();
  if (params.page) query.set("page", String(params.page));
  if (params.search) query.set("search", params.search);
  if (params.type) query.set("type", params.type);
  if (params.trashed) query.set("trashed", "true");
  const qs = query.toString();
  return siteApi<FileListResponse>(siteId, `/files${qs ? `?${qs}` : ""}`);
}

export async function uploadFile(
  siteId: string,
  file: File,
  provider: "filesystem" | "s3",
): Promise<FileItem> {
  const formData = new FormData();
  formData.append("file", file);
  formData.append("storage_provider", provider);

  const res = await fetch(`${BASE_URL}/files`, {
    method: "POST",
    credentials: "include",
    headers: {
      [SITE_HEADER]: siteId,
    },
    body: formData,
  });

  if (res.status === 401) {
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

export async function deleteFile(siteId: string, fileId: string) {
  return siteApi<{ message: string }>(siteId, `/files/${fileId}`, {
    method: "DELETE",
  });
}

export async function getFileReferences(siteId: string, fileId: string) {
  return siteApi<FileReference[]>(siteId, `/files/${fileId}/references`);
}

export async function restoreFile(siteId: string, fileId: string) {
  return siteApi<{ message: string }>(siteId, `/files/${fileId}/restore`, {
    method: "POST",
  });
}

export async function batchDeleteFiles(siteId: string, ids: string[]) {
  return siteApi<{ deleted: number }>(siteId, "/files/batch-delete", {
    method: "POST",
    body: JSON.stringify({ ids }),
  });
}

export async function batchRestoreFiles(siteId: string, ids: string[]) {
  return siteApi<{ restored: number }>(siteId, "/files/batch-restore", {
    method: "POST",
    body: JSON.stringify({ ids }),
  });
}

export async function batchPermanentDeleteFiles(siteId: string, ids: string[]) {
  return siteApi<{ deleted: number }>(siteId, "/files/batch-permanent-delete", {
    method: "POST",
    body: JSON.stringify({ ids }),
  });
}

// --- Singletons API (site-scoped) ---

export async function getSingletons(siteId: string) {
  return siteApi<SingletonResponse[]>(siteId, "/singletons");
}

export async function getSingleton(siteId: string, slug: string) {
  return siteApi<SingletonResponse>(siteId, `/singletons/${slug}`);
}

export async function updateSingletonData(
  siteId: string,
  slug: string,
  data: Record<string, unknown>,
) {
  return siteApi<SingletonResponse>(siteId, `/singletons/${slug}`, {
    method: "PUT",
    body: JSON.stringify({ data }),
  });
}
