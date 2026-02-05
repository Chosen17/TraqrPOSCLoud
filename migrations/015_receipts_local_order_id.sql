-- Store POS order_id on receipts so we can link receipts that arrive before order_created,
-- and so we can fetch receipts by (store_id, device_id, local_order_id) for order detail.
ALTER TABLE receipts
  ADD COLUMN local_order_id VARCHAR(255) NULL AFTER order_id;

CREATE INDEX idx_receipts_store_device_local_order
  ON receipts(store_id, device_id, local_order_id);
