# AI Agent Instructions for CMS (Rust + React)

## Architecture

- **Backend**: Rust + Axum HTTP server, `SQLx` + (SQLite | PostgreSQL), `rust-embed` (static assets)
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

The backend binary (`vcms`) is a clap CLI. With no subcommand it prints help.

```bash
vcms serve                             # run portable mode; refuse when native service is installed
vcms config show                       # print mode, root, bootstrap, secret presence, DB settings
vcms secrets reset                     # destructive trust-root reset and recovery report
vcms admin reset-password --email U --password P
vcms backup create [--scope instance|site] [--site ID] [--out FILE] [--no-files] [--encrypt]
vcms backup list                       # list recorded backups
vcms restore --file PATH [--scope instance|site] [--site ID] [--import-as-new] --yes
vcms mcp stdio                         # diskless proxy to a running server
vcms service status
vcms doctor
```

`backup`/`restore` run offline (no HTTP server) against the configured database —
the disaster-recovery path when the instance won't boot. `restore` is destructive
and requires `--yes`.

The server auto-migrates the database on every startup; there is no separate migrate command.

## Configuration

Bootstrap settings live only in strict `<root>/config.toml`; trust-root secrets and
the optional database URL live only in strict `<root>/secrets.toml`. There is no
server environment or CLI override layer.

## Data directory

Resolution lives in `apps/backend/src/paths.rs`. Native service registration selects
installed mode and its fixed OS root. Without a registered service, portable mode
uses `<current working directory>/vcms_data`. Detection failures stop startup.

```text
<root>/
  config.toml secrets.toml
  vcms.db (+ -wal / -shm)  logs/  storage/  backups/  search/
```

Installed roots are `/var/lib/vcms`, `/Library/Application Support/vcms`, and
`C:\ProgramData\vcms`. `vcms mcp stdio` creates nothing.

Fresh roots atomically create a master key and backup encryption key in
`secrets.toml`. An existing database without its secrets file is a hard error.
Domain-separated keys are derived for token indexing, webhook encryption, and
instance-setting encryption. Owner-managed settings and provider credentials live
in the database.

Bootstrap `config.toml`:

```toml
[server]
http_address = "127.0.0.1:3000"
grpc_address = "127.0.0.1:50051"

[log]
level = "cms=info,vcms=info"
output = "file"
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `VCMS_MCP_TOKEN` | - | `vcms_site_*` access token forwarded by `vcms mcp stdio` as the bearer credential (required for stdio) |
| `VCMS_MCP_URL` | `http://127.0.0.1:3000` | Running server's base URL that `vcms mcp stdio` proxies to (`{url}/mcp`) |

No server-side environment variables are supported.

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

## Full-text search

Entry search is backed by an embedded [Tantivy](https://github.com/quickwit-oss/tantivy)
index in `apps/backend/src/services/search/` — one engine that gives ranked,
tokenized, stemmed search identically across SQLite/Postgres (native per-DB
FTS would mean two implementations). The DB stays the source of truth; the index
is **derived** and fully rebuildable.

**Reads vs writes are split** so the single-writer limit never blocks *searching*:

- `schema.rs` — fixed schema over the schema-less `entries.data`: `id`, `site_id`,
  `collection_id`, `status` (exact-match filters), `slug`, and `body` (the flattened
  scalar text of `data`, English-stemmed, BM25-ranked). `fields_from` re-resolves the
  schema when opening an existing index read-only.
- `mod.rs` — `SearchService`. **Reading** needs no lock: `open_read_only` (reader
  only) lets *any* auxiliary process search the index alongside the server (this is a
  general capability; `vcms mcp stdio` itself no longer opens the index — it proxies
  over HTTP). **Writing** requires the directory lock: `open`
  (reader + writer) is held only by the running server. `index_doc`/`delete_doc`
  stage uncommitted ops; `commit` flushes; `rebuild_all`/`rebuild_site` reindex from
  the DB by reusing `EntryRepository::list` (covers singletons too). Write/commit on
  a read-only instance returns `SearchError::ReadOnly`.
- `queue.rs` — `SearchQueue` over the `search_index_queue` table (migration
  `20260618000000`). Content writes enqueue here
  (`enqueue`/`dequeue_batch`/`delete_ids`); UUIDv7 ids order the queue chronologically.
  The table exists purely for **durability** (crash recovery), not cross-process
  signaling — all producers run in the server process (`vcms mcp stdio` is an HTTP proxy).
- `indexer.rs` — the **single consumer**, spawned once by the server (it owns the
  writer). Drains the queue in batches (present in DB ⇒ upsert doc, absent ⇒ delete
  doc — `op` is advisory), commits per batch, deletes processed rows. **Purely
  event-driven**: one startup drain (rows left by a crash), then sleeps until an
  enqueue rings the in-process `Notify` — no polling.

Wiring: `Services` holds `search: Option<Arc<SearchService>>` (reads) and
`search_queue: Option<Arc<SearchQueue>>` (writes). `Services::new` opens the index
read-write (server); `Services::new_read_only` opens it read-only for an auxiliary
process that searches without taking the writer lock.
`EntryService`/`SingletonService` **enqueue** on write; `EntryService::list_entries`
queries the index — so REST, GraphQL, gRPC, and MCP all get ranked search via the
existing `search` param with **no handler changes**. Indexing is asynchronous
(enqueue → indexer), typically sub-second. When search is disabled
(`SEARCH_ENABLED=false`) or the index can't open, it falls back to SQL `LIKE`. The
index builds on startup when empty, rebuilds after a restore, and exposes
owner/operator reindex routes: `POST /api/dashboard/instance/search/reindex` and
`POST /api/dashboard/sites/{site_id}/search/reindex`.

Writes land in the durable queue and the running server indexes them; rows enqueued
before a crash drain on the next start. The remaining hard limit is *concurrent
writers* — only one process indexes at a time.

### No typo tolerance (deliberate) — and how to add it

Search has stemming + BM25 ranking + phrase/multi-field, but **no fuzzy / typo
tolerance**. These are different things: **stemming** maps word *forms* to a root
("running"→"run") so different grammatical forms match — it does **not** fix
misspellings ("runnnig" still matches nothing). **Fuzzy** matches within an edit
distance and is what handles typos.

Fuzzy is omitted on purpose. The obvious switch — `QueryParser::set_field_fuzzy` —
turns every term into a Tantivy `FuzzyTermQuery`, which scores by a **constant**
instead of BM25. That flattens relevance ranking: a doc mentioning the term three
times no longer outranks one mentioning it once (the `ranks_frequent_term_higher`
unit test in `services/search/mod.rs` catches exactly this). Since ranking is the
whole reason we moved off SQL `LIKE`, we kept ranking over fuzzy.

It can be added later **without losing ranking**, contained to `search_entries` in
`services/search/mod.rs` (no schema/migration/index changes — the index already holds
the stemmed terms):
- **Zero-hit fallback** (simplest): run the normal BM25 query; only retry with a fuzzy
  query when it returns nothing. Correct spellings keep perfect ranking; typos still
  return results. Won't rescue a typo buried in an otherwise-matching multi-word query.
- **Boosted exact-OR-fuzzy**: always search `(exact/stemmed)^high OR (fuzzy)^low` so
  exact matches keep their BM25 order and sort above fuzzy near-matches, while
  misspellings still surface via the low-boost arm. Always-on, ranking intact; slightly
  more query cost.

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
