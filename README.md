<div align="center">

# Weld

**The all-in-one developer workspace.**

Task boards, team chat, GitHub integration, secrets management, and time tracking — unified in a single product.

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)
[![SvelteKit](https://img.shields.io/badge/SvelteKit-2.x-FF3E00.svg)](https://kit.svelte.dev/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-24C8D8.svg)](https://v2.tauri.app/)

</div>

## Build Status

| Branch     | Status                                                                                       |
| ---------- | -------------------------------------------------------------------------------------------- |
| Production | ![CI](https://github.com/Weldtool/Weld/actions/workflows/ci.yml/badge.svg?branch=Production) |
| Dev        | ![CI](https://github.com/Weldtool/Weld/actions/workflows/ci.yml/badge.svg?branch=Dev)        |

---

## Why Weld?

Developers juggle too many tools — Jira for tasks, Slack for chat, 1Password for secrets, Toggl for time, GitHub for code. Context-switching between them kills productivity.

Weld welds them together into one workspace with a shared Rust core and native UI on every platform.

## Features

- **Kanban Task Board** — Drag-and-drop columns and cards with fractional indexing, real-time sync across all clients, filters by assignee/label/priority
- **Team Chat** — Channel-based messaging with threading, rich text (Tiptap/Markdown), typing indicators, @mentions
- **GitHub Integration** — GitHub App installation for org-level access, two-way sync between tasks and issues/PRs, webhook-driven updates
- **Secrets Vault** — AES-256-GCM encrypted secrets per project and environment, audit-logged access, optional client-side encryption via WASM
- **Time Tracking** — Timer-based or manual entry, weekly timesheets with approval workflows, reports and CSV export
- **Role-Based Access Control** — Team-level roles (Owner, Admin, Member, Viewer) enforced server-side

## Architecture

```
┌──────────────────────────────────────────────────────┐
│              Backend (Axum + PostgreSQL + Redis)       │
│   REST API · WebSockets · GitHub Webhooks · Auth      │
│   Secrets Vault · File Storage · Real-time Pub/Sub    │
└────────────────────────┬─────────────────────────────┘
                         │
              ┌──────────┴──────────┐
              │      weld-core      │
              │   (shared Rust lib) │
              │                     │
              │  Models · API Client│
              │  State · Encryption │
              │  Sync · Validation  │
              └──┬───┬───┬───┬───┬─┘
                 │   │   │   │   │
           ┌─────┘   │   │   │   └──────┐
           ▼         ▼   ▼   ▼          ▼
         Web      Desktop  Android  iOS    TUI
       SvelteKit  Tauri v2  Kotlin  Swift  Ratatui
       + WASM              Compose  SwiftUI
```

Business logic is written once in Rust. Each platform gets a native UI shell:

| Platform | UI Layer                             | Rust Bridge      |
| -------- | ------------------------------------ | ---------------- |
| Web      | SvelteKit + Tailwind + shadcn-svelte | WASM (wasm-pack) |
| Desktop  | Tauri v2 (reuses web UI)             | Direct Rust      |
| Android  | Jetpack Compose (Material 3)         | UniFFI           |
| iOS      | SwiftUI                              | UniFFI           |
| TUI      | Ratatui                              | Direct Rust      |

## Tech Stack

### Backend

Axum 0.8 · PostgreSQL 16 (sqlx) · Redis (fred) · JWT auth · Argon2id · AES-256-GCM · S3-compatible storage · octocrab · tokio · tracing

### Shared Core

reqwest · tokio-tungstenite · rusqlite + SQLCipher · serde · chrono · uuid · thiserror

### Web Frontend

SvelteKit 2 · TypeScript (strict) · Tailwind CSS 4 · shadcn-svelte · Svelte 5 runes · Tiptap · svelte-dnd-action · Lucide icons

## Project Structure

```
weld/
├── Cargo.toml                # Workspace root
├── crates/
│   ├── weld-core/            # Shared Rust library (models, API client, crypto, state)
│   ├── weld-server/          # Axum backend (routes, services, DB, middleware)
│   ├── weld-uniffi/          # UniFFI bindings for mobile
│   └── weld-tui/             # Terminal UI (Ratatui)
├── apps/
│   ├── web/                  # SvelteKit frontend
│   ├── desktop/              # Tauri v2 wrapper
│   ├── android/              # Kotlin Compose app
│   └── ios/                  # SwiftUI app
├── docker/                   # Docker Compose (Postgres, Redis, MinIO)
└── scripts/                  # Dev setup and migration scripts
```

## Getting Started

### Prerequisites

- Rust (latest stable via rustup)
- Node.js 20+ and pnpm
- Docker and Docker Compose
- sqlx-cli: `cargo install sqlx-cli`

### Setup

```bash
# Clone the repository
git clone https://github.com/Weldtool/Weld.git
cd Weld

# First-time setup (checks prerequisites, starts Docker, creates .env, runs migrations)
make setup

# Start infrastructure + run migrations
make start

# Start the backend server
make server

# Verify it's running
curl http://127.0.0.1:3000/health
# => {"status":"ok"}
```

### Development Commands

```bash
make setup        # First-time setup
make start        # Start Docker + run migrations
make server       # Start Axum backend (loads .env automatically)
make pr           # fmt, clippy -D warnings, tests, sqlx prepare, audit
make stop         # Stop Docker containers
make clean        # Stop containers + delete data volumes

# Web frontend (separate terminal)
cd apps/web && pnpm install && pnpm dev
```

## Current Status

Weld is in active early development.

### Phase 1: Foundation — Complete

- [x] Cargo workspace, domain models in `weld-core`, Axum server skeleton
- [x] PostgreSQL + sqlx migrations, Docker Compose stack (Postgres, Redis, MinIO)
- [x] Developer experience: `make setup`, `make start`, `make pr`
- [x] CI/CD pipeline (fmt, clippy, test, security audit)
- [x] Auth: register, login, JWT middleware, refresh token rotation, logout
- [x] Project CRUD (teams, projects, RBAC, soft-delete) — API + web UI
- [x] SvelteKit + Tailwind 4 + shadcn-svelte web shell, auth UI, project switcher

### Phase 2: Task Board — Complete

- [x] Schema (columns, tasks, labels) + board CRUD API with fractional indexing
- [x] Kanban UI with drag-and-drop columns/cards, filters, label management
- [x] WebSocket hub + live board updates across clients
- [x] Task detail panel with debounced inline editing and real-time reconciliation
- [x] Tiptap markdown editor with shortcuts, focus-aware reconciliation, and round-trip markdown serialization

### Upcoming Phases

3. **Chat** — Channels, threading, typing indicators
4. **Time Tracking** — Timers, manual entry, timesheets
5. **Secrets Management** — Encrypted vault, audit logging
6. **GitHub Integration** — App installation, webhooks, two-way sync
7. **Desktop App** — Tauri v2 with system tray, notifications
8. **Mobile Apps** — Android (Compose) + iOS (SwiftUI) via UniFFI
9. **TUI** — Terminal interface with Ratatui

## Contributing

Contributions are welcome! Please open an issue first to discuss what you'd like to change.

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Commit using conventional commits (`feat:`, `fix:`, `docs:`, etc.)
4. Open a pull request

## License

Licensed under the [Apache License 2.0](LICENSE).
