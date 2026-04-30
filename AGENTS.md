# AI Agent Instructions for CMS (Rust + React)

## Architecture

- **Backend**: Rust + Axum HTTP server, `SQLx` + (SQLite | PostgreSQL | MySQL), `rust-embed` (static assets)
- **Frontend**: React in `apps/dashboard/` with Tanstack Router, Tanstack Query, shadcn/ui
- **gRPC**: Separate server on port 50051 (compiled from `libs/proto/*.proto` via `tonic-build`)
- **Build**: Nx orchestrates `dashboard:build` → `backend:build`. `build.rs` compiles proto files only.
- **Runtime**: Single binary serves REST API (`/api/*`), gRPC, GraphQL (`/api/graphql`), and static SPA fallback

## Key Directories

- `apps/backend/src/` - Rust backend source
- `apps/backend/src/handlers/` - HTTP handlers (auth, content, schema, site, file, UI)
- `apps/backend/src/graphql/` - GraphQL schema and resolvers
- `apps/backend/src/grpc/` - gRPC service implementations (generated from `libs/proto/*.proto`)
- `apps/backend/src/repository/` - Data access layer
- `apps/backend/src/router/` - Route composition
- `libs/proto/` - Protocol Buffer definitions (`cms.proto`)
- `apps/dashboard/` - React frontend app

## Developer Commands

### Backend
```bash
nx run backend:run           # Runs server with embedded dashboard
nx run backend:run-dev       # Runs server without dashboard embed (dev mode)
nx run backend:build         # Release build
nx run backend:test          # Run all Rust tests
```

### Frontend
```bash
nx run dashboard:dev         # Dev server with API proxy
nx run dashboard:build       # Production build
nx run dashboard:format      # Biome format
nx run dashboard:lint        # Biome lint
nx run dashboard:typecheck   # TypeScript type check
```

### Testing
- Rust: `nx run backend:test` (runs both `#[cfg(test)]` modules and `tests/` integration tests)
- Integration tests in `apps/backend/tests/` use in-memory SQLite (`sqlite::memory:`) and include schema directly

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
- **Styling**: Tailwind CSS v4 with `tw-animate-css`; shadcn/ui components
- **Formatting**: Biome (not ESLint/Prettier) for frontend

## Tooling Config

- `rustfmt.toml`: 120 max width, 4 space indent
- `clippy.toml`: cognitive-complexity threshold 30
- `biome.json`: 2-space indent, double quotes, organized imports

## Agent Workflow

1. Discover: Check this file, `README.md`, `Cargo.toml`, `nx.json`, and code patterns
2. For new features: Identify handler and model boundaries; keep API stable
3. For bugfixes: Reproduce with `nx run backend:run-dev` + `nx run dashboard:dev` or `nx run backend:test`
4. If a handler changes: Update frontend data fetching and UI integration
