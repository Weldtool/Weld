#!/usr/bin/env bash
set -euo pipefail

# -- Resolve project root --
# Same pattern as setup.sh - find where this script lives, then go up on levels to find the project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Load .env file if it exists ---
if [[ -f "$PROJECT_ROOT/.env" ]]; then
  set -a
  source "$PROJECT_ROOT/.env"
  set +a
fi

# --- Require DATABASE_URL ---
if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "Error: DATABASE_URL is not set. Run setup.sh first or export it."
  exit 1
fi

# --- Run migrations ---
echo "Running database migrations..."
sqlx migrate run \
--source "$PROJECT_ROOT/crates/devforge-server/migrations" \
    "$@"
