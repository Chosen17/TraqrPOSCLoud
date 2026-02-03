-- Minimal entitlements for Phase 1: cloud sync add-on gating.
-- Full billing (plans, subscriptions, metering) in Phase 5.

CREATE TABLE plans (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  code text NOT NULL UNIQUE,
  name text NOT NULL,
  cloud_sync_included boolean NOT NULL DEFAULT false,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE org_entitlements (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  plan_id uuid NOT NULL REFERENCES plans(id),
  cloud_sync_add_on boolean NOT NULL DEFAULT false,
  device_limit int NULL,
  valid_from timestamptz NOT NULL DEFAULT now(),
  valid_until timestamptz NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(org_id, plan_id)
);

CREATE TABLE device_entitlements (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  store_id uuid NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
  device_id uuid NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  cloud_sync_enabled boolean NOT NULL DEFAULT false,
  valid_from timestamptz NOT NULL DEFAULT now(),
  valid_until timestamptz NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(device_id)
);

CREATE INDEX idx_org_entitlements_org ON org_entitlements(org_id);
CREATE INDEX idx_device_entitlements_device ON device_entitlements(device_id);

-- Seed default plan (no cloud sync by default; add-on required)
INSERT INTO plans (code, name, cloud_sync_included) VALUES
  ('default', 'Default', false)
ON CONFLICT (code) DO NOTHING;
