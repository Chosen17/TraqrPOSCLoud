-- Create an internal Traqr organization and attach the demo admin user
-- as a super_admin. This keeps super-admin privileges explicit in data.

-- Create Traqr Internal org if it doesn't exist
INSERT INTO organizations (id, name, slug, status)
SELECT UUID(), 'Traqr Internal', 'traqr-internal', 'active'
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM organizations WHERE slug = 'traqr-internal');

-- Attach admin@traqr.co.uk to Traqr Internal with super_admin role
INSERT INTO org_memberships (id, org_id, user_id, franchise_id, role_id, status)
SELECT
  UUID() AS id,
  o.id AS org_id,
  u.id AS user_id,
  NULL AS franchise_id,
  r.id AS role_id,
  'active' AS status
FROM organizations o
JOIN cloud_users u ON LOWER(u.email) = 'admin@traqr.co.uk'
JOIN cloud_roles r ON r.code = 'super_admin'
WHERE o.slug = 'traqr-internal'
  AND NOT EXISTS (
    SELECT 1 FROM org_memberships om
    WHERE om.org_id = o.id
      AND om.user_id = u.id
      AND om.role_id = r.id
  );

