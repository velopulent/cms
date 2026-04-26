<p align="center">
  <img src="assets/logo.avif" width="400" />
    <p align="center">
    A headless, API-first content management system built for developers.
    </p>
</p>

---

## Features

### Multi-Site Management
Manage multiple websites from a single installation with role-based access control.

### Custom Content Types
Define collections with your fields: text, rich text, numbers, dates, images, and more.

### API-First Design
REST and GraphQL APIs out of the box with auto-generated OpenAPI documentation.

### Media Management
Upload, organize, and serve media with automatic thumbnails and S3 support.

### Developer-Friendly
Single binary deployment. No external dependencies. JWT auth and API keys for programmatic access.

---

## Stack

### Backend

- [Axum](https://docs.rs/axum) - HTTP server & routing
- [SQLx](https://docs.rs/sqlx) - Async SQL toolkit
- SQLite - Database
- [rust-embed](https://docs.rs/rust-embed) - Static asset embedding
- [async-graphql](https://async-graphql.github.io/async-graphql/en/) - GraphQL API
- [utoipa](https://docs.rs/utoipa) - OpenAPI generation

### Frontend

- [React](https://react.dev) - UI framework
- [Tanstack Router](https://tanstack.com/router) - Routing
- [Tanstack Query](https://tanstack.com/query) - Server state
- [shadcn/ui](https://ui.shadcn.com) - UI components
- [Tailwind CSS](https://tailwindcss.com) - Styling
- [Tiptap](https://tiptap.dev) - Rich text editor

---

## How It Works

1. The React app lives inside the `dashboard/` folder.
2. During release builds, `build.rs` runs `bun run build`.
3. The compiled `dashboard/dist` files are embedded into the Rust binary using `rust-embed`.
4. The `ui_handler` serves static assets and provides SPA fallback.
5. `/api/*` routes are handled by Axum.

Result: one binary that serves both API and UI.

---

## Quick Start

```bash
# Build the project
cargo build --release

# Run the server
./target/release/cms
```

Visit `http://localhost:3000` to access the admin UI.

---

## Development

### Backend

```bash
cargo run
```

### Frontend

```bash
cd dashboard
bun run dev
```

During development, the React dev server proxies API requests to the Rust backend.

---

## API Access

| Endpoint | Description |
|----------|-------------|
| `/api/v1/` | REST API |
| `/api/graphql` | GraphQL API |
| `/api/v1/docs` | OpenAPI documentation |

---
