-- Command status: queued | delivered | acked | failed | expired
-- Sensitive commands stay queued until two-person approval; then deliverable.

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'device_command_queue_status_check'
  ) THEN
    ALTER TABLE device_command_queue
      ADD CONSTRAINT device_command_queue_status_check
      CHECK (status IN ('queued', 'delivered', 'acked', 'failed', 'expired'));
  END IF;
END $$;

ALTER TABLE device_command_queue
  ADD COLUMN IF NOT EXISTS expires_at timestamptz NULL,
  ADD COLUMN IF NOT EXISTS delivered_at timestamptz NULL,
  ADD COLUMN IF NOT EXISTS ack_result jsonb NULL;

CREATE INDEX IF NOT EXISTS idx_device_command_queue_device_status
  ON device_command_queue(device_id, status)
  WHERE status IN ('queued', 'delivered');
