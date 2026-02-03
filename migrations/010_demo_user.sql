-- Demo login user (for development). Remove or change in production.
-- Password: demo123
INSERT INTO cloud_users (email, password_hash, display_name, status)
SELECT 'admin@traqr.co.uk', crypt('demo123', gen_salt('bf')), 'Demo Admin', 'active'
WHERE NOT EXISTS (SELECT 1 FROM cloud_users WHERE email = 'admin@traqr.co.uk');
