CREATE TABLE cloud_users (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  email citext NOT NULL UNIQUE,
  password_hash text NULL,
  display_name text NULL,
  status text NOT NULL DEFAULT 'active',
  created_at timestamptz NOT NULL DEFAULT now(),
  last_login_at timestamptz NULL
);

CREATE TABLE cloud_roles (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  code text NOT NULL UNIQUE,
  description text NULL
);

CREATE TABLE org_memberships (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  user_id uuid NOT NULL REFERENCES cloud_users(id) ON DELETE CASCADE,
  franchise_id uuid NULL REFERENCES franchises(id) ON DELETE SET NULL,
  role_id uuid NOT NULL REFERENCES cloud_roles(id),
  status text NOT NULL DEFAULT 'active',
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(org_id, user_id, role_id, franchise_id)
);

CREATE TABLE store_memberships (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  store_id uuid NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
  user_id uuid NOT NULL REFERENCES cloud_users(id) ON DELETE CASCADE,
  role_id uuid NOT NULL REFERENCES cloud_roles(id),
  status text NOT NULL DEFAULT 'active',
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(store_id, user_id, role_id)
);
