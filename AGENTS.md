# AI Agent Instructions for CMS (Rust + React)

## Product Identity

- Human-facing product and service display name: **Velopulent CMS**.
- Canonical documentation: <https://cms.velopulent.com/docs>.
- Keep the executable, package name, native service identifier, environment prefix, and internal identifiers as `vcms` / `VCMS_*` for backward compatibility. Do not rename persisted paths or service keys when updating branding.

## Architecture

- **Backend**: Rust + Axum HTTP server, `SQLx` + (SQLite | PostgreSQL), `rust-embed` (static assets)
- **Frontend**: React in `apps/dashboard/` with Tanstack Router, Tanstack Query, shadcn/ui
- **gRPC**: Separate server on port 50051 (compiled from `libs/proto/*.proto` via `tonic-build`)
- **Build**: Nx orchestrates `dashboard:build` → `backend:build`. `build.rs` compiles proto files only.
- **Runtime**: Single binary serves REST API (`/api/*`), gRPC, GraphQL (`/api/graphql`), MCP (Streamable HTTP at `/mcp`), health probes (`/health/live`, `/health/ready`), and static SPA fallback

## Key Directories

- `apps/backend/src/` - Rust backend source
- `apps/backend/src/handlers/` - HTTP handlers (auth, content, schema, site, file, UI)
- `apps/backend/src/graphql/` - GraphQL schema and resolvers
- `apps/backend/src/grpc/` - gRPC service implementations (generated from `libs/proto/*.proto`)
- `apps/backend/src/mcp/` - MCP server (tools/`*.rs`, `schema.rs`, `server.rs`, `auth.rs`, `transports/`)
- `apps/backend/src/repository/` - Data access layer
- `apps/backend/src/services/backup/` - Backup & restore engine (dump/restore, scheduler, metadata)
- `apps/backend/src/router/` - Route composition
- `apps/backend/tests/common/` - Shared integration test infrastructure
- `apps/backend/tests/rest/` - REST API integration tests
- `apps/backend/tests/graphql/` - GraphQL API integration tests
- `apps/backend/tests/grpc/` - gRPC API integration tests
- `libs/proto/` - Protocol Buffer definitions (`cms.proto`)
- `apps/dashboard/` - React frontend app
- `apps/web/` - Landing Page and Documentation (NextJS + Fumadocs)
- `packaging/` - Native Linux, macOS, Windows, Debian, RPM, and Arch definitions and lifecycle scripts
- `xtask/` - Typed release/package orchestration; platform builders live in separate modules

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

The backend binary (`vcms`) is a clap CLI. With no subcommand it prints help.

```bash
vcms                                   # print help
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

`mcp stdio` opens no database, secrets, or search index of its own: it forwards
JSON-RPC between stdin/stdout and the server's `/mcp` Streamable-HTTP endpoint,
reading only `VCMS_MCP_TOKEN` (bearer) and `VCMS_MCP_URL` (default
`http://127.0.0.1:3000`). So it works even when the data is owned by the OS-service
account. The installed service always uses its fixed system root.

The server auto-migrates the database on every startup; there is no separate migrate command.

## Packaging and Releases

- Keep native package definitions and service files in `packaging/`; do not embed them in Rust source or workflow YAML.
- `xtask` orchestrates deterministic staging and packaging. Keep `xtask/src/main.rs` limited to CLI parsing and dispatch, with shared and platform-specific implementation in modules.
- Keep the release workflow thin: build the dashboard, run native build/package jobs, assemble artifacts, attest, and publish.
- Ordinary CI validates packaging templates, `xtask` tests, and deterministic dry-runs. It must not install or mutate host services.
- Release artifacts include portable archives, Debian, RPM, MSI, and macOS PKG packages. Arch is maintained as a package recipe consuming published Linux archives; render it with `xtask arch-render`, not as a fake release archive.
- The stable native service identifier is `vcms`; its human-facing display name is **Velopulent CMS**. Fresh Linux and macOS package installs register/enable the service but do not auto-start it; the Windows MSI installs the service for automatic startup and starts it immediately.

## Configuration

Bootstrap addresses and logging live in strict `config.toml`. The master key,
backup key, and optional database URL live in strict `secrets.toml`. There are no
server env overrides, CLI config overrides, search paths, or `.env` loading.

## Data directory

Mode is selected only by native service registration. Installed mode uses
`/var/lib/vcms`, `/Library/Application Support/vcms`, or `C:\ProgramData\vcms`.
Without a registered service, portable mode uses `<cwd>/vcms_data`. Detection
errors fail closed; directory existence and legacy paths never select a mode.

Both modes share one root layout:

```text
<root>/
  config.toml secrets.toml vcms.db
  storage/ backups/ logs/ search/
```

Fresh roots and strict files are auto-created. Existing malformed files are never
overwritten or silently repaired. A missing `secrets.toml` beside an existing
database is fatal.

Sample `config.toml`:

```toml
[server]
http_address = "127.0.0.1:3000"
grpc_address = "127.0.0.1:50051"

[log]
level = "cms=info,vcms=info"
output = "file"
```

`secrets.toml` contains `master_key`, `backup_encryption_key`, and optional
`database_url`. Owner settings and encrypted integration credentials live in the
database. Fixed filesystem paths and internal tuning constants are not configurable.

## Environment Variables

The server ignores environment configuration. Only the diskless MCP stdio client
reads environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `VCMS_MCP_TOKEN` | required | Site access token forwarded as bearer auth |
| `VCMS_MCP_URL` | `http://127.0.0.1:3000` | Running server base URL |

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
  DB-agnostic set of NDJSON rows that restores into either backend.
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
