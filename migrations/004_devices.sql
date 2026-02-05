CREATE TABLE devices (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  device_label VARCHAR(255) NULL,
  hardware_fingerprint VARCHAR(255) NULL,
  status VARCHAR(50) NOT NULL DEFAULT 'active',
  last_seen_at DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE
);

CREATE TABLE device_activation_keys (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  scope_type VARCHAR(50) NOT NULL,
  scope_id CHAR(36) NULL,
  key_hash VARCHAR(255) NOT NULL,
  is_multi_use TINYINT(1) NOT NULL DEFAULT 0,
  max_uses INT NULL,
  uses_count INT NOT NULL DEFAULT 0,
  expires_at DATETIME(3) NULL,
  revoked_at DATETIME(3) NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_device_activation_keys_org_hash (org_id, key_hash),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  CHECK (scope_type IN ('store','franchise','org'))
);

CREATE UNIQUE INDEX idx_device_activation_keys_key_hash_unique
  ON device_activation_keys(key_hash);

CREATE TABLE device_tokens (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  device_id CHAR(36) NOT NULL,
  token_hash VARCHAR(255) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  expires_at DATETIME(3) NULL,
  revoked_at DATETIME(3) NULL,
  UNIQUE KEY uq_device_tokens_token_hash (token_hash),
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);
