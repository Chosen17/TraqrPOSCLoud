CREATE TABLE device_event_log (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  store_id uuid NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
  device_id uuid NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  event_id uuid NOT NULL,
  seq bigint NULL,
  event_type text NOT NULL,
  event_body jsonb NOT NULL,
  occurred_at timestamptz NOT NULL,
  received_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(device_id, event_id)
);

CREATE TABLE device_sync_state (
  device_id uuid PRIMARY KEY REFERENCES devices(id) ON DELETE CASCADE,
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  store_id uuid NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
  last_ack_seq bigint NULL,
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE device_command_queue (
  command_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  store_id uuid NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
  device_id uuid NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  command_type text NOT NULL,
  command_body jsonb NOT NULL,
  status text NOT NULL DEFAULT 'queued',
  sensitive boolean NOT NULL DEFAULT false,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE approvals (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  command_id uuid NOT NULL REFERENCES device_command_queue(command_id) ON DELETE CASCADE,
  approver_user_id uuid NOT NULL REFERENCES cloud_users(id) ON DELETE CASCADE,
  decision text NOT NULL CHECK (decision IN ('approve','reject')),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(command_id, approver_user_id)
);
