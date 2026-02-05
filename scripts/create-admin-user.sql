-- Create or fix admin user: admin@traqr.co.uk / demo123 (bcrypt hash)
-- Run: mysql -u owlmailer -p traqrcloud < scripts/create-admin-user.sql
-- Or: mysql -u owlmailer -p traqrcloud -e "SOURCE scripts/create-admin-user.sql"

INSERT INTO cloud_users (id, email, password_hash, display_name, status)
VALUES (
  UUID(),
  'admin@traqr.co.uk',
  '$2b$12$zEPt9i7VxPqW7Y10xQgpIOMunTlusAhQm3YncJz0J0UnMWzSW8mbO',
  'Demo Admin',
  'active'
)
ON DUPLICATE KEY UPDATE
  password_hash = VALUES(password_hash),
  display_name = VALUES(display_name),
  status = VALUES(status);
