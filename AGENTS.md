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

The backend binary (`vcms`) is a clap CLI. With no subcommand it runs the server (back-compat with `cargo run`).

```bash
vcms                                   # run the server (alias for `vcms serve`)
vcms serve                             # run the server
vcms config init [--force] [--path P]  # write a default config.toml (non-secrets only)
vcms config show                       # print effective merged config (secrets redacted)
vcms config path                       # print resolved config file + search order
vcms admin reset-password --email U --password P
vcms backup create [--scope instance|site] [--site ID] [--out FILE] [--no-files] [--encrypt]
vcms backup list                       # list recorded backups
vcms restore --file PATH [--scope instance|site] [--site ID] [--import-as-new] --yes
vcms service install [--user NAME]     # install/enable/start as an OS service (systemd/launchd/SCM)
vcms service uninstall|status|start|stop
vcms mcp stdio                         # thin HTTP proxy to a running server's /mcp (for MCP clients)
```

`backup`/`restore` run offline (no HTTP server) against the configured database —
the disaster-recovery path when the instance won't boot. `restore` is destructive
and requires `--yes`.

`mcp stdio` opens no database, secrets, or search index of its own: it forwards
JSON-RPC between stdin/stdout and the server's `/mcp` Streamable-HTTP endpoint,
reading only `VCMS_MCP_TOKEN` (bearer) and `VCMS_MCP_URL` (default
`http://127.0.0.1:3000`). So it works even when the data is owned by the OS-service
account. `vcms service install` pins `VCMS_HOME` to a system dir so the daemon stores
everything under one owned root.

Global flags (highest precedence): `--config <PATH>`, `--bind <ADDR>`, `--database-url <URL>`, `--log-level <LEVEL>`.

The server auto-migrates the database on every startup; there is no separate migrate command.

## Configuration

Non-secret settings live in a TOML config file; secrets stay in the environment (or `.env`).
Layers merge with precedence: **CLI flag > env var > config file > built-in default**.

Config file search order (first existing wins; missing is fine):
1. `--config` flag / `VCMS_CONFIG` env
2. `./vcms.toml` (current dir)
3. the platform config dir (`config.toml`; `$VCMS_HOME/config.toml` in single-dir mode) — where `vcms config init` writes
4. `/etc/vcms/config.toml`

## Data directory

Resolution lives in `apps/backend/src/paths.rs` and has two layouts:

**Split (default, interactive installs)** — files land in the platform-conventional
per-type directories via the `directories` crate (`ProjectDirs`):

| File(s) | Dir | Linux | macOS | Windows |
|---------|-----|-------|-------|---------|
| `config.toml`, `secrets.toml`, `.env` | config | `~/.config/vcms` | `~/Library/Application Support/vcms` | `%APPDATA%\vcms\config` |
| `vcms.db`, `storage/`, `backups/` | data | `~/.local/share/vcms` | `~/Library/Application Support/vcms` | `%APPDATA%\vcms\data` |
| `search/` (derived, rebuildable) | cache | `~/.cache/vcms` | `~/Library/Caches/vcms` | `%LOCALAPPDATA%\vcms\cache` |
| `logs/` | state | `~/.local/state/vcms` | `~/Library/Application Support/vcms` | `%LOCALAPPDATA%\vcms\data` |

(`logs/` uses the state dir where the platform has one — Linux — else the local data dir.)

**Single** — everything nests under one root. Chosen (in precedence order) when:
1. **`$VCMS_HOME` is set** — forces the root explicitly.
2. **the system service home dir exists** — Linux `/var/lib/vcms`, macOS
   `/Library/Application Support/vcms`, Windows `C:\ProgramData\vcms`. The
   `vcms service` installer creates it (and leaves it behind on uninstall), so a plain
   `vcms serve`/`admin`/`backup` **follows the service's data instead of forking to a
   per-user split store**. This path is defined once in `paths::system_home()` and
   imported by the `service` submodules.
3. **a legacy `~/.vcms` exists** — an existing install keeps working untouched.

Otherwise (dev/eval boxes with no service) files use the platform split dirs.

```text
$VCMS_HOME/                # or system home, or ~/.vcms (legacy)
  config.toml secrets.toml .env
  vcms.db (+ -wal / -shm)  logs/  storage/  backups/  search/
```

When the active home is the system service home (owned by SYSTEM/root), the
data-touching commands (`serve`/`admin`/`backup`/`restore`) require elevation — a
non-elevated invocation **fails fast** with an "Administrator/root" hint (in
`paths::ensure`'s preflight) rather than silently forking to a second store.

Secrets: on first `serve`/`admin`, a random `HMAC_SECRET` is generated and persisted to
`secrets.toml` (`apps/backend/src/secrets.rs`), then loaded by the server processes.
`vcms mcp stdio` does **not** load secrets, the database, or any data-dir file: it is a
thin HTTP proxy to the running server's `/mcp` (see CLI), forwarding `VCMS_MCP_TOKEN` as
the bearer; the server owns all disk I/O.

Env-only secrets (never read from `config.toml` by convention, omitted from
`config init`): `DATABASE_URL`, `HMAC_SECRET`, `S3_ACCESS_KEY_ID`,
`S3_SECRET_ACCESS_KEY`, `BACKUP_S3_ACCESS_KEY_ID`, `BACKUP_S3_SECRET_ACCESS_KEY`,
`BACKUP_ENCRYPTION_KEY`. (`HMAC_SECRET` and a random backup encryption
key are auto-persisted to `secrets.toml`; the others remain env-only.)

Sample `config.toml` (generate with `vcms config init`):

```toml
bind_address = "0.0.0.0:3000"
grpc_bind_address = "0.0.0.0:50051"
max_upload_size_mb = 50
cookie_secure = false
session_lifetime_hours = 24
db_max_connections = 10
rate_limit_max_requests = 100
mcp_enabled = true
mcp_allowed_hosts = ["localhost", "127.0.0.1"]

[log]
level = "cms=debug,vcms=debug,tower_http=debug,axum=debug"
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
| `VCMS_CONFIG` | - | Explicit config file path (same as `--config`) |
| `VCMS_HOME` | - | If set, forces single-dir mode: db/config/secrets/logs/storage all nest under this root (else files use the platform split dirs) |
| `VCMS_MCP_TOKEN` | - | `vcms_site_*` access token forwarded by `vcms mcp stdio` as the bearer credential (required for stdio) |
| `VCMS_MCP_URL` | `http://127.0.0.1:3000` | Running server's base URL that `vcms mcp stdio` proxies to (`{url}/mcp`) |
| `DATABASE_URL` | `sqlite://<data dir>/vcms.db` | Database URL: `sqlite:path`, `postgres://...`, `mysql://...` |
| `HMAC_SECRET` | auto | HMAC key for token lookup (required; auto-generated to `secrets.toml`, env overrides) |
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
| `BACKUP_LOCAL_PATH` | `<data dir>/backups` | Local backup dir (when destination is filesystem) |
| `BACKUP_ZSTD_LEVEL` | `12` | zstd compression level for backups |
| `BACKUP_DEFAULT_RETENTION` | `7` | Default "keep last N" for new schedules |
| `BACKUP_S3_BUCKET` / `_REGION` / `_ENDPOINT` / `_PUBLIC_URL` | - | S3 backup destination (non-secret parts) |
| `BACKUP_S3_ACCESS_KEY_ID` | - | S3 backup access key (secret, env-only) |
| `BACKUP_S3_SECRET_ACCESS_KEY` | - | S3 backup secret key (secret, env-only) |
| `BACKUP_ENCRYPTION_KEY` | auto | AES-256 backup key (hex); auto-generated to `secrets.toml` |
| `MAX_UPLOAD_SIZE_MB` | `50` | Max upload size in MB |
| `COOKIE_SECURE` | `false` | Require HTTPS cookies |
| `DB_MAX_CONNECTIONS` | `10` | Max DB connections |
| `DB_MIN_CONNECTIONS` | `2` | Min DB connections |
| `RATE_LIMIT_MAX_REQUESTS` | `100` | Rate limit per window |
| `RATE_LIMIT_WINDOW_SECS` | `60` | Rate limit window |
| `VCMS_ENV` | - | `production` enables production security checks |
| `RUST_LOG` | `cms=debug,vcms=debug,tower_http=debug,axum=debug` | Log filter (`[log] level`) |
| `LOG_OUTPUT` | `stdout` | `stdout` or `file` (`[log] output`) |
| `LOG_FORMAT` | `pretty` | `pretty` or `json` (`[log] format`) |
| `LOG_ANNOTATIONS` | `false` | Include file + line numbers (`[log] annotations`) |
| `LOG_DIR` | `<state dir>/logs` | Log directory when `output = file` (`[log] dir`) |

**Note**: `HMAC_SECRET` is auto-generated and persisted to the config dir's
`secrets.toml` on first run. Set it explicitly via env to override.

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
