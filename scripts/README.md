# Database setup

Create the `traqr_cloud` database and app user so the Traqr Cloud app can connect.

## One-time setup

**Option 1 — run as postgres (Linux, if you have sudo):**

```bash
cd /var/rustApps/TraqrPOSCLoud
sudo -u postgres ./scripts/setup-db.sh
```

**Option 2 — run with your postgres password:**

If your postgres user has a password (e.g. you set it during install):

```bash
cd /var/rustApps/TraqrPOSCLoud
export PGPASSWORD=your_postgres_password
./scripts/setup-db.sh
```

**Option 3 — run the SQL by hand:**

1. Create the database: `createdb -U postgres traqr_cloud` (or use pgAdmin / another client).
2. Then run the SQL in `scripts/setup-db.sql` while connected to `traqr_cloud` as a superuser.

## After setup

- The app user is **`traqr_app`** with password **`traqr_app_pass`**.
- The project `.env` is already set to use it: `DATABASE_URL=postgres://traqr_app:traqr_app_pass@localhost:5432/traqr_cloud`.
- Run the app: `cargo run`. Migrations run on startup. Then open http://localhost:8080/login.html and log in with **admin@traqr.co.uk** / **demo123**.

Change `traqr_app_pass` in production (update the password in Postgres and in `DATABASE_URL`).
