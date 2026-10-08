# AI Agent Instructions for CMS (Rust + React)

## Product Identity

- Human-facing product and service display name: **Velopulent CMS**.
- Canonical documentation: <https://cms.velopulent.com/docs>.
- Keep the executable, package name, native service identifier, environment prefix, and internal identifiers as `vcms` / `VCMS_*`. Do not rename persisted paths or service keys when updating branding.

## Architecture

- **Backend**: Rust + Axum HTTP server, `SQLx` + (SQLite | PostgreSQL), `rust-embed` (static assets)
- **Frontend**: React in `apps/dashboard/` with Tanstack Router, Tanstack Query, shadcn/ui
- **gRPC**: Separate server on port 50051 (compiled from `libs/proto/*.proto` via `tonic-build`)
- **Build**: Nx orchestrates `dashboard:build` → `backend:build`. `build.rs` compiles proto files only.
- **Runtime**: Single binary serves the public REST API (`/api/v1/*`), dashboard API (`/api/dashboard/*`), gRPC, GraphQL (`/api/graphql`), MCP (Streamable HTTP at `/mcp`), health probes (`/health/live`, `/health/ready`), and the static SPA fallback

## Key Directories

- `apps/backend/src/handlers/` - HTTP handlers (auth, content, schema, site, file, UI)
- `apps/backend/src/graphql/` - GraphQL schema and resolvers
- `apps/backend/src/grpc/` - gRPC service implementations (generated from `libs/proto/*.proto`)
- `apps/backend/src/mcp/` - MCP server (`tools/*.rs`, `resources/`, `schema.rs`, `server.rs`, `auth.rs`, `transports/`)
- `apps/backend/src/repository/` - Data access layer (`sqlite/`, `postgres/`, `traits.rs`)
- `apps/backend/src/services/` - Domain services shared by every adapter
- `apps/backend/src/services/backup/` - Backup & restore engine (dump/restore, scheduler, metadata)
- `apps/backend/src/services/search/` - Full-text search engine (Tantivy index over entries)
- `apps/backend/src/router/` - Route composition
- `apps/backend/migrations/{sqlite,postgres}/` - One baseline schema per backend
- `apps/backend/tests/` - Integration tests (`common/`, `rest/`, `graphql/`, `grpc/`, `mcp/`)
- `libs/proto/` - Protocol Buffer definitions (`cms.proto` plus the vendored well-known types it imports)
- `apps/dashboard/` - React frontend app
- `apps/web/` - Landing page and documentation (Next.js + Fumadocs)
- `packaging/` - Native Linux, macOS, Windows, Debian, RPM, and Arch definitions and lifecycle scripts
- `xtask/` - Typed release/package orchestration; platform builders live in separate modules

## Developer Commands

All commands can be run via `bun` from the repository root:

```bash
bun run dev                  # Start backend, dashboard, and web in parallel
bun run dev:backend          # Backend only (no dashboard embed)
bun run dev:dashboard        # Dashboard Vite dev server only
bun run dev:web              # Web app (Next.js) dev server only
bun run run                  # Full backend with embedded dashboard
bun run build                # Build all projects
bun run build:backend        # Release build of backend
bun run build:dashboard      # Production build of dashboard
bun run build:web            # Production build of web app
bun run test                 # Run all Rust tests (SQLite)
bun run test:pg              # Run all Rust tests against Postgres (see docker-compose.test.yml)
bun run test:dashboard       # TypeScript type check
bun run lint                 # Lint all projects
bun run format               # Format all projects
```

CI also runs `cargo fmt --check`, `cargo clippy --no-default-features --all-targets -- -D warnings`, and `bunx biome ci` inside `apps/dashboard` and `apps/web`.

### Testing

- Unit tests live inline in `#[cfg(test)]` modules under `src/`.
- Integration tests in `apps/backend/tests/` are black-box tests against a real server (no internal imports):
  - `tests/common/` — shared infrastructure: `TestServer` (random port, isolated DB, temp storage, seeded admin), gRPC context, auth helpers, fixture builders
  - `tests/rest/`, `tests/graphql/`, `tests/grpc/`, `tests/mcp/` — one binary per protocol; `*contract_tests.rs` cover cross-protocol authorization and versioning rules
  - Each test gets its own server instance (isolated DB + storage)
  - Run one protocol: `cargo test --test rest` (likewise `graphql`, `grpc`, `mcp`)
- Postgres: `docker compose -f docker-compose.test.yml up -d && bun run test:pg`, then `down -v`.

## CLI

The backend binary (`vcms`) is a clap CLI. With no subcommand it prints help.

```bash
vcms serve                             # run portable mode from ./vcms_data
vcms config show                       # print mode, root, bootstrap, and redacted secrets
vcms secrets reset --yes               # replace trust root and invalidate credentials
vcms admin reset-password --email U --password P
vcms backup create [--scope instance|site] [--site ID] [--out FILE] [--no-files] [--encrypt]
vcms backup list                       # list recorded backups
vcms restore --file PATH [--scope instance|site] [--site ID] [--import-as-new] --yes
vcms mcp stdio                         # thin HTTP proxy to a running server's /mcp (for MCP clients)
vcms service status                    # normalized native-service status and manager details
vcms doctor                            # validate config, storage, database, bind ports, and service identity
```

`backup`/`restore` run offline (no HTTP server) against the configured database —
the disaster-recovery path when the instance won't boot. `restore` is destructive
and requires `--yes`.

`mcp stdio` opens no database, secrets, or search index: it forwards JSON-RPC
between stdin/stdout and the server's `/mcp` endpoint, passing the client's MCP
protocol version through. It reads only `VCMS_MCP_TOKEN` (bearer) and
`VCMS_MCP_URL` (default `http://127.0.0.1:3000`), so it works even when the data
is owned by the OS-service account.

The server applies migrations on every startup; there is no separate migrate command.

## Packaging and Releases

- Keep native package definitions and service files in `packaging/`; do not embed them in Rust source or workflow YAML.
- `xtask` orchestrates deterministic staging and packaging. Keep `xtask/src/main.rs` limited to CLI parsing and dispatch, with shared and platform-specific implementation in modules.
- Keep the release workflow thin: build the dashboard, run native build/package jobs, assemble artifacts, attest, and publish.
- Ordinary CI validates packaging templates, `xtask` tests, and deterministic dry-runs. It must not install or mutate host services.
- Release artifacts include portable archives, Debian, RPM, MSI, and macOS PKG packages. Arch is maintained as a package recipe consuming published Linux archives; render it with `xtask arch-render`, not as a fake release archive.
- The package version comes from the release tag with its leading `v` removed. Package formats differ in what they accept, so check prerelease versions against each format before tagging one.
- The stable native service identifier is `vcms`; its human-facing display name is **Velopulent CMS**. Fresh Linux and macOS package installs register/enable the service but do not auto-start it; the Windows MSI installs the service for automatic startup and starts it immediately.

## Configuration

Bootstrap addresses and logging live in strict `config.toml`. The master key,
backup key, and optional database URL live in strict `secrets.toml`. There are no
server env overrides, CLI config overrides, search paths, or `.env` loading.
Owner-managed settings (registration, upload limit, MCP, origins, trusted proxy
headers, storage, backups) and encrypted provider credentials live in the database
and are edited through the dashboard.

## Data directory

Resolution lives in `apps/backend/src/paths.rs`. Mode is selected only by native
service registration. Installed mode uses `/var/lib/vcms`,
`/Library/Application Support/vcms`, or `C:\ProgramData\vcms`. Without a registered
service, portable mode uses `<cwd>/vcms_data`. Detection errors fail closed.

```text
<root>/
  config.toml secrets.toml vcms.db (+ -wal / -shm)
  storage/ backups/ logs/ search/
```

Fresh roots and strict files are auto-created. Existing malformed files are never
overwritten or silently repaired. A missing `secrets.toml` beside an existing
database is fatal. `master_key` derives domain-separated keys for sessions, token
indexing, signed uploads, webhook encryption, and instance-setting encryption.

Sample `config.toml`:

```toml
[server]
http_address = "127.0.0.1:3000"
grpc_address = "127.0.0.1:50051"

[log]
level = "cms=info,vcms=info"
output = "file"
```

## Environment Variables

The server ignores environment configuration. Only the MCP stdio proxy reads
environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `VCMS_MCP_TOKEN` | required | `vcms_site_*` or `vcms_pat_*` token with `mcp.use`, forwarded as bearer auth |
| `VCMS_MCP_URL` | `http://127.0.0.1:3000` | Running server base URL; the proxy posts to `{url}/mcp` |

## Database schema

Each backend has a single baseline migration
(`migrations/{sqlite,postgres}/20260804000000_initial_schema.sql`). Keep both
backends equivalent. Cross-site ownership is enforced with composite foreign keys
(`collections`, `entries`, `files` expose `UNIQUE (id, site_id)`). `entries.version`
is bumped by a trigger on every update and backs optimistic concurrency.

SQLite write transactions must begin with `BEGIN IMMEDIATE`
(`pool.begin_with("BEGIN IMMEDIATE")`): a deferred transaction that reads before it
writes fails its lock upgrade with `SQLITE_BUSY` immediately instead of waiting.

## Proto Compilation

- Proto files in `libs/proto/` are compiled by `apps/backend/build.rs` into `apps/backend/src/grpc/cms/` using `tonic-build`
- Generated code is **not** committed; `build.rs` runs on every `cargo build`
- `cargo:rerun-if-changed=../../libs/proto/` triggers rebuilds

## First-Run Behavior

On initial startup, the server seeds a default admin user (the first user created
is automatically granted the `instance_owner` role). **Login is by email**; the
`name` field is a display name (non-unique, e.g. "John Doe"):
- Email: `admin@cms.local`
- Password: `admin`
- Display name: `admin`
**Change this password immediately in production.**

## Authorization (RBAC)

The access model has two independent tiers. The policy lives in
`models/authorization.rs` (`Authorizer`, `Action`, `InstanceRole`, `SiteRole`) and
is enforced by `middleware/authz.rs`; `require_*_action` helpers live in
`middleware/auth.rs`, and `services/authorization.rs` applies the same policy for
GraphQL, gRPC, and MCP.

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
- `editor` — read everything on the site including drafts, write and publish content, and write files.
- `viewer` — read-only. Dashboard sessions see drafts; viewer tokens cannot preview drafts.
- Everything else on a site (schema, webhooks, API keys, member management, site
  delete) is operator-only.
- Instance operators are never added as site members — they already have full
  access, so inviting one as a member is rejected (`SiteError::CannotInviteOperator`).

**Access tokens** (`vcms_site_*` site keys, `vcms_pat_*` personal tokens) carry
scopes (`models/access_token.rs::TokenScope`): `site.read`, `schema.read`,
`content.read`, `content.preview.read`, `content.write`, `content.publish`,
`files.read`, `files.write`, `mcp.use`. Each action maps to exactly one scope
(`scope_for_action`). Personal tokens additionally need the user's live role on
the site. Tokens never reach dashboard-only actions (`Authorizer::token_hard_denied`).

Member management routes are nested under
`/api/dashboard/sites/{site_id}/members`; the update (PUT) and remove (DELETE)
endpoints take the path param `{member_user_id}` (must match the `MemberPath`
extractor field name).

## Public API contract

Every protocol names the site explicitly: REST paths `/api/v1/sites/{site_id}/…`,
GraphQL `site(id:)` (a namespace whose fields each authorize themselves) and
mutation `siteId`, gRPC `site_id` on every site-scoped request, MCP `site_id` on
every site tool. Reads return published content unless `include_drafts` is set
(`content.preview.read`). Entry and singleton updates accept an expected version
(REST `If-Match` / `expected_version`). Schema, webhook, deployment, member, key,
and backup management are dashboard-only.

Content APIs share an `ApiRateLimiter` (1200 requests/min per client) separate
from the login `RateLimiter`. Never record request headers in tracing spans: skip
`HeaderMap` arguments in `#[instrument]`.

## Backups & Restore

A logical backup/restore subsystem lives in `apps/backend/src/services/backup/`:
- `schema.rs` — table registry + cross-backend dump/restore. Every value is
  normalized to **text** (Postgres casts `::text`/`::int`; restore casts back with
  `::jsonb`/`::timestamptz`/`::bigint`/`::int::boolean`), so a backup is a portable,
  DB-agnostic set of NDJSON rows that restores into either backend.
- `mod.rs` — `BackupService`: a snapshot read (REPEATABLE READ / WAL) dumps tables →
  tar → zstd → optional AES-256-GCM, written to a destination `StorageProvider`.
  Restore is full-replace within the chosen scope in one transaction (site filter,
  user-ref reconciliation, optional id remap for "import as new site"). Restored
  entries get fresh random versions so pre-restore ETags never match.
- `meta.rs` — CRUD for the `backups`, `backup_schedules`, `restore_jobs` tables
  (these tables are **not** part of a backup payload, nor are access tokens or
  `signed_upload_uses`).
- `scheduler.rs` — background poller spawned from `serve` that runs due cron
  schedules and prunes per-schedule retention; `schedule.rs` wraps cron (`croner`).

Scope is `instance` (every site + users/roles; excludes sessions and `secrets.toml`)
or `site` (one self-contained site). Instance routes are **owner-only** under
`/api/dashboard/instance/{backups,restore,backup-schedules}`; site routes are
**operator-only** under `/api/dashboard/sites/{site_id}/{backups,restore,backup-schedules}`.
Restore endpoints require a typed `confirm: "RESTORE"`.

Encryption uses `backup_encryption_key` from `secrets.toml`, kept separate from the
backup destination. The manifest stamps the format + DB migration version; restore
refuses a backup taken on a **newer** schema.

## Full-text search

Entry search uses an embedded [Tantivy](https://github.com/quickwit-oss/tantivy)
index in `apps/backend/src/services/search/`, giving the same ranked, stemmed search
on SQLite and Postgres. The DB stays the source of truth; the index is derived and
fully rebuildable.

- `schema.rs` — fixed schema over `entries.data`: `id`, `site_id`, `collection_id`,
  `status` (exact-match filters), `slug`, and `body` (flattened scalar text,
  English-stemmed, BM25-ranked).
- `mod.rs` — `SearchService`, opened read-write by the server, which owns Tantivy's
  single writer. `rebuild_all`/`rebuild_site` reindex from the DB.
- `queue.rs` — `SearchQueue` over the `search_index_queue` table. Content writes
  enqueue here for durability across crashes.
- `indexer.rs` — the single consumer spawned by the server. Drains the queue in
  batches (present in DB ⇒ upsert, absent ⇒ delete), commits per batch, and sleeps
  until an enqueue rings its in-process `Notify`.

`EntryService` and `SingletonService` enqueue on write; `EntryService::list_entries`
queries the index, so every protocol gets ranked search through its `search`
parameter. When search is disabled or the index cannot open, listing falls back to
SQL `LIKE`. The index builds on startup when empty, rebuilds after a restore, and has
reindex routes: `POST /api/dashboard/instance/search/reindex` and
`POST /api/dashboard/sites/{site_id}/search/reindex`.

Search deliberately has no typo tolerance: Tantivy fuzzy queries score by a constant
and would flatten BM25 ranking (`ranks_frequent_term_higher` guards this). To add it
without losing ranking, change only `search_entries`: either retry with a fuzzy
query when the normal query returns nothing, or always search
`(exact)^high OR (fuzzy)^low`.

## Code Conventions

- **Rust**: Idiomatic `Result`/error handling, `axum` extractors, custom `AppError` enum for HTTP errors
- **React**: Functional components, hooks, Tanstack Query for server state
- **MCP**: The server supports protocol revisions `2025-06-18`, `2025-11-25`, and `2026-07-28` (`supported_protocol_versions` in `mcp/server.rs`). Tool results are JSON objects.
- **MCP tool schemas**: Generated by `rmcp` + `schemars` (Draft 2020-12). `list_tools` post-processes them with `clean_input_schema()` in `mcp/schema.rs` to strip `$schema`/`title`, simplify nullable types, and keep MCP Inspector/Postman compatible.
  - Unit struct param types must use manual `JsonSchema` impl (not derive) returning `{"type":"object","properties":{}}`
  - `serde_json::Value` fields must use `#[schemars(with = "ArbitraryJson")]` to avoid boolean `true` schemas
- **Styling**: Tailwind CSS v4 with `tw-animate-css`; shadcn/ui components
- **Formatting**: Biome (not ESLint/Prettier) for frontend

## Tooling Config

- `rustfmt.toml`: 120 max width, 4 space indent
- `clippy.toml`: cognitive-complexity threshold 30
- `apps/dashboard/biome.json`, `apps/web/biome.json`: 2-space indent, double quotes, organized imports

## Agent Workflow

1. Discover: Check this file, `README.md`, `Cargo.toml`, `nx.json`, and code patterns
2. For new features: Identify handler and model boundaries; change every protocol adapter together
3. For bugfixes: Reproduce with `bun run dev` or `bun run test`
4. If a handler changes: Update frontend data fetching and UI integration
5. If the public contract changes: Update `apps/web/content/docs/api/`
