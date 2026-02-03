#!/usr/bin/env bash
# Set password for PostgreSQL user 'traqr' so the app can connect.
# Run once: sudo -u postgres ./scripts/set-password-traqr.sh
set -e
psql -c "ALTER ROLE traqr WITH PASSWORD 'traqr_pass';"
echo "Password set. Restart the app (cargo run) and log in at http://localhost:8080/login.html"
