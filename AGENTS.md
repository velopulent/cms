# AI Agent Instructions for CMS (Rust + React)

## Architecture

- **Backend**: Rust + Axum HTTP server, `SQLx` + (SQLite | PostgreSQL | MySQL), `rust-embed` (static assets)
- **Frontend**: React in `dashboard/` with Tanstack Router, Tanstack Query, shadcn/ui
- **gRPC**: Separate server on port 50051 (compiled from `proto/*.proto` via `tonic-build`)
- **Build**: `cargo build` runs `build.rs` which executes `bun run build` in `dashboard/` and compiles proto files
- **Runtime**: Single binary serves REST API (`/api/*`), gRPC, GraphQL (`/api/graphql`), and static SPA fallback

## Key Directories

- `src/` - Rust backend source
- `src/handlers/` - HTTP handlers (auth, content, schema, site, file, UI)
- `src/graphql/` - GraphQL schema and resolvers
- `src/grpc/` - gRPC service implementations (generated from `proto/*.proto`)
- `src/repository/` - Data access layer
- `src/router/` - Route composition
- `proto/` - Protocol Buffer definitions (`site.proto`, `admin.proto`)
- `dashboard/` - React frontend app

## Developer Commands

### Backend
```bash
cargo run                    # Runs server (rebuilds assets via build.rs)
SKIP_DASHBOARD_BUILD=1 cargo build  # Skip frontend build for faster iteration
```

### Frontend
```bash
cd dashboard && bun run dev    # Dev server with API proxy
cd dashboard && bun run build   # Production build
cd dashboard && bun run format  # Biome format
cd dashboard && bun run lint    # Biome lint
cd dashboard && bun run check   # Biome check (format + lint + imports)
```

### Testing
- Rust: `cargo test` (runs both `#[cfg(test)]` modules and `tests/` integration tests)
- Integration tests in `tests/` use in-memory SQLite (`sqlite::memory:`) and include schema directly

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
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

**Warning**: Default `JWT_SECRET` and `HMAC_SECRET` print warnings on startup—set these in production.

## Proto Compilation

- Proto files in `proto/` are compiled by `build.rs` into `src/grpc/cms/` using `tonic-build`
- Generated code is **not** committed; `build.rs` runs on every `cargo build`
- `cargo:rerun-if-changed=dashboard/` and `cargo:rerun-if-changed=proto/` trigger rebuilds

## First-Run Behavior

On initial startup, the server seeds a default admin user:
- Username: `admin`
- Password: `admin`
**Change this password immediately in production.**

## Code Conventions

- **Rust**: Idiomatic `Result`/error handling, `axum` extractors, custom `AppError` enum for HTTP errors
- **React**: Functional components, hooks, Tanstack Query for server state
- **Styling**: Tailwind CSS v4 with `tw-animate-css`; shadcn/ui components
- **Formatting**: Biome (not ESLint/Prettier) for frontend

## Tooling Config

- `rustfmt.toml`: 120 max width, 4 space indent
- `clippy.toml`: cognitive-complexity threshold 30
- `biome.json`: 2-space indent, double quotes, organized imports

## Agent Workflow

1. Discover: Check this file, `README.md`, `Cargo.toml`, `build.rs`, and code patterns
2. For new features: Identify handler and model boundaries; keep API stable
3. For bugfixes: Reproduce with `cargo run` + `bun run dev` or `cargo test`
4. If a handler changes: Update frontend data fetching and UI integration
