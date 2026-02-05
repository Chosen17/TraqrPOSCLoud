-- Read model: mirrors POS orders/transactions/receipts for reporting and sync.
CREATE TABLE orders (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  local_order_id VARCHAR(255) NOT NULL,
  status VARCHAR(50) NOT NULL DEFAULT 'open',
  total_cents BIGINT NULL,
  occurred_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_orders_store_device_local (store_id, device_id, local_order_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE TABLE order_items (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  order_id CHAR(36) NOT NULL,
  local_item_id VARCHAR(255) NULL,
  product_ref VARCHAR(255) NULL,
  quantity DECIMAL(18,4) NOT NULL DEFAULT 1,
  unit_price_cents BIGINT NULL,
  line_total_cents BIGINT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE CASCADE
);

CREATE TABLE transactions (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  order_id CHAR(36) NULL,
  local_transaction_id VARCHAR(255) NOT NULL,
  kind VARCHAR(100) NOT NULL,
  amount_cents BIGINT NOT NULL,
  occurred_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_transactions_store_device_local (store_id, device_id, local_transaction_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE,
  FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE SET NULL
);

CREATE TABLE receipts (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  order_id CHAR(36) NULL,
  transaction_id CHAR(36) NULL,
  local_receipt_id VARCHAR(255) NOT NULL,
  occurred_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_receipts_store_device_local (store_id, device_id, local_receipt_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE,
  FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE SET NULL,
  FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE SET NULL
);

CREATE TABLE order_events (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  order_id CHAR(36) NOT NULL,
  event_type VARCHAR(100) NOT NULL,
  event_body JSON NULL,
  occurred_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE CASCADE
);

CREATE INDEX idx_orders_org_store ON orders(org_id, store_id);
CREATE INDEX idx_orders_occurred ON orders(occurred_at);
CREATE INDEX idx_order_items_order ON order_items(order_id);
CREATE INDEX idx_transactions_org_store ON transactions(org_id, store_id);
CREATE INDEX idx_transactions_occurred ON transactions(occurred_at);
CREATE INDEX idx_receipts_org_store ON receipts(org_id, store_id);
CREATE INDEX idx_order_events_order ON order_events(order_id);
