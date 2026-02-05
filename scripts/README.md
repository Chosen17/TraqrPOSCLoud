# Scripts

## Create activation code

Create an activation code so a till can connect to the cloud. The code is shown once — give it to staff to enter on the till (Settings → Cloud).

**Using the script (easiest):**

```bash
./scripts/create-activation-key.sh                          # uses http://localhost:8080/api
./scripts/create-activation-key.sh https://cloud.traqr.co.uk/api
export ADMIN_KEY=your-secret && ./scripts/create-activation-key.sh   # if server requires admin key
```

Requires `jq`. The script creates a demo org/store and prints the activation code.

**Using the API (curl):**

```bash
curl -X POST http://localhost:8080/api/admin/activation-keys \
  -H "Content-Type: application/json" \
  -d '{"org_name":"Acme","org_slug":"acme","store_name":"Store 1","scope_type":"store"}'
```

Response includes `activation_key` (the code — show once to the customer). If the server has `ADMIN_API_KEY` set, add: `-H "X-Admin-Key: YOUR_ADMIN_KEY"`.

---

## Fix "migration 5 is partially applied"

If `sqlx migrate run` fails with:

```text
error: migration 5 is partially applied; fix and remove row from `_sqlx_migrations` table
```

run the fix script against your MySQL database, then run migrations again:

```bash
# From project root, with DATABASE_URL set (or pass connection details)
mysql traqrcloud -u youruser -p < scripts/fix-migration-5.sql
sqlx migrate run
```

Or run the SQL in `scripts/fix-migration-5.sql` in any MySQL client connected to the `traqrcloud` database.

---

## Database setup

Create the database and app user so the Traqr Cloud app can connect.

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
