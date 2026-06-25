const BASE_URL = "/api/dashboard";
const AUTH_URL = "/api/auth";

export class ApiError extends Error {
  status: number;
  body?: Record<string, unknown>;

  constructor(
    status: number,
    message?: string,
    body?: Record<string, unknown>,
  ) {
    super(message);
    this.status = status;
    this.body = body;
  }
}

export async function api<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const csrfToken = getCsrfToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(csrfToken ? { "X-CSRF-Token": csrfToken } : {}),
    ...(options.headers as Record<string, string>),
  };

  const res = await fetch(`${BASE_URL}${path}`, {
    ...options,
    headers,
    credentials: "include",
  });

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new ApiError(res.status, body.error || "Request failed", body);
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
  const csrfToken = getCsrfToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(csrfToken ? { "X-CSRF-Token": csrfToken } : {}),
    ...(options.headers as Record<string, string>),
  };

  const res = await fetch(`${AUTH_URL}${path}`, {
    ...options,
    headers,
    credentials: "include",
  });

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new ApiError(res.status, body.error || "Request failed", body);
  }

  if (res.status === 204) {
    return undefined as T;
  }

  return res.json();
}

// --- Types ---

export type InstanceRole = "instance_owner" | "instance_admin";

export interface UserPublic {
  id: string;
  name: string;
  email: string;
  instance_role: InstanceRole | null;
  must_change_password: boolean;
}

/** The non-operator role, modelled on the frontend as the string "user". */
export const ROLE_USER = "user";
export type RoleValue = InstanceRole | typeof ROLE_USER;

/**
 * value→label items for an instance-role `<Select>`. base-ui's `<SelectValue />`
 * renders the trigger label from this map, so the dropdowns must pass it.
 */
export const INSTANCE_ROLE_ITEMS: { value: RoleValue; label: string }[] = [
  { value: ROLE_USER, label: "User" },
  { value: "instance_admin", label: "Instance admin" },
  { value: "instance_owner", label: "Instance owner" },
];

/** True for instance operators (owner or admin) who manage the whole installation. */
export function isOperator(role: InstanceRole | null | undefined): boolean {
  return role === "instance_owner" || role === "instance_admin";
}

/** Human label for an instance role, used in settings/user lists. */
export function instanceRoleLabel(
  role: InstanceRole | null | undefined,
): string {
  if (role === "instance_owner") return "Instance owner";
  if (role === "instance_admin") return "Instance admin";
  return "User";
}

/** Friendly label for the effective role on a site (collaborator or operator). */
export function siteRoleLabel(
  role: InstanceRole | "editor" | "viewer" | null | undefined,
): string {
  switch (role) {
    case "instance_owner":
      return "Owner";
    case "instance_admin":
      return "Admin";
    case "editor":
      return "Editor";
    case "viewer":
      return "Viewer";
    default:
      return "—";
  }
}

export interface AuthResponse {
  user: UserPublic;
}

export interface Site {
  id: string;
  name: string;
  storage_provider: string;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface SiteWithRole extends Site {
  // For collaborators this is their site role; for instance operators the server
  // synthesizes their instance role (operators have implicit access to every site).
  role: InstanceRole | "editor" | "viewer";
}

export interface SiteMember {
  id: string;
  site_id: string;
  user_id: string;
  name: string;
  email: string;
  role: "editor" | "viewer";
  created_at: string;
}

export interface SessionSummary {
  id: string;
  created_at: string;
  expires_at: string;
  last_seen_at: string;
  current: boolean;
}

function getCsrfToken() {
  if (typeof document === "undefined") return null;
  return (
    document.cookie
      .split(";")
      .map((part) => part.trim())
      .find((part) => part.startsWith("csrf="))
      ?.slice("csrf=".length) ?? null
  );
}

export interface Collection {
  id: string;
  site_id: string;
  name: string;
  slug: string;
  definition: string;
  is_singleton: boolean;
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
  entry_id: string | null;
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
  accept?: string[];
  // --- Per-field config (PocketBase-style). All optional & backward-compatible. ---
  /** Help text shown under the input in the entry editor. */
  help?: string;
  /** Used as the display label when this collection is the target of a relation. */
  presentable?: boolean;
  /** Stored only; no behavior yet (omit from API responses — future). */
  hidden?: boolean;
  /** Text/textarea: min/max character length. */
  min_length?: number;
  max_length?: number;
  /** Text: validation regex the value must match. */
  pattern?: string;
  /** url: domain allow/deny lists (host or subdomain match). */
  except_domains?: string[];
  only_domains?: string[];
  /** rich_text/json: max payload size in bytes. */
  max_size?: number;
  /** rich_text: stored only (strip URL domains). */
  strip_urls_domain?: boolean;
  /** Number: inclusive bounds. */
  min?: number;
  max?: number;
  /** select/file/relation: hold an array of values. */
  multiple?: boolean;
  /** select/file/relation: bounds on number of selected values. */
  min_select?: number;
  max_select?: number;
  /** File: protected flag (stored only). */
  protected?: boolean;
  /** Relation: target collection slug + cascade delete behavior. */
  target_collection?: string;
  cascade_delete?: boolean;
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

export interface EntryRevision {
  id: string;
  entry_id: string;
  revision_number: number;
  data: string | Record<string, unknown>;
  created_by: string | null;
  created_at: string;
  change_summary: string | null;
  diff_from_previous?: Record<string, unknown> | null;
}

export interface EntryListResponse {
  items: Entry[];
  total: number;
  page: number;
  per_page: number;
}

export interface RevisionsListResponse {
  items: EntryRevision[];
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
  site_id: string;
  name: string;
  token_prefix: string;
  permission: "read" | "write";
  last_used_at: string | null;
  created_at: string;
  expires_at: string | null;
}

interface AccessTokenResponse {
  id: string;
  site_id: string;
  name: string;
  token: string;
  token_prefix: string;
  permission: "read" | "write";
  created_at: string;
}

function mapAccessToken(token: AccessToken): ApiKey {
  return {
    id: token.id,
    site_id: token.site_id,
    name: token.name,
    key_prefix: token.token_prefix,
    permissions: token.permission,
    last_used_at: token.last_used_at,
    created_at: token.created_at,
    expires_at: token.expires_at,
  };
}

function mapCreatedAccessToken(token: AccessTokenResponse): ApiKeyResponse {
  return {
    id: token.id,
    site_id: token.site_id,
    name: token.name,
    key: token.token,
    key_prefix: token.token_prefix,
    permissions: token.permission,
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

// --- Webhook Types ---

export interface Webhook {
  id: string;
  site_id: string;
  label: string;
  url: string;
  headers?: Record<string, string>;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface WebhookDeliveryList {
  id: string;
  webhook_id: string;
  status: string;
  status_code: number | null;
  response_body: string | null;
  duration_ms: number | null;
  triggered_by: string;
  triggered_at: string;
}

export interface WebhookDeliveriesResponse {
  items: WebhookDeliveryList[];
  total: number;
  page: number;
  per_page: number;
}

// --- Webhooks API (site-scoped, path-based) ---

export async function getWebhooks(siteId: string) {
  return api<Webhook[]>(`/sites/${siteId}/webhooks`);
}

export async function getWebhook(siteId: string, webhookId: string) {
  return api<Webhook>(`/sites/${siteId}/webhooks/${webhookId}`);
}

export async function createWebhook(
  siteId: string,
  data: {
    label: string;
    url: string;
    headers?: Record<string, string>;
  },
) {
  return api<Webhook>(`/sites/${siteId}/webhooks`, {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateWebhook(
  siteId: string,
  webhookId: string,
  data: {
    label?: string;
    url?: string;
    headers?: Record<string, string>;
  },
) {
  return api<Webhook>(`/sites/${siteId}/webhooks/${webhookId}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteWebhook(siteId: string, webhookId: string) {
  return api<void>(`/sites/${siteId}/webhooks/${webhookId}`, {
    method: "DELETE",
  });
}

export async function triggerWebhook(siteId: string, webhookId: string) {
  return api<WebhookDeliveryList>(
    `/sites/${siteId}/webhooks/${webhookId}/trigger`,
    {
      method: "POST",
    },
  );
}

export async function getWebhookDeliveries(
  siteId: string,
  webhookId: string,
  page?: number,
  perPage?: number,
) {
  const query = new URLSearchParams();
  if (page) query.set("page", String(page));
  if (perPage) query.set("per_page", String(perPage));
  const qs = query.toString();
  return api<WebhookDeliveriesResponse>(
    `/sites/${siteId}/webhooks/${webhookId}/deliveries${qs ? `?${qs}` : ""}`,
  );
}

// --- Auth API ---

export async function login(email: string, password: string) {
  return authApi<AuthResponse>("/login", {
    method: "POST",
    body: JSON.stringify({ email, password }),
  });
}

export async function register(name: string, email: string, password: string) {
  return authApi<AuthResponse>("/register", {
    method: "POST",
    body: JSON.stringify({ name, email, password }),
  });
}

export async function getMe() {
  return authApi<UserPublic>("/me");
}

export async function logoutApi() {
  return authApi<void>("/logout", { method: "POST" });
}

export async function getSessions() {
  return authApi<SessionSummary[]>("/sessions");
}

export async function revokeAllSessions() {
  return authApi<void>("/sessions/revoke-all", { method: "POST" });
}

export async function changePassword(
  currentPassword: string,
  newPassword: string,
) {
  return authApi<void>("/change-password", {
    method: "POST",
    body: JSON.stringify({
      current_password: currentPassword,
      new_password: newPassword,
    }),
  });
}

export async function getInstanceUsers() {
  return api<UserPublic[]>("/instance/users");
}

export async function createManagedUser(data: {
  name: string;
  email: string;
  temporary_password: string;
  instance_role: InstanceRole | null;
}) {
  return api<UserPublic>("/instance/users", {
    method: "POST",
    body: JSON.stringify(data),
  });
}

export async function updateInstanceRole(
  userId: string,
  instanceRole: InstanceRole | null,
) {
  return api<void>(`/instance/users/${userId}/role`, {
    method: "PUT",
    body: JSON.stringify({ instance_role: instanceRole }),
  });
}

export async function updateUser(
  userId: string,
  data: { name: string; email: string },
) {
  return api<void>(`/instance/users/${userId}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteUser(userId: string) {
  return api<void>(`/instance/users/${userId}`, { method: "DELETE" });
}

export async function adminSetUserPassword(
  userId: string,
  newPassword: string,
) {
  return api<void>(`/instance/users/${userId}/password`, {
    method: "POST",
    body: JSON.stringify({ new_password: newPassword }),
  });
}

export async function updateMyProfile(data: { name: string }) {
  return authApi<UserPublic>("/me", {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

// --- Backup & Restore API ---

export type BackupScope =
  | { kind: "instance" }
  | { kind: "site"; siteId: string };

export function backupScopePrefix(scope: BackupScope): string {
  return scope.kind === "instance" ? "/instance" : `/sites/${scope.siteId}`;
}

/** A stable, serializable query-key segment for a backup scope. */
export function backupScopeKey(scope: BackupScope): string {
  return scope.kind === "instance" ? "instance" : `site:${scope.siteId}`;
}

export interface BackupInfo {
  id: string;
  schedule_id: string | null;
  scope: "instance" | "site";
  site_id: string | null;
  status: "pending" | "running" | "success" | "failed";
  schema_version: string | null;
  size_bytes: number;
  file_count: number;
  includes_files: boolean;
  encrypted: boolean;
  checksum: string | null;
  error: string | null;
  created_by: string | null;
  completed_at: string | null;
  created_at: string;
}

export interface BackupSchedule {
  id: string;
  scope: "instance" | "site";
  site_id: string | null;
  cron: string;
  retention_n: number;
  include_files: boolean;
  encrypt: boolean;
  enabled: boolean;
  last_run_at: string | null;
  next_run_at: string | null;
  created_at: string;
}

export interface CreateBackupInput {
  include_files?: boolean;
  encrypt?: boolean;
}

export interface ScheduleInput {
  cron: string;
  retention_n: number;
  include_files: boolean;
  encrypt: boolean;
  enabled: boolean;
}

export interface RestoreInput {
  backup_id?: string;
  destination_key?: string;
  mode?: "instance" | "site";
  site_id?: string;
  /** Sites to restore when mode = "site" (multi-select). Preferred over site_id. */
  site_ids?: string[];
  import_as_new?: boolean;
  confirm: string;
}

/** A site contained in a backup, for the restore pick-list. */
export interface BackupSiteRef {
  id: string;
  name: string | null;
}

/** Result of inspecting a backup before restore: its scope and the sites it holds. */
export interface InspectResult {
  scope: "instance" | "site";
  site_id: string | null;
  sites: BackupSiteRef[];
  /** Set when an uploaded file was staged server-side; pass as destination_key to restore. */
  staging_key: string | null;
}

export async function listBackups(scope: BackupScope) {
  return api<BackupInfo[]>(`${backupScopePrefix(scope)}/backups`);
}

export async function createBackup(
  scope: BackupScope,
  input: CreateBackupInput,
) {
  return api<BackupInfo>(`${backupScopePrefix(scope)}/backups`, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export async function deleteBackup(scope: BackupScope, backupId: string) {
  return api<void>(`${backupScopePrefix(scope)}/backups/${backupId}`, {
    method: "DELETE",
  });
}

/** Same-origin URL for downloading a backup artifact (auth via cookie). */
export function backupDownloadUrl(
  scope: BackupScope,
  backupId: string,
): string {
  return `${BASE_URL}${backupScopePrefix(scope)}/backups/${backupId}/download`;
}

export async function restoreBackup(scope: BackupScope, input: RestoreInput) {
  return api<void>(`${backupScopePrefix(scope)}/restore`, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export async function restoreBackupUpload(
  scope: BackupScope,
  file: File,
  opts: {
    mode?: "instance" | "site";
    site_id?: string;
    import_as_new?: boolean;
    confirm: string;
  },
) {
  const formData = new FormData();
  formData.append("file", file);
  if (opts.mode) formData.append("mode", opts.mode);
  if (opts.site_id) formData.append("site_id", opts.site_id);
  formData.append("import_as_new", opts.import_as_new ? "true" : "false");
  formData.append("confirm", opts.confirm);
  const csrfToken = getCsrfToken();
  const res = await fetch(
    `${BASE_URL}${backupScopePrefix(scope)}/restore/upload`,
    {
      method: "POST",
      credentials: "include",
      body: formData,
      headers: csrfToken ? { "X-CSRF-Token": csrfToken } : undefined,
    },
  );
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new ApiError(
      res.status,
      body.message || body.error || "Restore failed",
      body,
    );
  }
}

/** Inspect a stored backup (by id or key) to list the sites it contains. */
export async function inspectBackup(
  scope: BackupScope,
  input: { backup_id?: string; destination_key?: string },
) {
  return api<InspectResult>(`${backupScopePrefix(scope)}/restore/inspect`, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

/**
 * Inspect an uploaded backup file: lists its sites and stages the bytes
 * server-side (returned as `staging_key`) so the follow-up restore can reference
 * them without re-uploading the file.
 */
export async function inspectBackupUpload(scope: BackupScope, file: File) {
  const formData = new FormData();
  formData.append("file", file);
  const csrfToken = getCsrfToken();
  const res = await fetch(
    `${BASE_URL}${backupScopePrefix(scope)}/restore/inspect/upload`,
    {
      method: "POST",
      credentials: "include",
      body: formData,
      headers: csrfToken ? { "X-CSRF-Token": csrfToken } : undefined,
    },
  );
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new ApiError(
      res.status,
      body.message || body.error || "Could not read backup file",
      body,
    );
  }
  return (await res.json()) as InspectResult;
}

export async function listBackupSchedules(scope: BackupScope) {
  return api<BackupSchedule[]>(`${backupScopePrefix(scope)}/backup-schedules`);
}

export async function createBackupSchedule(
  scope: BackupScope,
  input: ScheduleInput,
) {
  return api<BackupSchedule>(`${backupScopePrefix(scope)}/backup-schedules`, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export async function updateBackupSchedule(
  scope: BackupScope,
  id: string,
  input: ScheduleInput,
) {
  return api<void>(`${backupScopePrefix(scope)}/backup-schedules/${id}`, {
    method: "PUT",
    body: JSON.stringify(input),
  });
}

export async function deleteBackupSchedule(scope: BackupScope, id: string) {
  return api<void>(`${backupScopePrefix(scope)}/backup-schedules/${id}`, {
    method: "DELETE",
  });
}

export async function runBackupSchedule(scope: BackupScope, id: string) {
  return api<BackupInfo>(
    `${backupScopePrefix(scope)}/backup-schedules/${id}/run`,
    {
      method: "POST",
    },
  );
}

/** Rebuild the full-text search index for the given scope (owner/operator). */
export async function reindexSearch(scope: BackupScope) {
  return api<{ reindexed: number }>(
    `${backupScopePrefix(scope)}/search/reindex`,
    { method: "POST" },
  );
}

// --- Sites API ---

export async function getSites() {
  return api<SiteWithRole[]>("/sites");
}

export async function createSite(data: {
  name: string;
  storage_provider?: string;
}) {
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
  data: { email: string; role: string },
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
  const tokens = await api<AccessToken[]>(`/sites/${siteId}/tokens`);
  return tokens.map(mapAccessToken);
}

export async function createApiKey(
  siteId: string,
  name: string,
  permissions?: string,
) {
  const token = await api<AccessTokenResponse>(`/sites/${siteId}/tokens`, {
    method: "POST",
    body: JSON.stringify({ name, permission: permissions ?? "read" }),
  });
  return mapCreatedAccessToken(token);
}

export async function deleteApiKey(siteId: string, keyId: string) {
  return api<void>(`/sites/${siteId}/tokens/${keyId}`, {
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
    is_singleton?: boolean;
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
  return api<EntryListResponse>(
    `/sites/${siteId}/entries${qs ? `?${qs}` : ""}`,
  );
}

export async function getEntryById(siteId: string, id: string) {
  return api<Entry>(`/sites/${siteId}/entries/${id}`);
}

export async function createEntry(
  siteId: string,
  data: {
    collection_id: string;
    data: Record<string, unknown>;
    slug: string;
  },
) {
  return api<Entry>(`/sites/${siteId}/entries`, {
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
    change_summary?: string;
  },
) {
  return api<Entry>(`/sites/${siteId}/entries/${id}`, {
    method: "PUT",
    body: JSON.stringify(data),
  });
}

export async function deleteEntry(siteId: string, id: string) {
  return api<void>(`/sites/${siteId}/entries/${id}`, { method: "DELETE" });
}

export async function publishEntry(siteId: string, id: string) {
  return api<Entry>(`/sites/${siteId}/entries/${id}/publish`, {
    method: "POST",
  });
}

export async function unpublishEntry(siteId: string, id: string) {
  return api<Entry>(`/sites/${siteId}/entries/${id}/unpublish`, {
    method: "POST",
  });
}

export async function getEntryRevisions(
  siteId: string,
  entryId: string,
  params: { page?: number; per_page?: number } = {},
) {
  const query = new URLSearchParams();
  if (params.page) query.set("page", String(params.page));
  if (params.per_page) query.set("per_page", String(params.per_page));
  const qs = query.toString();
  return api<RevisionsListResponse>(
    `/sites/${siteId}/entries/${entryId}/revisions${qs ? `?${qs}` : ""}`,
  );
}

export async function getEntryRevision(
  siteId: string,
  entryId: string,
  revisionNumber: number,
  diff?: boolean,
) {
  const query = new URLSearchParams();
  if (diff) query.set("diff", "true");
  const qs = query.toString();
  return api<EntryRevision>(
    `/sites/${siteId}/entries/${entryId}/revisions/${revisionNumber}${qs ? `?${qs}` : ""}`,
  );
}

export async function restoreEntryRevision(
  siteId: string,
  entryId: string,
  revisionNumber: number,
) {
  return api<Entry>(
    `/sites/${siteId}/entries/${entryId}/revisions/${revisionNumber}/restore`,
    {
      method: "POST",
    },
  );
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
  return api<FileListResponse>(`/sites/${siteId}/files${qs ? `?${qs}` : ""}`);
}

export async function uploadFile(
  siteId: string,
  file: File,
  provider: "filesystem" | "s3",
): Promise<FileItem> {
  const formData = new FormData();
  formData.append("file", file);
  formData.append("storage_provider", provider);
  const csrfToken = getCsrfToken();

  const res = await fetch(`${BASE_URL}/sites/${siteId}/files`, {
    method: "POST",
    credentials: "include",
    body: formData,
    headers: csrfToken ? { "X-CSRF-Token": csrfToken } : undefined,
  });

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new ApiError(res.status, body.error || "Upload failed", body);
  }

  return res.json();
}

export async function deleteFile(siteId: string, fileId: string) {
  return api<{ message: string }>(`/sites/${siteId}/files/${fileId}`, {
    method: "DELETE",
  });
}

export async function getFileReferences(siteId: string, fileId: string) {
  return api<FileReference[]>(`/sites/${siteId}/files/${fileId}/references`);
}

export async function restoreFile(siteId: string, fileId: string) {
  return api<{ message: string }>(`/sites/${siteId}/files/${fileId}/restore`, {
    method: "POST",
  });
}

export async function batchDeleteFiles(siteId: string, ids: string[]) {
  return api<{ deleted: number }>(`/sites/${siteId}/files/batch-delete`, {
    method: "POST",
    body: JSON.stringify({ ids }),
  });
}

export async function batchRestoreFiles(siteId: string, ids: string[]) {
  return api<{ restored: number }>(`/sites/${siteId}/files/batch-restore`, {
    method: "POST",
    body: JSON.stringify({ ids }),
  });
}

export async function batchPermanentDeleteFiles(siteId: string, ids: string[]) {
  return api<{ deleted: number }>(
    `/sites/${siteId}/files/batch-permanent-delete`,
    {
      method: "POST",
      body: JSON.stringify({ ids }),
    },
  );
}

// --- Singletons API (site-scoped) ---

export async function getSingletons(siteId: string) {
  return api<SingletonResponse[]>(`/sites/${siteId}/singletons`);
}

export async function getSingleton(siteId: string, slug: string) {
  return api<SingletonResponse>(`/sites/${siteId}/singletons/${slug}`);
}

export async function updateSingletonData(
  siteId: string,
  slug: string,
  data: Record<string, unknown>,
  changeSummary?: string,
) {
  return api<SingletonResponse>(`/sites/${siteId}/singletons/${slug}`, {
    method: "PUT",
    body: JSON.stringify({ data, change_summary: changeSummary }),
  });
}
