# AI Agent Instructions for CMS (Rust + React)

## Architecture

- **Backend**: Rust + Axum HTTP server, `SQLx` + (SQLite | PostgreSQL | MySQL), `rust-embed` (static assets)
- **Frontend**: React in `apps/dashboard/` with Tanstack Router, Tanstack Query, shadcn/ui
- **gRPC**: Separate server on port 50051 (compiled from `libs/proto/*.proto` via `tonic-build`)
- **Build**: Nx orchestrates `dashboard:build` → `backend:build`. `build.rs` compiles proto files only.
- **Runtime**: Single binary serves REST API (`/api/*`), gRPC, GraphQL (`/api/graphql`), MCP (Streamable HTTP at `/mcp`), and static SPA fallback

## Key Directories

- `apps/backend/src/` - Rust backend source
- `apps/backend/src/handlers/` - HTTP handlers (auth, content, schema, site, file, UI)
- `apps/backend/src/graphql/` - GraphQL schema and resolvers
- `apps/backend/src/grpc/` - gRPC service implementations (generated from `libs/proto/*.proto`)
- `apps/backend/src/mcp/` - MCP server (tools/`*.rs`, `schema.rs`, `server.rs`, `auth.rs`, `transports/`)
- `apps/backend/src/repository/` - Data access layer
- `apps/backend/src/services/backup/` - Backup & restore engine (dump/restore, scheduler, metadata)
- `apps/backend/src/services/search/` - Full-text search engine (Tantivy index over entries)
- `apps/backend/src/router/` - Route composition
- `apps/backend/tests/common/` - Shared integration test infrastructure
- `apps/backend/tests/rest/` - REST API integration tests
- `apps/backend/tests/graphql/` - GraphQL API integration tests
- `apps/backend/tests/grpc/` - gRPC API integration tests
- `libs/proto/` - Protocol Buffer definitions (`cms.proto`)
- `apps/dashboard/` - React frontend app
- `apps/web/` - Landing Page and Documentation (NextJS + Fumadocs)

## Developer Commands

All commands can be run via `bun` from the repository root:

```bash
bun run dev                  # Start backend, dashboard, and web in parallel
bun run dev:backend          # Backend only (no dashboard embed)
bun run dev:dashboard        # Dashboard Vite dev server only
bun run dev:web              # Web app (NextJS) dev server only
bun run run                  # Full backend with embedded dashboard
bun run build                # Build all projects
bun run build:backend        # Release build of backend
bun run build:dashboard      # Production build of dashboard
bun run build:web            # Production build of web app
bun run test                 # Run all Rust tests
bun run test:dashboard       # TypeScript type check
bun run lint                 # Lint all projects
bun run format               # Format all projects
```

### Testing
- Rust unit tests: `bun run test` (runs `#[cfg(test)]` modules in `src/`)
- Integration tests: `cargo test --test rest` from `apps/backend/` (runs HTTP-level API tests)
- Unit tests live inline in source files (19 modules) and in `apps/backend/tests/mock_user_repository.rs`, `apps/backend/tests/file_service_tests.rs`
- Integration tests in `apps/backend/tests/` are black-box tests against a real server (no internal imports)
  - `tests/common/` — shared infrastructure: `TestServer` (random port, SQLite in-memory, temp storage, seeded admin), auth helpers, `TestClient` wrapper, fixture builders
  - `tests/rest/` — REST API tests: `auth`, `sites`, `collections`, `entries`, `singletons`, `files`, `webhooks`, `access_tokens`, `roles`, `backups`
  - `tests/graphql/` — GraphQL API tests: `auth`, `sites`, `collections`, `entries`, `files`, `webhooks`
  - `tests/grpc/` — gRPC API tests: `collections`, `entries`, `singletons`, `files`, `sites`, `webhooks` (uses tonic clients with real auth interceptor)
  - Each test module gets its own server instance (isolated DB + storage)
  - Tests communicate only via HTTP using `reqwest` (REST/GraphQL) or tonic clients (gRPC)
  - Run REST: `cargo test --test rest -- --test-threads=1`
  - Run GraphQL: `cargo test --test graphql -- --test-threads=1`
  - Run gRPC: `cargo test --test grpc -- --test-threads=1`

## CLI

The backend binary (`cms`) is a clap CLI. With no subcommand it runs the server (back-compat with `cargo run`).

```bash
cms                                   # run the server (alias for `cms serve`)
cms serve                             # run the server
cms config init [--force] [--path P]  # write a default config.toml (non-secrets only)
cms config show                       # print effective merged config (secrets redacted)
cms config path                       # print resolved config file + search order
cms admin reset-password --username U --password P
cms backup create [--scope instance|site] [--site ID] [--out FILE] [--no-files] [--encrypt]
cms backup list                       # list recorded backups
cms restore --file PATH [--scope instance|site] [--site ID] [--import-as-new] --yes
```

`backup`/`restore` run offline (no HTTP server) against the configured database —
the disaster-recovery path when the instance won't boot. `restore` is destructive
and requires `--yes`.

Global flags (highest precedence): `--config <PATH>`, `--bind <ADDR>`, `--database-url <URL>`, `--log-level <LEVEL>`.

The server auto-migrates the database on every startup; there is no separate migrate command.

## Configuration

Non-secret settings live in a TOML config file; secrets stay in the environment (or `.env`).
Layers merge with precedence: **CLI flag > env var > config file > built-in default**.

Config file search order (first existing wins; missing is fine):
1. `--config` flag / `CMS_CONFIG` env
2. `./cms.toml` (current dir)
3. `~/.cms/config.toml` (CMS home; `$CMS_HOME/config.toml` if set) — where `cms config init` writes
4. `/etc/cms/config.toml`

## Data directory (CMS home)

All runtime files live under one home directory: `$CMS_HOME` if set, else `~/.cms`
(same layout on Windows, macOS, Linux via the `directories` crate). `cms serve`
creates it on first run. Resolution lives in `apps/backend/src/paths.rs`.

```text
~/.cms/
  config.toml     # non-secret config (cms config init target)
  secrets.toml    # auto-generated JWT_SECRET/HMAC_SECRET (0600 on unix)
  cms.db          # default SQLite database (+ -wal / -shm)
  logs/           # rolling logs when [log] output = "file"
  storage/        # default filesystem storage for uploads
  search/         # Tantivy full-text search index (derived; rebuildable)
```

Secrets: on first `serve`/`admin`, random `JWT_SECRET`/`HMAC_SECRET` are generated
and persisted to `secrets.toml` (`apps/backend/src/secrets.rs`), then loaded by
every process — including `cms mcp stdio`, which is launched from an arbitrary cwd
and so cannot rely on a cwd `.env`. Env vars still override the file. `mcp stdio`
is read-only: it never creates the home dir, database, or secrets file.

Env-only secrets (never read from `config.toml` by convention, omitted from
`config init`): `DATABASE_URL`, `JWT_SECRET`, `HMAC_SECRET`, `S3_ACCESS_KEY_ID`,
`S3_SECRET_ACCESS_KEY`, `BACKUP_S3_ACCESS_KEY_ID`, `BACKUP_S3_SECRET_ACCESS_KEY`,
`BACKUP_ENCRYPTION_KEY`. (`JWT_SECRET`/`HMAC_SECRET` and a random backup encryption
key are auto-persisted to `secrets.toml`; the others remain env-only.)

Sample `config.toml` (generate with `cms config init`):

```toml
bind_address = "0.0.0.0:3000"
grpc_bind_address = "0.0.0.0:50051"
max_upload_size_mb = 50
cookie_secure = false
session_lifetime_hours = 24
db_max_connections = 10
rate_limit_max_requests = 100
search_enabled = true
mcp_enabled = true
mcp_allowed_hosts = ["localhost", "127.0.0.1"]

[log]
level = "cms=debug,tower_http=debug,axum=debug"
output = "stdout"   # stdout | file
format = "pretty"   # pretty | json
annotations = false
dir = "logs"
```

## Environment Variables

Every non-secret key below can also be set in `config.toml` (env still overrides the file).
Logging keys map to the `[log]` table: `RUST_LOG`→`log.level`, `LOG_OUTPUT`→`log.output`,
`LOG_FORMAT`→`log.format`, `LOG_ANNOTATIONS`→`log.annotations`, `LOG_DIR`→`log.dir`.

| Variable | Default | Description |
|----------|---------|-------------|
| `CMS_CONFIG` | - | Explicit config file path (same as `--config`) |
| `CMS_HOME` | `~/.cms` | CMS home directory (db, config, secrets, logs, storage) |
| `DATABASE_URL` | `sqlite://~/.cms/cms.db` | Database URL: `sqlite:path`, `postgres://...`, `mysql://...` |
| `JWT_SECRET` | `cms-jwt-secret-change-in-production` | JWT signing secret |
| `HMAC_SECRET` | `cms-hmac-secret-change-in-production` | HMAC key for token lookup |
| `BIND_ADDRESS` | `0.0.0.0:3000` | REST API listen address |
| `GRPC_BIND_ADDRESS` | `0.0.0.0:50051` | gRPC server listen address |
| `STORAGE_FS_PATH` | - | Filesystem storage path |
| `S3_ACCESS_KEY_ID` | - | S3 access key |
| `S3_SECRET_ACCESS_KEY` | - | S3 secret key |
| `S3_BUCKET` | - | S3 bucket name |
| `S3_REGION` | `us-east-1` | S3 region |
| `S3_ENDPOINT` | - | S3 endpoint (for S3-compatible services) |
| `S3_PUBLIC_URL` | - | Public URL for S3 assets |
| `BACKUP_ENABLED` | `true` | Run the scheduled-backup poller / allow backups |
| `BACKUP_DESTINATION` | `filesystem` | Backup destination: `filesystem` or `s3` |
| `BACKUP_LOCAL_PATH` | `~/.cms/backups` | Local backup dir (when destination is filesystem) |
| `BACKUP_ZSTD_LEVEL` | `12` | zstd compression level for backups |
| `BACKUP_DEFAULT_RETENTION` | `7` | Default "keep last N" for new schedules |
| `BACKUP_S3_BUCKET` / `_REGION` / `_ENDPOINT` / `_PUBLIC_URL` | - | S3 backup destination (non-secret parts) |
| `BACKUP_S3_ACCESS_KEY_ID` | - | S3 backup access key (secret, env-only) |
| `BACKUP_S3_SECRET_ACCESS_KEY` | - | S3 backup secret key (secret, env-only) |
| `BACKUP_ENCRYPTION_KEY` | auto | AES-256 backup key (hex); auto-generated to `secrets.toml` |
| `SEARCH_ENABLED` | `true` | Build/use the Tantivy full-text index for entry search (else SQL `LIKE`) |
| `SEARCH_INDEX_PATH` | `~/.cms/search` | Directory for the Tantivy search index |
| `MAX_UPLOAD_SIZE_MB` | `50` | Max upload size in MB |
| `COOKIE_SECURE` | `false` | Require HTTPS cookies |
| `DB_MAX_CONNECTIONS` | `10` | Max DB connections |
| `DB_MIN_CONNECTIONS` | `2` | Min DB connections |
| `RATE_LIMIT_MAX_REQUESTS` | `100` | Rate limit per window |
| `RATE_LIMIT_WINDOW_SECS` | `60` | Rate limit window |
| `CMS_ENV` | - | `production` enables production security checks |
| `RUST_LOG` | `cms=debug,tower_http=debug,axum=debug` | Log filter (`[log] level`) |
| `LOG_OUTPUT` | `stdout` | `stdout` or `file` (`[log] output`) |
| `LOG_FORMAT` | `pretty` | `pretty` or `json` (`[log] format`) |
| `LOG_ANNOTATIONS` | `false` | Include file + line numbers (`[log] annotations`) |
| `LOG_DIR` | `~/.cms/logs` | Log directory when `output = file` (`[log] dir`) |

**Note**: `JWT_SECRET`/`HMAC_SECRET` are auto-generated and persisted to
`~/.cms/secrets.toml` on first run. Set them explicitly via env to override.

## Proto Compilation

- Proto files in `libs/proto/` are compiled by `apps/backend/build.rs` into `apps/backend/src/grpc/cms/` using `tonic-build`
- Generated code is **not** committed; `build.rs` runs on every `cargo build`
- `cargo:rerun-if-changed=../../libs/proto/` triggers rebuilds

## First-Run Behavior

On initial startup, the server seeds a default admin user (the first user created
is automatically granted the `instance_owner` role):
- Username: `admin`
- Password: `admin`
**Change this password immediately in production.**

## Authorization (RBAC)

The access model has two independent tiers. The policy lives in
`models/authorization.rs` (`Authorizer`, `Action`, `InstanceRole`, `SiteRole`) and
is enforced by `middleware/authz.rs`; `require_*_action` helpers live in
`middleware/auth.rs`.

**Instance roles** (operators, span the whole installation; stored on
`users.instance_role`):
- `instance_owner` — strict superset of admin. Owner-only powers: granting/revoking
  instance roles (`InstanceRolesGrant`); instance-wide backup/restore
  (`InstanceBackup`/`InstanceRestore`).
- `instance_admin` — manage the instance and its users, create and delete sites,
  and back up / restore individual sites (`SiteBackup`/`SiteRestore`).
- A user with no instance role has no instance-level powers.

Both operators have **implicit full authority over every site** without being a
site member (the `allows_site_as_instance` override).

**Site roles** (collaborators, per-site; stored in `site_members.role`):
- `editor` — read everything on the site plus write content and files.
- `viewer` — read-only.
- Everything else on a site (schema, webhooks, API keys, member management, site
  delete) is operator-only.
- Instance operators are never added as site members — they already have full
  access, so inviting one as a member is rejected (`SiteError::CannotInviteOperator`).

**API keys** (site-scoped tokens): reads are always allowed; writes (content,
schema, files, webhooks, site manage) are gated by the key's `can_write` flag. Keys
never receive member or API-key management authority.

Member management routes are nested under
`/api/dashboard/sites/{site_id}/members`; the update (PUT) and remove (DELETE)
endpoints take the path param `{member_user_id}` (must match the `MemberPath`
extractor field name).

The `roles_v2` migration (`migrations/*/20260616000000_roles_v2.sql`) widened
`users.instance_role` to allow `instance_admin` and restricted `site_members.role`
to `editor`/`viewer` (legacy `owner`/`admin` site members collapse to `editor`,
since those operators now act through their instance role).

## Backups & Restore

A logical backup/restore subsystem lives in `apps/backend/src/services/backup/`:
- `schema.rs` — table registry + cross-backend dump/restore. Every value is
  normalized to **text** (Postgres casts `::text`/`::int`; restore casts back with
  `::jsonb`/`::timestamptz`/`::bigint`/`::int::boolean`), so a backup is a portable,
  DB-agnostic set of NDJSON rows that restores into any of the three backends.
- `mod.rs` — `BackupService`: a snapshot read (REPEATABLE READ / WAL) dumps tables →
  tar → zstd → optional AES-256-GCM, written to a destination `StorageProvider`.
  Restore is full-replace within the chosen scope in one transaction (site filter,
  user-ref reconciliation, optional id remap for "import as new site").
- `meta.rs` — CRUD for the `backups`, `backup_schedules`, `restore_jobs` tables
  (migration `migrations/*/20260617000000_backups.sql`; these tables are **not**
  part of a backup payload).
- `scheduler.rs` — background poller spawned from `serve` that runs due cron
  schedules and prunes per-schedule retention; `schedule.rs` wraps cron (`croner`).

Scope is `instance` (every site + users/roles; excludes sessions and `secrets.toml`)
or `site` (one self-contained site). REST handlers are in
`handlers/backup_handler.rs`, routes in `router/backup.rs`: instance routes are
**owner-only** under `/api/dashboard/instance/{backups,restore,backup-schedules}`;
site routes are **operator-only** under
`/api/dashboard/sites/{site_id}/{backups,restore,backup-schedules}`. Restore
endpoints require a typed `confirm: "RESTORE"`. The dashboard exposes a **Backups**
tab in both site and instance settings (`components/backups/backups-section.tsx`).

Encryption is optional AES-256-GCM using a key from `BACKUP_ENCRYPTION_KEY` (else a
random key auto-persisted to `secrets.toml`), kept separate from the backup
destination. The manifest stamps the format + DB migration version; restore refuses
a backup taken on a **newer** schema and relies on additive migrations otherwise.

## Full-text search

Entry search is backed by an embedded [Tantivy](https://github.com/quickwit-oss/tantivy)
index in `apps/backend/src/services/search/` — one engine that gives ranked,
tokenized, stemmed search identically across SQLite/Postgres/MySQL (native per-DB
FTS would mean three implementations). The DB stays the source of truth; the index
is **derived** and fully rebuildable.

**Reads vs writes are split** so the single-writer limit never blocks *searching*:

- `schema.rs` — fixed schema over the schema-less `entries.data`: `id`, `site_id`,
  `collection_id`, `status` (exact-match filters), `slug`, and `body` (the flattened
  scalar text of `data`, English-stemmed, BM25-ranked). `fields_from` re-resolves the
  schema when opening an existing index read-only.
- `mod.rs` — `SearchService`. **Reading** needs no lock: `open_read_only` (reader
  only) lets *any* process search the index, including a separate `cms mcp stdio`
  running next to the server. **Writing** requires the directory lock: `open`
  (reader + writer) is held only by the running server. `index_doc`/`delete_doc`
  stage uncommitted ops; `commit` flushes; `rebuild_all`/`rebuild_site` reindex from
  the DB by reusing `EntryRepository::list` (covers singletons too). Write/commit on
  a read-only instance returns `SearchError::ReadOnly`.
- `queue.rs` — `SearchQueue` over the `search_index_queue` table (migration
  `20260618000000`). Content writes from *any* process enqueue here
  (`enqueue`/`dequeue_batch`/`delete_ids`); UUIDv7 ids order the queue chronologically.
- `indexer.rs` — the **single consumer**, spawned once by the server (it owns the
  writer). Drains the queue in batches (present in DB ⇒ upsert doc, absent ⇒ delete
  doc — `op` is advisory), commits per batch, deletes processed rows. Wakes instantly
  on a local enqueue (`Notify`) and polls every 2s to catch other processes' enqueues.

Wiring: `Services` holds `search: Option<Arc<SearchService>>` (reads) and
`search_queue: Option<Arc<SearchQueue>>` (writes). `Services::new` opens the index
read-write (server); `Services::new_read_only` opens it read-only (`cms mcp stdio`).
`EntryService`/`SingletonService` **enqueue** on write; `EntryService::list_entries`
queries the index — so REST, GraphQL, gRPC, and MCP all get ranked search via the
existing `search` param with **no handler changes**. Indexing is asynchronous
(enqueue → indexer), typically sub-second. When search is disabled
(`SEARCH_ENABLED=false`) or the index can't open, it falls back to SQL `LIKE`. The
index builds on startup when empty, rebuilds after a restore, and exposes
owner/operator reindex routes: `POST /api/dashboard/instance/search/reindex` and
`POST /api/dashboard/sites/{site_id}/search/reindex`.

This is the cross-process model: writes from `cms mcp stdio` (or any process) land in
the durable queue and the running server indexes them; if the server is down they
drain on its next start. The remaining hard limit is *concurrent writers* — only one
process indexes at a time. Typo tolerance is a deliberate follow-up (Tantivy fuzzy
queries score by constant, which flattens ranking).

## Code Conventions

- **Rust**: Idiomatic `Result`/error handling, `axum` extractors, custom `AppError` enum for HTTP errors
- **React**: Functional components, hooks, Tanstack Query for server state
- **MCP tool schemas**: Generated by `rmcp` + `schemars` (Draft 2020-12). Post-processed via `clean_input_schema()` in `mcp/schema.rs` to strip `$schema`/`title`, simplify nullable types, and ensure MCP Inspector/Postman compatibility.
  - Unit struct param types must use manual `JsonSchema` impl (not derive) returning `{"type":"object","properties":{}}`
  - `serde_json::Value` fields must use `#[schemars(with = "ArbitraryJson")]` to avoid boolean `true` schemas
  - Protocol version echoing is handled in the `initialize()` override in `mcp/server.rs`
- **Styling**: Tailwind CSS v4 with `tw-animate-css`; shadcn/ui components
- **Formatting**: Biome (not ESLint/Prettier) for frontend

## Tooling Config

- `rustfmt.toml`: 120 max width, 4 space indent
- `clippy.toml`: cognitive-complexity threshold 30
- `biome.json`: 2-space indent, double quotes, organized imports

## Agent Workflow

1. Discover: Check this file, `README.md`, `Cargo.toml`, `nx.json`, and code patterns
2. For new features: Identify handler and model boundaries; keep API stable
3. For bugfixes: Reproduce with `bun run dev` or `bun run test`
4. If a handler changes: Update frontend data fetching and UI integration
