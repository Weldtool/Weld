.PHONY: setup start stop server docker migrate clean web web-install test lint fmt check audit

# First-time setup (runs everything)
setup:
	./scripts/setup.sh

# Start Docker + run migrations (daily driver command)
start: docker migrate
	echo "Ready! Run 'make server' to start the backend."

# Just start the Docker containers (for development)
docker:
	docker compose -f $(CURDIR)/docker/docker-compose.yml up -d

# Run database migrations
migrate:
	./scripts/migrate.sh

# Start the backend server (after running 'make start')
server:
	set -a && source $(CURDIR)/.env && set +a && cargo run -p weld-server

# Stop Docker containers
stop:
	docker compose -f $(CURDIR)/docker/docker-compose.yml down

# Stop containers and delete all data volumes (use with caution)
clean:
	docker compose -f $(CURDIR)/docker/docker-compose.yml down -v

# Start the SvelteKit dev server (for frontend development)
web:
	cd $(CURDIR)/apps/web && pnpm run dev

# Install web dependencies
web-install:
	cd $(CURDIR)/apps/web && pnpm install

# Run all Rust tests
test:
	cargo test --workspace --all-features

# Run clippy lint + format check
lint:
	cargo clippy --workspace -- -W clippy::all
	cargo fmt --check

# Format all Rust code
fmt:
	cargo fmt

# Run all pre-push checks (format + lint + tests)
check: lint test

#Run security audit on Rust dependencies
audit:
	cargo audit
