#!/usr/bin/env bash
# Fix "migration 5 is partially applied" by running the SQL and then run: sqlx migrate run
set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Load .env from project root if DATABASE_URL not set
[[ -z "$DATABASE_URL" && -f "$PROJECT_ROOT/.env" ]] && set -a && source "$PROJECT_ROOT/.env" && set +a

if [[ -n "$DATABASE_URL" ]]; then
  # Parse mysql://user:password@host:port/database (password may contain : or @)
  if [[ "$DATABASE_URL" =~ ^mysql://([^:]+):([^@]+)@([^:/]+):([0-9]+)/([^?&#]*) ]]; then
    MUSER="${BASH_REMATCH[1]}"
    MPASS="${BASH_REMATCH[2]}"
    MHOST="${BASH_REMATCH[3]}"
    MPORT="${BASH_REMATCH[4]}"
    MDB="${BASH_REMATCH[5]}"
    echo "Running fix against $MDB@$MHOST:$MPORT as $MUSER ..."
    MYSQL_PWD="$MPASS" mysql -h "$MHOST" -P "$MPORT" -u "$MUSER" "$MDB" < "$SCRIPT_DIR/fix-migration-5.sql"
    echo "Done. Run: sqlx migrate run"
    exit 0
  fi
fi

echo "Run the fix SQL against your database, then: sqlx migrate run"
echo ""
echo "Option 1 - if DATABASE_URL is set (e.g. in .env):"
echo "  export DATABASE_URL='mysql://user:password@localhost:3306/traqrcloud'  # or: set -a && source .env && set +a"
echo "  $0"
echo ""
echo "Option 2 - run MySQL yourself:"
echo "  mysql -u owlmailer -p traqrcloud < $SCRIPT_DIR/fix-migration-5.sql"
echo ""
exit 1
