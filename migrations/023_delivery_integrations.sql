-- Delivery integrations and normalized delivery orders for third-party platforms

CREATE TABLE delivery_integrations (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  provider VARCHAR(50) NOT NULL,
  status VARCHAR(50) NOT NULL DEFAULT 'disconnected',
  api_key_enc TEXT NULL,
  client_id_enc TEXT NULL,
  client_secret_enc TEXT NULL,
  access_token_enc TEXT NULL,
  refresh_token_enc TEXT NULL,
  token_expires_at DATETIME(3) NULL,
  webhook_secret_enc TEXT NULL,
  provider_store_reference VARCHAR(255) NULL,
  last_sync_at DATETIME(3) NULL,
  last_error_message TEXT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  updated_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)) ON UPDATE CURRENT_TIMESTAMP(3),
  UNIQUE KEY uq_delivery_integrations_store_provider (store_id, provider),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE
);

CREATE TABLE delivery_orders (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  integration_id CHAR(36) NOT NULL,
  provider VARCHAR(50) NOT NULL,
  provider_order_id VARCHAR(255) NOT NULL,
  status VARCHAR(50) NOT NULL,
  customer_name VARCHAR(255) NULL,
  customer_phone VARCHAR(50) NULL,
  delivery_address JSON NULL,
  items JSON NOT NULL,
  subtotal_cents BIGINT NULL,
  tax_cents BIGINT NULL,
  delivery_fee_cents BIGINT NULL,
  total_cents BIGINT NULL,
  notes TEXT NULL,
  raw_payload JSON NOT NULL,
  received_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  updated_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)) ON UPDATE CURRENT_TIMESTAMP(3),
  UNIQUE KEY uq_delivery_orders_provider_order (provider, provider_order_id),
  INDEX idx_delivery_orders_store_received (store_id, received_at),
  INDEX idx_delivery_orders_integration_received (integration_id, received_at),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (integration_id) REFERENCES delivery_integrations(id) ON DELETE CASCADE
);

CREATE TABLE delivery_integration_logs (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  provider VARCHAR(50) NOT NULL,
  store_id CHAR(36) NULL,
  integration_id CHAR(36) NULL,
  request_url TEXT NULL,
  request_method VARCHAR(20) NULL,
  request_payload JSON NULL,
  response_status INT NULL,
  response_payload JSON NULL,
  error_message TEXT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3))
);

