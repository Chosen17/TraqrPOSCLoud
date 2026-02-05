-- Fix "migration 5 is partially applied" so you can run: sqlx migrate run
-- Run this with MySQL, then run from project root: sqlx migrate run
--
-- Usage: mysql -u youruser -p traqrcloud < scripts/fix-migration-5.sql
-- Or:    mysql traqrcloud -e "source /var/TraqrPOSCLoud/scripts/fix-migration-5.sql"

-- Drop tables created by migration 005_sync.sql (reverse order due to FKs)
SET FOREIGN_KEY_CHECKS = 0;
DROP TABLE IF EXISTS approvals;
DROP TABLE IF EXISTS device_command_queue;
DROP TABLE IF EXISTS device_sync_state;
DROP TABLE IF EXISTS device_event_log;
SET FOREIGN_KEY_CHECKS = 1;

-- Remove the partial migration record so sqlx will re-apply migration 5
DELETE FROM _sqlx_migrations WHERE version = 5;
