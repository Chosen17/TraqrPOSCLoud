\# Traqr Cloud

Multi-tenant cloud platform for the Traqr POS ecosystem. Web app at **cloud.traqr.co.uk** (marketing, pricing, devices, contact) and API for POS sync at **api.traqr.co.uk** (or `/api` on the same host).

## Features

- Org → Franchise → Store tenancy
- Head office menu publishing
- Store-level availability overrides
- Offline-first POS with device-authoritative sync
- Append-only event log
- Command queue with two-person approval

## Tech Stack

- Rust + Axum
- PostgreSQL
- Tailwind CSS (local build)
- S3-compatible storage (planned)

## Database and migrations

**PostgreSQL** is required for the API (device activation, sync, login). The app runs migrations automatically on startup from the `migrations/` directory, so you don't need the `sqlx` CLI.

**First-time setup (database + app user):** run the script once so the app has a user and database to connect to:

```bash
# As postgres (Linux): sudo -u postgres ./scripts/setup-db.sh
# Or with your postgres password: PGPASSWORD=your_postgres_password ./scripts/setup-db.sh
cd /var/rustApps/TraqrPOSCLoud && ./scripts/setup-db.sh
```

See `scripts/README.md` for options. This creates the database `traqr_cloud` and user **`traqr_app`** (password **`traqr_app_pass`**). The project `.env` is set to use that user. Then `cargo run` will apply migrations and start the server.

### Verify database connection

1. **On startup** the app logs either **`Database: connected`** or **`Database: not available — ...`**. If not available, it also logs a redacted `DATABASE_URL` and a hint.
2. **Check from the API:** `curl http://localhost:8080/api/health` returns `{ "ok": true, "db": "connected" }` or `"db": "disconnected"`.
3. **Checklist if DB is disconnected:**
   - Is PostgreSQL running? (`pg_isready -h localhost` or your system service)
   - Does the database exist? `createdb traqr_cloud` or `psql -c "CREATE DATABASE traqr_cloud;"`
   - Is `DATABASE_URL` correct? (user, password, host, port, database name). Use `.env` or `export DATABASE_URL=...`.

## Demo login

After the app has run (and applied `010_demo_user.sql`), you can log in at `/login.html` with:

- **Email:** `admin@traqr.co.uk`
- **Password:** `demo123`

Requires PostgreSQL (with pgcrypto) and the demo user seed. Change or remove the demo user in production.

## Web app (cloud.traqr.co.uk)

Static site with Tailwind CSS: Product, Pricing, Devices, Contact, Login.

- **Build CSS:** `npm run build:css` (output: `web/public/css/output.css`)
- **Watch CSS:** `npm run watch:css`
- **Serve:** Run the API server; static files from `web/public` are served at `/`. Set `WEB_ROOT` to override (default `web/public`).

## API

API routes are under **`/api`** so the same server can host both the web app and the POS API:

- `GET /api/health` — returns `{ "ok": true, "db": "connected" }` or `"db": "disconnected"`
- `POST /api/device/activate`
- `POST /api/sync/events` (Bearer device token)
- `GET /api/sync/commands` (Bearer device token)
- `POST /api/sync/commands/ack` (Bearer device token)
- `POST /api/auth/login` (JSON: `{ "email", "password" }` — portal login)

POS clients should use base URL `https://api.traqr.co.uk` or `https://cloud.traqr.co.uk/api`.



