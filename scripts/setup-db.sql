-- Create app user and grants for Traqr Cloud.
-- Run after connecting to traqr_cloud: psql -U postgres -d traqr_cloud -f scripts/setup-db.sql

-- Create app user (ignore error if exists)
DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'traqr_app') THEN
    CREATE ROLE traqr_app WITH LOGIN PASSWORD 'traqr_app_pass';
  ELSE
    ALTER ROLE traqr_app WITH PASSWORD 'traqr_app_pass';
  END IF;
END
$$;

-- Grant connect and usage
GRANT CONNECT ON DATABASE traqr_cloud TO traqr_app;
GRANT USAGE ON SCHEMA public TO traqr_app;
GRANT CREATE ON SCHEMA public TO traqr_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO traqr_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO traqr_app;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO traqr_app;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO traqr_app;
