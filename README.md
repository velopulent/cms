<p align="center">
  <img src="assets/logo.avif" width="400" alt="Velopulent CMS Logo" />
</p>

<h1 align="center">Velopulent CMS</h1>

<p align="center">
 Open-source headless CMS focused on user experience and content flexibility.
</p>

<p align="center">
  <a href="https://cms.velopulent.com">Website</a> •
  <a href="https://cms.velopulent.com/docs">Documentation</a> •
  <a href="#what-is-this">About</a> •
  <a href="#features">Features</a> •
  <a href="#getting-started">Getting Started</a> •
  <a href="#why-this-cms">Why This?</a>
</p>

---

> ⚠️ This project is still under active development.

## What Is This?

A modern content management system that gives you complete control over your content without forcing you into a specific frontend technology. Define your content structure, manage multiple websites, and deliver content anywhere via API.

The entire system ships as a single binary. One file runs the admin dashboard, REST API, GraphQL endpoint, and gRPC services. No dependencies. No containers. No complex infrastructure.

---

## Features

### 🏗️ Content Modeling That Fits Your Data

Build custom content types through an intuitive interface. Whether you need blog posts, product catalogs, documentation pages, or landing pages, you define the structure and the system handles the rest.

### 🌐 One CMS, Multiple Sites

Manage content for multiple websites or applications from a single dashboard. Each site stays isolated with its own content, media library, and user permissions.

### 🚀 API-First by Design

Your content is instantly available via REST, GraphQL, gRPC, and MCP (Model Context Protocol) APIs. Build websites, mobile apps, or any digital experience using the tools and frameworks you prefer.

### 📁 Media Management Built In

Upload, organize, and serve images and files with automatic thumbnail generation. Works with local storage or connect your own S3-compatible storage.

### 💾 Backups & Disaster Recovery

Create on-demand or scheduled backups of a single site or the whole instance. Backups are compressed, optionally encrypted (AES-256-GCM), and stored on local disk or a separate S3 bucket — keep the last N automatically. Restore in place or import a site as a copy, from the dashboard or offline with `vcms restore` when the server won't even boot. Backups are a portable logical dump, so you can move data between SQLite, PostgreSQL, and MySQL.

### 🔐 Secure & Scalable

Two-tier role-based access control, Session based authentication, and rate limiting included out of the box. Instance operators (owner and admins) manage the installation and its sites, while per-site collaborators get editor (write content) or viewer (read-only) access. Whether you're running a personal blog or a multi-tenant platform, the security model adapts to your needs.

### 💻 Modern Admin Dashboard

A clean, fast interface for content editors and administrators. Rich text editing, media browsing, content previews, and user management—all in one place.

---

## Getting Started

### Run It

```bash
bun run build
./target/release/vcms
```

Visit `http://localhost:3000/dashboard` and log in with:
- **Email:** `admin@cms.local`
- **Password:** `admin`

*Login is by email; the name field is just a display name. Change the default
password after your first login.*

### Access Your Content

| Endpoint | What It Does |
|----------|--------------|
| `/api/v1/` | REST API for your content |
| `/api/graphql` | GraphQL endpoint |
| `/api/v1/docs` | Interactive API documentation |
| `port 50051`   | gRPC endpoint |
| `/mcp` | MCP Streamable HTTP endpoint |
| `/health/live` | Unauthenticated process liveness probe |
| `/health/ready` | Unauthenticated database-backed readiness probe |

---

### MCP over stdio

For clients that launch a local stdio MCP server, run `vcms mcp stdio`. It is a
**thin proxy** to a running server's `/mcp` endpoint — it opens no database, secrets,
or search index of its own. It forwards JSON-RPC between stdin/stdout and the server
over HTTP, so it keeps working even when the data is owned by a system-service account
that the client process can't read.

```bash
VCMS_MCP_TOKEN=vcms_site_... VCMS_MCP_URL=http://127.0.0.1:3000 vcms mcp stdio
```

It needs only two env vars: `VCMS_MCP_TOKEN` (a `vcms_site_*` access token, forwarded
as the `Authorization: Bearer` credential) and `VCMS_MCP_URL` (the running server's
base URL, default `http://127.0.0.1:3000`; the proxy posts to `{url}/mcp`). A `vcms
serve` instance must be running. MCP protocol messages use stdout; logs use stderr.

```jsonc
// Example MCP client config
{
  "command": "vcms",
  "args": ["mcp", "stdio"],
  "env": { "VCMS_MCP_TOKEN": "vcms_site_...", "VCMS_MCP_URL": "http://127.0.0.1:3000" }
}
```

### Data directory

Runtime mode is deterministic. If the native service is registered, commands use
the installed root. Otherwise `vcms serve` is portable and uses
`<current directory>/vcms_data`. Directory existence never selects a mode.

| Mode | Root |
|------|------|
| Portable | `<current directory>/vcms_data` |
| Installed Linux | `/var/lib/vcms` |
| Installed macOS | `/Library/Application Support/vcms` |
| Installed Windows | `C:\ProgramData\vcms` |

Both modes use one layout: `config.toml`, `secrets.toml`, `vcms.db`, `storage/`,
`backups/`, `logs/`, and `search/` beneath the root. Fresh roots are created
automatically. Existing malformed files are rejected, not repaired. Server env
overrides, config search paths, and legacy layouts are unsupported.



### Installed service operations

Native packages register the hidden `vcms service run` entrypoint with systemd,
launchd, or Windows SCM. Inspect it with `vcms doctor`, `vcms config show`, and
`vcms service status`, then control it through the native service manager.

`vcms doctor` checks resolved configuration, directory access, current database
schema, listener availability, and execution identity without creating or migrating
the database. Normal package removal preserves configuration, secrets, databases,
uploads, backups, and search state. Delete the documented system data directory only
when an irreversible purge is intended.

Upgrades use package-manager semantics and keep paths/configuration stable. Back up
before upgrading: database migrations are forward-only, so downgrading requires an
explicit restore. On first start, immediately change the temporary
`admin@cms.local` / `admin` credentials.

## Why This CMS?

### One File, Everything Included

Most CMS platforms require databases, web servers, reverse proxies, and container orchestration just to get started. This CMS compiles to a single executable that embeds the dashboard, APIs, and documentation site. Copy one file to your server and run it.

### Developer Experience First

Built by developers, for developers. The API is predictable, the documentation is interactive, and the codebase is designed to be extended and customized.

### Database Flexibility

Use SQLite for simple deployments or connect to PostgreSQL or MySQL for production workloads. The same binary works with all three.

### Built for Teams

Multi-site support and two-tier role-based permissions mean your content team, developers, and stakeholders can all work in the same system without stepping on each other. Operators administer the instance and its sites; editors and viewers collaborate per site.

---

## Development

Clone this repository

```bash
git clone https://github.com/velopulent/cms
```

```bash
# Run development server
cd cms
bun install
bun run dev
```

Visit `localhost:3000` to access the backend, `localhost:5173` to access the React Dashboard.

---

## License

[AGPL v3](LICENSE)
