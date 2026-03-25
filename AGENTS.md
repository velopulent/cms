# AI Agent Instructions for CMS (Rust + React)

This file provides the workspace-level agent guidance for anyone contributing or exploring this repository.

## 🧩 Project overview

- Backend: Rust + Axum HTTP server, `SQLx` + SQLite, `rust-embed` (for static assets).
- Frontend: React in `ui/` with Tanstack Router, Tanstack Query, shadcn/ui.
- Build path: root `cargo build` triggers `build.rs` which runs `bun run build` in `ui/`, then embeds `ui/dist` into binary.
- Runtime: one executable serves `/api/*` endpoints and static site SPA fallback.

## 🚀 Key directories

- `src/`: Rust backend source.
  - `database/`: DB setup and connection helpers.
  - `handlers/`: HTTP handlers (auth, content, schema, site, UI).
  - `middleware/`: auth middleware.
  - `models/`: domain models.
  - `router/`: route composition.
- `ui/`: frontend app (React + TypeScript).
- `target/`: build artifacts (ignored in VCS).

## 🛠️ Standard commands

### Backend dev

- `cargo run` (runs server, auto asset-rebuild from `build.rs` if needed)

### Frontend dev

- `cd ui && bun run dev`
- Ensure `ui` dev server proxies to backend at local API host.

### Production build

- `cargo build --release`
- `./target/release/cms`

### Testing

- Add tests in code modules (typically `#[cfg(test)]` in Rust modules).
- No dedicated test harness in repo currently, use `cargo test` when added.

## 🧰 Agent workflow

1. Discover conventions: check this file, `README.md`, and code patterns in `src/` and `ui/src/`.
2. Follow project style:
   - Rust: idiomatic `Result`/error handling, `axum` extractors.
   - REST handlers return JSON and status codes across `handlers/`.
   - React: functional components, hooks, data fetching via Tanstack Query.
3. For new features:
   - Identify existing handler and model boundaries (content, schema, site, auth).
   - Keep API shape stable; if new endpoints are added, ensure frontend uses them.
4. For bugfixes:
   - Reproduce with `cargo run` + `bun run dev` or `cargo test`.
   - Add regression tests near the bug.

## 🧾 Code review/PR hints

- Keep changes scoped and explain any cross-cutting backend/frontend behavior.
- If a handler API changes, update frontend data fetching + UI integration.
- Prefer simple, explicit data mappings in payloads.

## 🔗 Documentation and knowledge links

- `README.md`: canonical architecture + commands.
- `src/` and `ui/src/`: authoritative patterns.
