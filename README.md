<p align="center">
  <img src="assets/logo.avif" width="400" alt="CMS Logo" />
</p>

<h1 align="center">The CMS That Ships As a Single Binary</h1>

<p align="center">
 Open-source headless CMS focused on user experience and content flexibility.
</p>

<p align="center">
  <a href="https://cms.velopulent.com">Website</a> •
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

Your content is instantly available via REST, GraphQL, gRPC and MCP APIs. Build websites, mobile apps, or any digital experience using the tools and frameworks you prefer.

### 📁 Media Management Built In

Upload, organize, and serve images and files with automatic thumbnail generation. Works with local storage or connect your own S3-compatible storage.

### 🔐 Secure & Scalable

Role-based access control, JWT authentication, and rate limiting included out of the box. Whether you're running a personal blog or a multi-tenant platform, the security model adapts to your needs.

### 💻 Modern Admin Dashboard

A clean, fast interface for content editors and administrators. Rich text editing, media browsing, content previews, and user management—all in one place.

---

## Getting Started

### Run It

```bash
bun run build
./target/release/cms
```

Visit `http://localhost:3000` and log in with:
- **Username:** `admin`
- **Password:** `admin`

*Change the default password after your first login.*

### Access Your Content

| Endpoint | What It Does |
|----------|--------------|
| `/api/v1/` | REST API for your content |
| `/api/graphql` | GraphQL endpoint |
| `/api/v1/docs` | Interactive API documentation |
| `port 50051`   | gRPC endpoint|
---

## Why This CMS?

### One File, Everything Included

Most CMS platforms require databases, web servers, reverse proxies, and container orchestration just to get started. This CMS compiles to a single executable that embeds the dashboard, APIs, and documentation site. Copy one file to your server and run it.

### Developer Experience First

Built by developers, for developers. The API is predictable, the documentation is interactive, and the codebase is designed to be extended and customized.

### Database Flexibility

Use SQLite for simple deployments or connect to PostgreSQL or MySQL for production workloads. The same binary works with all three.

### Built for Teams

Multi-site support and role-based permissions mean your content team, developers, and stakeholders can all work in the same system without stepping on each other.

---

## Development

Clone this repository

```bash
git clone https://github.com/velopulent/cms
```

```bash
# Run development server
cd cms
bun run dev
```

Visit `localhost:3000` to access the backend, `localhost:5173` to access the React Dashboard.

---

## License

[AGPL v3](LICENSE)
