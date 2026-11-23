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
│   │   ├── tests/                # Integration tests, one file per area
│   │   └── src/
│   │       ├── main.rs           # Entry point — tokio, tracing, PgPool, TcpListener
│   │       ├── config.rs         # Config::from_env() — all env vars
│   │       ├── state.rs          # AppState (PgPool, realtime Hub, GitHub/agent config)
│   │       ├── errors.rs         # ApiError → HTTP status; Internal detail is logged, never returned
│   │       ├── rate_limit.rs     # In-process limiter for the unauthenticated endpoints
│   │       ├── middleware/       # AuthUser extractor (JWT bearer)
│   │       ├── realtime/         # WebSocket hub: rooms, events, broadcast
│   │       ├── routes/           # HTTP handlers; mod.rs holds the route table
│   │       └── services/         # Business logic per area: auth, team, project, board,
│   │                             #   column, label, task, chat, github, study
│   ├── weld-uniffi/              # UniFFI bindings for mobile (planned)
│   └── weld-tui/                 # Terminal UI with Ratatui (planned)
├── .sqlx/                        # Committed sqlx offline query cache (`cargo sqlx prepare --workspace`)
├── apps/
│   ├── web/                      # SvelteKit + Tailwind + shadcn-svelte
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
- Integration tests for all API endpoints (`crates/weld-server/tests/`, run against the `DATABASE_URL` database)
- Changing or adding a `sqlx::query!` means regenerating `.sqlx/` with `cargo sqlx prepare --workspace`
- Access checks: use `require_project_team_role` for anything scoped to a project (team membership alone does not grant project access to Members/Viewers), and `require_channel_access` / `require_channel_admin` for channels

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

What exists today:
- Cargo workspace with `weld-core` (domain models) and `weld-server`
- Auth: register/login, JWT access tokens, rotating refresh-token cookie
- Teams (with personal workspaces), projects with per-project membership
- Board: columns, tasks, labels, drag-and-drop ordering
- Chat: channels (public/private/DM), threads, mentions, task references
- Realtime WebSocket hub (project, channel and team rooms, presence)
- GitHub App integration: install + account linking, issue/PR sync, webhooks, review and merge
- Research-study mode (`STUDY_MODE`): participant provisioning, telemetry, context-scoped agent, export
- SvelteKit web app in `apps/web`
- Docker Compose for local infrastructure, dev scripts and Makefile

Not built yet: secrets vault, time tracking, desktop/mobile/TUI shells.

## Key Domain Models

Core entities: User, Team, TeamMembership, Project, Task, Column, Label, Channel, ChatMessage, Secret, TimeEntry, Timesheet, GitHubConnection.

Enums: GlobalRole (Admin/Member), TeamRole (Owner/Admin/Member/Viewer), Priority (Critical/High/Medium/Low/None), Environment (Development/Staging/Production), TimesheetStatus (Draft/Submitted/Approved/Rejected).

All models are defined in `crates/weld-core/src/models/`.

## Running Locally

```bash
make setup    # First time: install deps, start Docker, create .env, run migrations
make start    # Daily: start Docker + run migrations
make server   # Start the Axum backend on :3001
make web      # Start the SvelteKit dev server on :3000 (proxies /api to :3001)
make test     # Run all Rust tests (needs the database from `make start`)
make lint     # clippy + fmt check
make pr       # Everything CI runs for Rust, plus `cargo sqlx prepare`
```

Web checks run from `apps/web`: `pnpm check`, `pnpm lint`, `pnpm test`.

Requires: Rust (stable), Docker, sqlx-cli, Node.js 20+ and pnpm (for web frontend).
