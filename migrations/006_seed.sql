INSERT INTO cloud_roles (code, description) VALUES
 ('head_office_admin', 'Head office admin'),
 ('head_office_ops', 'Head office operations'),
 ('store_manager', 'Store manager'),
 ('finance', 'Finance'),
 ('support', 'Support')
ON CONFLICT (code) DO NOTHING;
