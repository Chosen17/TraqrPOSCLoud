CREATE TABLE cloud_users (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  email VARCHAR(255) NOT NULL,
  password_hash VARCHAR(255) NULL,
  display_name VARCHAR(255) NULL,
  status VARCHAR(50) NOT NULL DEFAULT 'active',
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  last_login_at DATETIME(3) NULL,
  UNIQUE KEY uq_cloud_users_email (email)
);

CREATE TABLE cloud_roles (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  code VARCHAR(100) NOT NULL,
  description VARCHAR(255) NULL,
  UNIQUE KEY uq_cloud_roles_code (code)
);

CREATE TABLE org_memberships (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  user_id CHAR(36) NOT NULL,
  franchise_id CHAR(36) NULL,
  role_id CHAR(36) NOT NULL,
  status VARCHAR(50) NOT NULL DEFAULT 'active',
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_org_memberships (org_id, user_id, role_id, franchise_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (user_id) REFERENCES cloud_users(id) ON DELETE CASCADE,
  FOREIGN KEY (franchise_id) REFERENCES franchises(id) ON DELETE SET NULL,
  FOREIGN KEY (role_id) REFERENCES cloud_roles(id)
);

CREATE TABLE store_memberships (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  user_id CHAR(36) NOT NULL,
  role_id CHAR(36) NOT NULL,
  status VARCHAR(50) NOT NULL DEFAULT 'active',
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_store_memberships (store_id, user_id, role_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (user_id) REFERENCES cloud_users(id) ON DELETE CASCADE,
  FOREIGN KEY (role_id) REFERENCES cloud_roles(id)
);
