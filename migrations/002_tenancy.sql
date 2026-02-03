CREATE TABLE organizations (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  name text NOT NULL,
  slug text NOT NULL UNIQUE,
  status text NOT NULL DEFAULT 'active',
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE franchises (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  name text NOT NULL,
  code text NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(org_id, name)
);

CREATE TABLE stores (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  franchise_id uuid NULL REFERENCES franchises(id) ON DELETE SET NULL,
  name text NOT NULL,
  code text NULL,
  timezone text NOT NULL DEFAULT 'Europe/London',
  address_json jsonb NULL,
  status text NOT NULL DEFAULT 'active',
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_franchises_org_id ON franchises(org_id);
CREATE INDEX idx_stores_org_id ON stores(org_id);
CREATE INDEX idx_stores_franchise_id ON stores(franchise_id);
