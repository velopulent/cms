-- Roles v2: instance operators (owner/admin) + site collaborators (editor/viewer).

-- Widen users.instance_role to allow instance_admin.
ALTER TABLE users DROP CHECK chk_users_instance_role;
ALTER TABLE users ADD CONSTRAINT chk_users_instance_role
    CHECK (instance_role IS NULL OR instance_role IN ('instance_owner', 'instance_admin'));

-- Restrict site_members.role to editor/viewer; legacy owner/admin collapse to editor
-- (those operators now act through their instance role, not site membership).
UPDATE site_members SET role = 'editor' WHERE role IN ('owner', 'admin');
ALTER TABLE site_members DROP CHECK site_members_chk_1;
ALTER TABLE site_members ADD CONSTRAINT site_members_role_check
    CHECK (role IN ('editor', 'viewer'));
