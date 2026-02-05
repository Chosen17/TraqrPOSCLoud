#!/usr/bin/env bash
# Traqr Cloud uses MySQL. Create database and user if needed, then set DATABASE_URL in .env.
# You already have: database traqrcloud, user owlmailer, password in .env.
# This script is a no-op reminder; migrations run automatically on cargo run.

set -e
echo "MySQL: ensure database 'traqrcloud' exists and DATABASE_URL is set in .env"
echo "Then run: cargo run"
echo "Migrations will run on first start."
