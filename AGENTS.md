# AGENTS.md — Weld

Context file for AI coding assistants working on this codebase.

## Project Overview

Weld is a cross-platform developer workspace (task boards, chat, GitHub integration, secrets vault, time tracking). It uses a **shared Rust core + platform-native UI shells** architecture.

## Repository Layout

```
weld/
├── Cargo.toml                    # Workspace root (resolver v2)
├── crates/
│   ├── weld-core/                # Shared Rust library — models, API client, state, crypto
│   │   └── src/
│   │       ├── lib.rs
│   │       └── models/           # Domain models (user, task, message, project, secret, timeentry, github, enums)
│   ├── weld-server/              # Axum 0.8 backend
│   │   ├── migrations/           # sqlx SQL migrations
│   │   └── src/
│   │       ├── main.rs           # Entry point — tokio, tracing, PgPool, TcpListener
│   │       ├── config.rs         # Config::from_env() — HOST, PORT, DATABASE_URL
│   │       ├── state.rs          # AppState (holds PgPool)
│   │       └── routes/mod.rs     # Route definitions (currently: GET /health)
│   ├── weld-uniffi/              # UniFFI bindings for mobile (planned)
│   └── weld-tui/                 # Terminal UI with Ratatui (planned)
├── apps/
│   ├── web/                      # SvelteKit + Tailwind + shadcn-svelte (planned)
│   ├── desktop/                  # Tauri v2 (planned)
│   ├── android/                  # Kotlin Compose (planned)
│   └── ios/                      # SwiftUI (planned)
├── docker/
│   └── docker-compose.yml        # PostgreSQL 16, Redis 7, MinIO
├── scripts/
│   ├── setup.sh                  # First-time dev environment setup
│   └── migrate.sh                # sqlx migration runner
└── Makefile                      # Dev workflow commands
```

## Tech Stack

- **Backend**: Axum 0.8, PostgreSQL 16 (sqlx with compile-time checked queries), Redis (fred), tokio, tracing
- **Shared Core**: reqwest, tokio-tungstenite, rusqlite + SQLCipher, serde, chrono, uuid, thiserror
- **Web Frontend**: SvelteKit 2, TypeScript (strict), Tailwind CSS 4, shadcn-svelte, Svelte 5 runes
- **Desktop**: Tauri v2 (wraps web UI)
- **Mobile**: UniFFI bindings → Kotlin Compose (Android) / SwiftUI (iOS)
- **TUI**: Ratatui + crossterm

## Coding Standards

### Rust
- `thiserror` for library errors, `anyhow` only in binary crates
- No `.unwrap()` in library code — use `?` or `.expect("reason")`
- `#[instrument]` from `tracing` on all async service methods
- `cargo clippy -- -W clippy::all` and `cargo fmt` before commits
- Integration tests for all API endpoints using `sqlx::test`

### TypeScript / Svelte
- Strict TypeScript — no `any` types
- Svelte 5 runes syntax (`$state`, `$derived`, `$effect`)
- PascalCase component file names
- Tailwind classes only — no custom CSS unless necessary
- All interactive elements have ARIA labels

### General
- Conventional commits (`feat:`, `fix:`, `docs:`, `chore:`)
- UUIDs as primary keys everywhere
- All timestamps UTC ISO 8601
- Secrets never committed — use `.env` files (gitignored)
- SQL changes via sqlx migrations only, never manual SQL
- Errors propagate up to route handlers and convert to HTTP status codes

## Current State

**Phase 1 (Foundation) is in progress.** What exists today:
- Cargo workspace with `weld-core` and `weld-server`
- All domain models defined in `weld-core/src/models/`
- Axum server with health check endpoint
- PostgreSQL connection pool via sqlx
- Initial migration: users, teams, team_memberships, projects tables
- Docker Compose for local infrastructure
- Dev scripts (setup.sh, migrate.sh) and Makefile

**What's being built next:** Auth system (register, login, JWT middleware), SvelteKit frontend setup, project CRUD.

## Key Domain Models

Core entities: User, Team, TeamMembership, Project, Task, Column, Label, Channel, ChatMessage, Secret, TimeEntry, Timesheet, GitHubConnection.

Enums: GlobalRole (Admin/Member), TeamRole (Owner/Admin/Member/Viewer), Priority (Critical/High/Medium/Low/None), Environment (Development/Staging/Production), TimesheetStatus (Draft/Submitted/Approved/Rejected).

All models are defined in `crates/weld-core/src/models/`.

## Running Locally

```bash
make setup    # First time: install deps, start Docker, create .env, run migrations
make start    # Daily: start Docker + run migrations
make server   # Start the Axum backend on :3000
make test     # Run all Rust tests
make lint     # clippy + fmt check
```

Requires: Rust (stable), Docker, sqlx-cli, Node.js 20+ and pnpm (for web frontend).
