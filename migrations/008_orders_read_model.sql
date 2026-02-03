-- Read model: mirrors POS orders/transactions/receipts for reporting and sync.
-- Populated from device_event_log (Phase 2).

CREATE TABLE orders (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  store_id uuid NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
  device_id uuid NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  local_order_id text NOT NULL,
  status text NOT NULL DEFAULT 'open',
  total_cents bigint NULL,
  occurred_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(store_id, device_id, local_order_id)
);

CREATE TABLE order_items (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id uuid NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
  local_item_id text NULL,
  product_ref text NULL,
  quantity numeric NOT NULL DEFAULT 1,
  unit_price_cents bigint NULL,
  line_total_cents bigint NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE transactions (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  store_id uuid NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
  device_id uuid NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  order_id uuid NULL REFERENCES orders(id) ON DELETE SET NULL,
  local_transaction_id text NOT NULL,
  kind text NOT NULL,
  amount_cents bigint NOT NULL,
  occurred_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(store_id, device_id, local_transaction_id)
);

CREATE TABLE receipts (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  store_id uuid NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
  device_id uuid NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  order_id uuid NULL REFERENCES orders(id) ON DELETE SET NULL,
  transaction_id uuid NULL REFERENCES transactions(id) ON DELETE SET NULL,
  local_receipt_id text NOT NULL,
  occurred_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(store_id, device_id, local_receipt_id)
);

CREATE TABLE order_events (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  store_id uuid NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
  order_id uuid NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
  event_type text NOT NULL,
  event_body jsonb NULL,
  occurred_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_orders_org_store ON orders(org_id, store_id);
CREATE INDEX idx_orders_occurred ON orders(occurred_at);
CREATE INDEX idx_order_items_order ON order_items(order_id);
CREATE INDEX idx_transactions_org_store ON transactions(org_id, store_id);
CREATE INDEX idx_transactions_occurred ON transactions(occurred_at);
CREATE INDEX idx_receipts_org_store ON receipts(org_id, store_id);
CREATE INDEX idx_order_events_order ON order_events(order_id);
