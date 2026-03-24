# CMS

**CMS** is a content management system built with rust and react.

---

## Stack

#### Backend:

- [Axum](https://docs.rs/axum) (API & File Server)
- [SQLx](https://docs.rs/sqlx) (Async SQL toolkit)
- SQLite (via SQLx)
- [rust-embed](https://docs.rs/rust-embed) (Static asset embedding)

#### Frontend:

- [React](https://react.dev) (with [Tanstack Router](https://tanstack.com/router))
- [Tanstack Query](https://tanstack.com/query)
- [shadcn/ui](https://ui.shadcn.com)

---

## How It Works

1. The React app lives inside the `ui/` folder.
2. During release builds, `build.rs` runs `bun run build`.
3. The compiled `ui/dist` files are embedded into the Rust binary using `rust-embed`.
4. The `ui_handler` serves static assets and provides SPA fallback.
5. `/api/*` routes are handled by Axum.

Result: one binary that serves both API and UI.

---

## Development Workflow

### Backend

```bash
cargo run
```

### Frontend

```bash
cd ui
bun run dev
```

During development, the React dev server can proxy API requests to the Rust backend.

---

## Production Build

```bash
cargo build --release
./target/release/cms
```

This will:

- Build the React app
- Embed the UI into the binary
- Produce a single deployable executable

No external runtime dependencies required.

---
