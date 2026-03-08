#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

# -- Resolve project root --
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Helper function ---
check_tool() {
  if command -v "$1" &> /dev/null; then
    echo -e "${GREEN}Found $1${NC}"
  else
    echo -e "${RED}Error: $1 is not installed. Please install it and try again.${NC}"
    MISSING=1
  fi
}

echo "Checking prerequisites..."
MISSING=0

check_tool cargo
check_tool sqlx
check_tool docker
check_tool node
check_tool pnpm

# If any tool was missing, MISSING will be 1. Exit so the user can install them.
if [[ "$MISSING" -eq 1 ]]; then
  echo -e "\n${RED}Some tools are missing. Install them and re-run this script.${NC}"
  exit 1
fi
echo ""

# --- Start docker services ---
echo -n "Starting Docker services..."
docker compose -f "$PROJECT_ROOT/docker/docker-compose.yml" up -d --quiet-pull
echo -e " ${GREEN}Done${NC}"

# --- Wait for postgres ---
echo -n "Waiting for Postgres..."
for i in $(seq 1 30); do
  if docker compose -f "$PROJECT_ROOT/docker/docker-compose.yml" exec -T postgres pg_isready -U devforge &> /dev/null; then
    echo -e " ${GREEN}Ready${NC}"
    break
  fi
  if [[ "$i" -eq 30 ]]; then
    echo -e " ${RED}Timed out after 30s${NC}"
    exit 1
  fi
  sleep 1
done

# --- Create .env if it doesn't exit ---
if [[ ! -f "$PROJECT_ROOT/.env" ]]; then
    echo -n "Creating .env file..."

    JWT_SECRET=$(openssl rand -hex 32)
    MASTER_KEY=$(openssl rand -hex 16)

  cat << EOF > "$PROJECT_ROOT/.env"
DATABASE_URL=postgres://devforge:devforge@localhost:5432/devforge
REDIS_URL=redis://localhost:6379
HOST=127.0.0.1
PORT=3000
JWT_SECRET=${JWT_SECRET}
MASTER_ENCRYPTION_KEY=${MASTER_KEY}
S3_ENDPOINT=http://localhost:9000
S3_ACCESS_KEY=devforge
S3_SECRET_KEY=devforge
S3_BUCKET=devforge-files
SMTP_HOST=localhost
SMTP_PORT=1025
EOF
echo -e "${GREEN}done${NC}"
else
    echo -e "${YELLOW}.env file already exists, skipping creation${NC}"
fi

# --- Run migrations ---
echo ""
"$SCRIPT_DIR/migrate.sh"

# --- Done ---
echo ""
echo -e "${GREEN}Devforge dev environment setup complete!${NC}"
echo ""
echo "Start the server with:"
echo " cd $PROJECT_ROOT && make server"
