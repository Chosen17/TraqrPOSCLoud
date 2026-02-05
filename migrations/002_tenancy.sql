CREATE TABLE organizations (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  name VARCHAR(255) NOT NULL,
  slug VARCHAR(255) NOT NULL,
  status VARCHAR(50) NOT NULL DEFAULT 'active',
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_organizations_slug (slug)
);

CREATE TABLE franchises (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  name VARCHAR(255) NOT NULL,
  code VARCHAR(100) NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_franchises_org_name (org_id, name),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE
);

CREATE TABLE stores (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  franchise_id CHAR(36) NULL,
  name VARCHAR(255) NOT NULL,
  code VARCHAR(100) NULL,
  timezone VARCHAR(100) NOT NULL DEFAULT 'Europe/London',
  address_json JSON NULL,
  status VARCHAR(50) NOT NULL DEFAULT 'active',
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (franchise_id) REFERENCES franchises(id) ON DELETE SET NULL
);

CREATE INDEX idx_franchises_org_id ON franchises(org_id);
CREATE INDEX idx_stores_org_id ON stores(org_id);
CREATE INDEX idx_stores_franchise_id ON stores(franchise_id);
