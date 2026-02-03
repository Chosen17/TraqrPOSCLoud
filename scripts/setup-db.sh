#!/usr/bin/env bash
# Create traqr_cloud database and traqr_app user for Traqr Cloud.
# Run with: ./scripts/setup-db.sh
# You need postgres superuser access (e.g. PGPASSWORD=your_postgres_password or sudo -u postgres).

set -e
export PGHOST=${PGHOST:-localhost}
export PGPORT=${PGPORT:-5432}
export PGUSER=${PGUSER:-postgres}

echo "Creating database traqr_cloud (if not exists)..."
createdb traqr_cloud 2>/dev/null || true

echo "Creating user traqr_app and granting privileges..."
psql -d traqr_cloud -f "$(dirname "$0")/setup-db.sql"

echo "Done. Use DATABASE_URL=postgres://traqr_app:traqr_app_pass@localhost:5432/traqr_cloud"
