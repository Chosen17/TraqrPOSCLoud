\# Traqr Cloud

Multi-tenant cloud platform for the Traqr POS ecosystem.

## Quick start (run in browser)

1. **MySQL:** Create database `traqrcloud` and set `DATABASE_URL` in `.env` (see `.env.example`). You already have: database **traqrcloud**, user **owlmailer**, password in `.env`.
2. **Start the server:** `cargo run` (loads `.env` from project root; migrations run on first start; serve at http://0.0.0.0:8080).
3. **Open in browser:** http://localhost:8080. Check DB: http://localhost:8080/api/health → `"db": "connected"`. **Login:** `/login.html` with **admin@traqr.co.uk** / **demo123**.

## Features

- Org → Franchise → Store tenancy
- Head office menu publishing
- Store-level availability overrides
- Offline-first POS with device-authoritative sync
- Append-only event log
- Command queue with two-person approval

## Tech Stack

- Rust + Axum
- MySQL
- Tailwind CSS (local build)
- S3-compatible storage (planned)

## Database and migrations

**MySQL** is required for the API (device activation, sync, login). The app runs migrations automatically on startup from the `migrations/` directory.

**Setup:** Create the database (e.g. `CREATE DATABASE traqrcloud;`) and set `DATABASE_URL` in `.env` to `mysql://user:password@localhost:3306/traqrcloud`. Then `cargo run` applies migrations and starts the server.

If you see **"Table 'traqrcloud.plans' doesn't exist"** or **"Migrations failed"** on startup, run migrations manually from the project root (with `DATABASE_URL` in `.env`):

```bash
cargo install sqlx-cli --no-default-features --features mysql
sqlx migrate run
```

Then run `cargo run` again. If a migration failed partway, drop and recreate the database, then run again.

### Verify database connection

1. **On startup** the app logs **`Database: connected`** or **`Database: not available — ...`**.
2. **Check from the API:** `curl http://localhost:8080/api/health` returns `{ "ok": true, "db": "connected" }` or `"db": "disconnected"`.
3. If disconnected: ensure MySQL is running, the database exists, and `DATABASE_URL` in `.env` is correct.

## Demo login

After migrations have run (including `010_demo_user.sql`), log in at `/login.html` with:

- **Email:** `admin@traqr.co.uk`
- **Password:** `demo123`

Change or remove the demo user in production.

## Deploying to a server

The app is ready to run on a server. Use this checklist:

1. **Server and MySQL** — Install MySQL, create the `traqrcloud` database and a user. Set `DATABASE_URL` in `.env` (e.g. `mysql://user:password@localhost:3306/traqrcloud`).
2. **Environment** — Copy `.env.example` to `.env` and set at least:
   - `DATABASE_URL` — MySQL connection string.
   - `ADMIN_API_KEY` (recommended) — Stops anyone without the key from creating activation codes. Use a long random secret; pass it as `X-Admin-Key` when calling the admin API or when using the portal/store “Create activation code” (if you add that header in your setup).
   - `CLOUD_API_URL` — Set this to the public API URL (no trailing slash), e.g. `https://cloud.traqr.co.uk/api`, if the POS or docs need to reference it.
3. **Build and run** — From the project root: `cargo build --release`, then run the binary (e.g. `./target/release/cloud_api`). The app binds to `0.0.0.0:8080` by default. Put it behind a reverse proxy (e.g. nginx) for HTTPS.
4. **Portal login** — After migrations, log in at `/login.html`. Change or remove the demo user (`admin@traqr.co.uk` / `demo123`) for production (e.g. add real users and remove or update the seed).
5. **Static files** — The server serves `web/public` at `/`. Build CSS first: `npm run build:css`. Optionally set `WEB_ROOT` in `.env` to point at a different directory.

Once these are done, the portal (organizations, stores, devices, menus, orders, command center) and the POS API (activate, sync, commands) are ready to use.

### Production readiness checklist

- **HTTPS** — Run the app behind a reverse proxy (nginx, Caddy) with TLS. Do not expose the Rust server directly on the internet.
- **Secrets** — Use strong, unique values for `DATABASE_URL`, `ADMIN_API_KEY`, and any session secrets. Never commit `.env` to git.
- **Demo user** — Remove or change the seed user (`admin@traqr.co.uk` / `demo123`) and use strong passwords for real super-admin accounts.
- **Uploads** — Set `UPLOAD_DIR` to a path the process can write to (avatars, blog images). Ensure the directory exists and has correct permissions.
- **Logging** — Set `RUST_LOG` (e.g. `info` or `warn`) to control log volume in production.
- **Database** — Use a dedicated MySQL instance, backups, and a connection pool size appropriate for your traffic.
- **Session cookie** — The app sets a session cookie; ensure your domain and SameSite/Secure settings match your HTTPS setup when behind a proxy.

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
- `POST /api/admin/activation-keys` — create org/store if needed and issue an activation code (optional: set `ADMIN_API_KEY` and send `X-Admin-Key`)

**POS configuration:** Set **CLOUD_API_URL** on the POS to **`https://cloud.traqr.co.uk/api`** (no trailing slash). The POS then calls `POST /device/activate`, `POST /sync/events`, `GET /sync/commands`, `POST /sync/commands/ack` relative to that base.

**Activation codes:** Create a code via the admin API, Super Admin, or script (see `scripts/README.md`). The code is short (e.g. `traqr-1a2b-3c4d-5e6f-7a8b`) so staff can type it on the till. It is shown once; the customer enters it in the POS (Settings → Cloud). When the portal sends `void_order` or `refund_order` commands, the command body must include the POS local order id as `local_order_id` (or `order_id`); the portal gets this from the orders read model (`orders.local_order_id`).



