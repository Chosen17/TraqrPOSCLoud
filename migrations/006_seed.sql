INSERT IGNORE INTO cloud_roles (id, code, description) VALUES
 (UUID(), 'super_admin', 'Traqr internal super admin'),
 (UUID(), 'head_office_admin', 'Head office admin'),
 (UUID(), 'head_office_ops', 'Head office operations'),
 (UUID(), 'store_manager', 'Store manager'),
 (UUID(), 'finance', 'Finance'),
 (UUID(), 'support', 'Support');
