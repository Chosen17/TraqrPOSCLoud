CREATE TABLE plans (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  code VARCHAR(100) NOT NULL,
  name VARCHAR(255) NOT NULL,
  cloud_sync_included TINYINT(1) NOT NULL DEFAULT 0,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_plans_code (code)
);

CREATE TABLE org_entitlements (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  plan_id CHAR(36) NOT NULL,
  cloud_sync_add_on TINYINT(1) NOT NULL DEFAULT 0,
  device_limit INT NULL,
  valid_from DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  valid_until DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_org_entitlements_org_plan (org_id, plan_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (plan_id) REFERENCES plans(id)
);

CREATE TABLE device_entitlements (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  cloud_sync_enabled TINYINT(1) NOT NULL DEFAULT 0,
  valid_from DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  valid_until DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_device_entitlements_device (device_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE INDEX idx_org_entitlements_org ON org_entitlements(org_id);
CREATE INDEX idx_device_entitlements_device ON device_entitlements(device_id);

INSERT IGNORE INTO plans (id, code, name, cloud_sync_included) VALUES
  (UUID(), 'default', 'Default', 0);
