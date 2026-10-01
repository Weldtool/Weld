# Contributing to Weld

Thanks for your interest in contributing to Weld! This guide will help you get set up and understand how we work.

## Getting Started

### Prerequisites

You'll need the following installed:

- **Rust** (latest stable) — install via [rustup](https://rustup.rs/)
- **Node.js 20+** and **pnpm** — install pnpm via `npm install -g pnpm`
- **PostgreSQL 16**
- **Redis 7**
- **Docker and Docker Compose**

Install the required Rust tools:

```bash
cargo install sqlx-cli
cargo install tauri-cli
cargo install wasm-pack
```

### Local Setup

```bash
# Clone the repo
git clone https://github.com/welddev/weld.git
cd weld

# Start Postgres, Redis, and MinIO
docker compose -f docker/docker-compose.yml up -d

# Copy the example env file and edit as needed
cp .env.example .env

# Run database migrations
cd crates/weld-server
sqlx migrate run
cd ../..

# Start the backend
cargo run -p weld-server

# In a separate terminal, start the web frontend
cd apps/web
pnpm install
pnpm dev
```

The backend runs at `http://localhost:3001` and the web frontend at `http://localhost:3000` (it proxies `/api` to the backend).

## Making Changes

### Branch Naming

Create your branch off `dev` (not `main`) using one of these prefixes:

- `feature/` — new functionality (e.g. `feature/task-filters`)
- `fix/` — bug fixes (e.g. `fix/chat-scroll-position`)
- `chore/` — tooling, CI, refactors (e.g. `chore/update-dependencies`)
- `docs/` — documentation changes (e.g. `docs/api-examples`)

### Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/). Every commit message should follow this format:

```
type: short description

Optional longer explanation.

Closes #123
```

Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `ci`

Examples:

- `feat: add label filtering to task board`
- `fix: prevent duplicate WebSocket connections on reconnect`
- `docs: add secrets management API examples`

### Pull Requests

1. Push your branch and open a PR against `dev`
2. Fill out the PR template — don't skip the "How to test" section
3. Make sure CI passes (tests, clippy, fmt)
4. Request a review

Keep PRs focused. One feature or fix per PR. If you find an unrelated issue while working, open a separate PR for it.

## Coding Standards

### Rust

- Format with `cargo fmt` before every commit
- Lint with `cargo clippy -- -W clippy::all` — fix all warnings
- Use `thiserror` for error types in library crates, `anyhow` only in binaries
- No `.unwrap()` in library code — use `?` or `.expect("reason")`
- Add `#[instrument]` from `tracing` on async service methods
- Write doc comments on all public functions and types
- Write tests — use `sqlx::test` for anything touching the database

### TypeScript / Svelte

- Strict TypeScript — no `any` types
- Use Svelte 5 runes syntax (`$state`, `$derived`, `$effect`)
- Components use PascalCase file names
- Tailwind classes only — avoid custom CSS unless absolutely necessary
- Run `pnpm lint` and `pnpm check` before committing
- All interactive elements must have ARIA labels

### General

- All timestamps are UTC
- All IDs are UUIDs
- Database changes go through sqlx migrations — never modify the database manually

## Running Tests

```bash
# Rust unit and integration tests
cargo test

# Clippy lint check
cargo clippy -- -W clippy::all

# Format check
cargo fmt -- --check

# Web frontend tests
cd apps/web
pnpm test

# E2E tests (requires backend running)
cd apps/web
pnpm test:e2e
```

## Project Structure at a Glance

```
weld/
├── crates/
│   ├── weld-core/        # Shared Rust library — models, API client, crypto, state
│   ├── weld-server/      # Backend API server (Axum + Postgres + Redis)
│   ├── weld-uniffi/      # UniFFI bindings for mobile apps
│   └── weld-tui/         # Terminal UI (optional)
├── apps/
│   ├── web/              # SvelteKit web frontend
│   ├── desktop/          # Tauri v2 desktop wrapper
│   ├── android/          # Kotlin + Jetpack Compose
│   └── ios/              # Swift + SwiftUI
├── docker/               # Docker Compose and Dockerfiles
├── docs/                 # Architecture and API documentation
└── scripts/              # Dev helper scripts
```

## Where to Contribute

Not sure where to start? Look for issues labeled **good-first-issue** or **help-wanted**. You can also:

- Improve documentation or add examples
- Write tests for untested code paths
- Fix bugs from the issue tracker
- Suggest and discuss features before implementing them

## Questions?

- Open a [discussion](https://github.com/welddev/weld/discussions) on GitHub
- Ask in our [Discord](https://discord.gg/YOUR_INVITE_LINK) in the #contributing channel

We're glad you're here. Let's build something great.
