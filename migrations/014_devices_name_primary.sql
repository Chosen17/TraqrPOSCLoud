-- Device display name and primary (authority) flag from POS activate and device_updated events.
ALTER TABLE devices
  ADD COLUMN device_name VARCHAR(255) NULL AFTER device_label,
  ADD COLUMN is_primary TINYINT(1) NOT NULL DEFAULT 0 AFTER device_name;
