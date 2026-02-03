CREATE TABLE devices (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  store_id uuid NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
  device_label text NULL,
  hardware_fingerprint text NULL,
  status text NOT NULL DEFAULT 'active',
  last_seen_at timestamptz NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE device_activation_keys (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  scope_type text NOT NULL CHECK (scope_type IN ('store','franchise','org')),
  scope_id uuid NULL,
  key_hash text NOT NULL,
  is_multi_use boolean NOT NULL DEFAULT false,
  max_uses int NULL,
  uses_count int NOT NULL DEFAULT 0,
  expires_at timestamptz NULL,
  revoked_at timestamptz NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(org_id, key_hash)
);

-- Allow lookup by key_hash only when key is globally unique per issuance
CREATE UNIQUE INDEX idx_device_activation_keys_key_hash_unique
  ON device_activation_keys(key_hash)
  WHERE revoked_at IS NULL;

CREATE TABLE device_tokens (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  device_id uuid NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  token_hash text NOT NULL UNIQUE,
  created_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NULL,
  revoked_at timestamptz NULL
);
