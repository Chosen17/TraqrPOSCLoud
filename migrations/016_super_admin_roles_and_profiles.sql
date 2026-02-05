-- Super admin hierarchy: owner (can override), manager (team + blogs), sales_rep (customers).
-- Profiles for Traqr staff: avatar, phone, etc.

INSERT IGNORE INTO cloud_roles (id, code, description) VALUES
 (UUID(), 'sa_owner', 'Traqr super admin owner – full access, can override'),
 (UUID(), 'sa_manager', 'Traqr super admin manager – team, blogs, customers'),
 (UUID(), 'sa_sales_rep', 'Traqr super admin sales rep – customers and profile');

CREATE TABLE IF NOT EXISTS cloud_user_profiles (
  user_id CHAR(36) PRIMARY KEY,
  avatar_path VARCHAR(512) NULL COMMENT 'Path relative to uploads root, e.g. avatars/uuid.jpg',
  phone VARCHAR(50) NULL,
  job_title VARCHAR(255) NULL,
  bio TEXT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  updated_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)) ON UPDATE CURRENT_TIMESTAMP(3),
  FOREIGN KEY (user_id) REFERENCES cloud_users(id) ON DELETE CASCADE
);
