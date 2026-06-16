-- Roles v2: instance operators (owner/admin) + site collaborators (editor/viewer).

-- Widen users.instance_role to allow instance_admin.
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_instance_role_check;
ALTER TABLE users ADD CONSTRAINT users_instance_role_check
    CHECK (instance_role IS NULL OR instance_role IN ('instance_owner', 'instance_admin'));

-- Restrict site_members.role to editor/viewer; legacy owner/admin collapse to editor
-- (those operators now act through their instance role, not site membership).
UPDATE site_members SET role = 'editor' WHERE role IN ('owner', 'admin');
ALTER TABLE site_members DROP CONSTRAINT IF EXISTS site_members_role_check;
ALTER TABLE site_members ADD CONSTRAINT site_members_role_check
    CHECK (role IN ('editor', 'viewer'));
