-- Command status: queued | delivered | acked | failed | expired
ALTER TABLE device_command_queue
  ADD COLUMN expires_at DATETIME(3) NULL,
  ADD COLUMN delivered_at DATETIME(3) NULL,
  ADD COLUMN ack_result JSON NULL,
  ADD CONSTRAINT chk_device_command_queue_status
    CHECK (status IN ('queued', 'delivered', 'acked', 'failed', 'expired'));

CREATE INDEX idx_device_command_queue_device_status
  ON device_command_queue(device_id, status);
