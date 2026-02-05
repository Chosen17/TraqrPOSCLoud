-- Assign existing super_admin (demo admin) to sa_owner so they are the main owner.
-- Run after 016 so sa_owner role exists.

UPDATE org_memberships om
JOIN organizations o ON o.id = om.org_id AND o.slug = 'traqr-internal'
JOIN cloud_roles r_old ON r_old.id = om.role_id AND r_old.code = 'super_admin'
JOIN cloud_roles r_new ON r_new.code = 'sa_owner'
SET om.role_id = r_new.id;
