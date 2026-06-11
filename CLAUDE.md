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
  - `tests/rest/` — REST API tests: `auth`, `sites`, `collections`, `entries`, `singletons`, `files`, `webhooks`, `access_tokens`
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
```

Global flags (highest precedence): `--config <PATH>`, `--bind <ADDR>`, `--database-url <URL>`, `--log-level <LEVEL>`.

The server auto-migrates the database on every startup; there is no separate migrate command.

## Configuration

Non-secret settings live in a TOML config file; secrets stay in the environment (or `.env`).
Layers merge with precedence: **CLI flag > env var > config file > built-in default**.

Config file search order (first existing wins; missing is fine):
1. `--config` flag / `CMS_CONFIG` env
2. `./cms.toml` (current dir)
3. user config dir — `~/.config/cms/config.toml` (Linux), `%APPDATA%\cms\config\config.toml` (Windows), `~/Library/Application Support/cms/config.toml` (macOS)
4. `/etc/cms/config.toml`

Env-only secrets (never read from the config file by convention, omitted from `config init`):
`DATABASE_URL`, `JWT_SECRET`, `HMAC_SECRET`, `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY`.

Sample `config.toml` (generate with `cms config init`):

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
| `DATABASE_URL` | `sqlite:cms.db` | Database URL: `sqlite:path`, `postgres://...`, `mysql://...` |
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
| `LOG_DIR` | `logs` | Log directory when `output = file` (`[log] dir`) |

**Warning**: Default `JWT_SECRET` and `HMAC_SECRET` print warnings on startup—set these in production.

## Proto Compilation

- Proto files in `libs/proto/` are compiled by `apps/backend/build.rs` into `apps/backend/src/grpc/cms/` using `tonic-build`
- Generated code is **not** committed; `build.rs` runs on every `cargo build`
- `cargo:rerun-if-changed=../../libs/proto/` triggers rebuilds

## First-Run Behavior

On initial startup, the server seeds a default admin user:
- Username: `admin`
- Password: `admin`
**Change this password immediately in production.**

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
