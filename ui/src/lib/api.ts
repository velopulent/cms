const BASE_URL = "/api";

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

export interface ContentType {
  id: string;
  site_id: string;
  name: string;
  slug: string;
  schema_json: string;
  created_at: string;
  updated_at: string;
}

export interface ContentTypeSchema {
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
  type_id: string;
  data: string;
  slug: string;
  status: string;
  created_at: string;
  updated_at: string;
  published_at: string | null;
}

// --- Auth API ---

export async function login(username: string, password: string) {
  return api<AuthResponse>("/auth/login", {
    method: "POST",
    body: JSON.stringify({ username, password }),
  });
}

export async function register(
  username: string,
  email: string,
  password: string,
) {
  return api<AuthResponse>("/auth/register", {
    method: "POST",
    body: JSON.stringify({ username, email, password }),
  });
}

export async function getMe() {
  return api<UserPublic>("/auth/me");
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

export async function updateSite(
  id: string,
  data: { name?: string },
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

// --- Content Types API (site-scoped) ---

export async function getContentTypes(siteId: string) {
  return api<ContentType[]>(`/sites/${siteId}/content-types`);
}

export async function getContentType(siteId: string, slug: string) {
  return api<ContentType>(`/sites/${siteId}/content-types/${slug}`);
}

export async function createContentType(
  siteId: string,
  data: {
    name: string;
    slug: string;
    schema_json: ContentTypeSchema;
  },
) {
  return api<ContentType>(`/sites/${siteId}/content-types`, {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateContentType(
  siteId: string,
  slug: string,
  data: { name?: string; slug?: string; schema_json?: ContentTypeSchema },
) {
  return api<ContentType>(`/sites/${siteId}/content-types/${slug}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteContentType(siteId: string, slug: string) {
  return api<void>(`/sites/${siteId}/content-types/${slug}`, {
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
    type_id: string;
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
