-- Admin login: admin@traqr.co.uk / demo123 (bcrypt hash)
INSERT INTO cloud_users (id, email, password_hash, display_name, status)
SELECT UUID(), 'admin@traqr.co.uk', '$2b$12$zEPt9i7VxPqW7Y10xQgpIOMunTlusAhQm3YncJz0J0UnMWzSW8mbO', 'Demo Admin', 'active'
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM cloud_users WHERE LOWER(email) = 'admin@traqr.co.uk');
