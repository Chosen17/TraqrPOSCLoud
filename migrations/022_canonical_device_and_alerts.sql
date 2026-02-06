-- Canonical device per store + config alerts for non-primary changes

ALTER TABLE stores
  ADD COLUMN canonical_device_id CHAR(36) NULL AFTER timezone,
  ADD CONSTRAINT fk_stores_canonical_device
    FOREIGN KEY (canonical_device_id) REFERENCES devices(id)
    ON DELETE SET NULL;

CREATE TABLE device_config_alerts (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  event_type VARCHAR(100) NOT NULL,
  details TEXT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

