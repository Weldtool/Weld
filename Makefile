.PHONY: setup start stop server clean web web-install test lint pr audit

# First-time setup (runs everything)
setup:
	./scripts/setup.sh

# Start Docker + run migrations (daily driver command)
start:
	docker compose -f $(CURDIR)/docker/docker-compose.yml up -d
	./scripts/migrate.sh
	@echo "Ready! Run 'make server' (backend :3001) and 'make web' (frontend :3000) in separate terminals."

# Start the backend server (after running 'make start')
server:
	set -a && source $(CURDIR)/.env && set +a && cargo run -p weld-server

# Stop Docker containers
stop:
	docker compose -f $(CURDIR)/docker/docker-compose.yml down

# Stop containers and delete all data volumes (use with caution)
clean:
	docker compose -f $(CURDIR)/docker/docker-compose.yml down -v

# Start the SvelteKit dev server at localhost:3000 (proxies /api to backend at :3001)
web:
	cd $(CURDIR)/apps/web && pnpm run dev

# Install web dependencies
web-install:
	cd $(CURDIR)/apps/web && pnpm install

# Run all Rust tests (needs the database from 'make start')
test:
	set -a && source $(CURDIR)/.env && set +a && cargo test --workspace

# Formatting + clippy, without touching the database
lint:
	cargo fmt --all -- --check
	SQLX_OFFLINE=true cargo clippy --workspace --all-targets -- -W clippy::all

# Run all checks needed before opening a PR
pr:
	cargo fmt
	SQLX_OFFLINE=true cargo clippy --workspace -- -W clippy::all
	cargo test --workspace
	cargo sqlx prepare --workspace
	cargo audit

#Run security audit on Rust dependencies
audit:
	cargo audit
