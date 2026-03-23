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
  id: number;
  username: string;
  email: string;
}

export interface AuthResponse {
  token: string;
  user: UserPublic;
}

export interface ContentType {
  id: number;
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
  id: number;
  type_id: number;
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

// --- Content Types API ---

export async function getContentTypes() {
  return api<ContentType[]>("/content-types");
}

export async function getContentType(slug: string) {
  return api<ContentType>(`/content-types/${slug}`);
}

export async function createContentType(data: {
  name: string;
  slug: string;
  schema_json: ContentTypeSchema;
}) {
  return api<ContentType>("/content-types", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateContentType(
  id: number,
  data: { name?: string; slug?: string; schema_json?: ContentTypeSchema },
) {
  return api<ContentType>(`/content-types/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteContentType(id: number) {
  return api<void>(`/content-types/${id}`, { method: "DELETE" });
}

// --- Content API ---

export async function getContent(params: {
  type?: string;
  status?: string;
  search?: string;
}) {
  const query = new URLSearchParams();
  if (params.type) query.set("type", params.type);
  if (params.status) query.set("status", params.status);
  if (params.search) query.set("search", params.search);
  const qs = query.toString();
  return api<Content[]>(`/content${qs ? `?${qs}` : ""}`);
}

export async function getContentById(id: number) {
  return api<Content>(`/content/${id}`);
}

export async function createContent(data: {
  type_id: number;
  data: Record<string, unknown>;
  slug: string;
}) {
  return api<Content>("/content", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateContent(
  id: number,
  data: {
    data?: Record<string, unknown>;
    slug?: string;
    status?: string;
  },
) {
  return api<Content>(`/content/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteContent(id: number) {
  return api<void>(`/content/${id}`, { method: "DELETE" });
}

export async function publishContent(id: number) {
  return api<Content>(`/content/${id}/publish`, { method: "POST" });
}

export async function unpublishContent(id: number) {
  return api<Content>(`/content/${id}/unpublish`, { method: "POST" });
}
