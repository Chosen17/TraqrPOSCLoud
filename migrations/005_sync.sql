CREATE TABLE device_event_log (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  event_id CHAR(36) NOT NULL,
  seq BIGINT NULL,
  event_type VARCHAR(100) NOT NULL,
  event_body JSON NOT NULL,
  occurred_at DATETIME(3) NOT NULL,
  received_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_device_event_log_device_event (device_id, event_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE TABLE device_sync_state (
  device_id CHAR(36) PRIMARY KEY,
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  last_ack_seq BIGINT NULL,
  updated_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)) ON UPDATE CURRENT_TIMESTAMP(3),
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE,
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE
);

CREATE TABLE device_command_queue (
  command_id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  command_type VARCHAR(100) NOT NULL,
  command_body JSON NOT NULL,
  status VARCHAR(50) NOT NULL DEFAULT 'queued',
  `sensitive` TINYINT(1) NOT NULL DEFAULT 0,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE TABLE approvals (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  command_id CHAR(36) NOT NULL,
  approver_user_id CHAR(36) NOT NULL,
  decision VARCHAR(50) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_approvals_command_approver (command_id, approver_user_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (command_id) REFERENCES device_command_queue(command_id) ON DELETE CASCADE,
  FOREIGN KEY (approver_user_id) REFERENCES cloud_users(id) ON DELETE CASCADE,
  CHECK (decision IN ('approve','reject'))
);
