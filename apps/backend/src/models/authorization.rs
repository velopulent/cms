use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Instance-level (operator) roles, spanning the whole installation.
/// `InstanceOwner` is strictly above `InstanceAdmin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstanceRole {
    InstanceAdmin,
    InstanceOwner,
}

impl InstanceRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstanceAdmin => "instance_admin",
            Self::InstanceOwner => "instance_owner",
        }
    }
}

impl FromStr for InstanceRole {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "instance_admin" => Ok(Self::InstanceAdmin),
            "instance_owner" => Ok(Self::InstanceOwner),
            _ => Err(format!("Unknown instance role '{value}'")),
        }
    }
}

impl fmt::Display for InstanceRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Site-scoped (collaborator) roles, attached per site via `site_members`.
/// Site administration is performed by instance operators, not site roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SiteRole {
    Viewer,
    Editor,
}

impl SiteRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Editor => "editor",
        }
    }
}

impl FromStr for SiteRole {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "viewer" => Ok(Self::Viewer),
            "editor" => Ok(Self::Editor),
            _ => Err(format!("Unknown site role '{value}'")),
        }
    }
}

impl fmt::Display for SiteRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    InstanceManage,
    SiteCreate,
    SiteRead,
    SiteManage,
    SiteDelete,
    ContentRead,
    ContentWrite,
    SchemaRead,
    SchemaWrite,
    FilesRead,
    FilesWrite,
    WebhooksRead,
    WebhooksWrite,
    ApiKeysManage,
    MembersRead,
    MembersManage,
    /// Grant or revoke instance roles (owner-only).
    InstanceRolesGrant,
}

pub struct Authorizer;

impl Authorizer {
    /// Instance operators. Owner is a strict superset of Admin; the only Owner-only
    /// powers are granting instance roles (and, later, instance backup/restore).
    pub const fn allows_instance(role: Option<InstanceRole>, action: Action) -> bool {
        match role {
            Some(InstanceRole::InstanceOwner) => matches!(
                action,
                Action::InstanceManage | Action::SiteCreate | Action::SiteDelete | Action::InstanceRolesGrant
            ),
            Some(InstanceRole::InstanceAdmin) => {
                matches!(action, Action::InstanceManage | Action::SiteCreate | Action::SiteDelete)
            }
            None => false,
        }
    }

    /// Instance operators (Owner/Admin) have full authority over every site.
    /// This is the override that lets operators manage all sites without membership.
    pub const fn allows_site_as_instance(role: Option<InstanceRole>, _action: Action) -> bool {
        matches!(
            role,
            Some(InstanceRole::InstanceOwner | InstanceRole::InstanceAdmin)
        )
    }

    /// Site-scoped collaborator authority. Editors write content/files; Viewers read.
    /// Anything beyond that (site/schema/webhook/key/member management) is operator-only.
    pub const fn allows_site(role: SiteRole, action: Action) -> bool {
        match action {
            Action::SiteRead
            | Action::ContentRead
            | Action::SchemaRead
            | Action::FilesRead
            | Action::WebhooksRead
            | Action::MembersRead => true,
            Action::ContentWrite | Action::FilesWrite => matches!(role, SiteRole::Editor),
            _ => false,
        }
    }

    pub const fn allows_api_key(can_write: bool, action: Action) -> bool {
        match action {
            Action::SiteRead | Action::ContentRead | Action::SchemaRead | Action::FilesRead | Action::WebhooksRead => {
                true
            }
            Action::SiteManage
            | Action::ContentWrite
            | Action::SchemaWrite
            | Action::FilesWrite
            | Action::WebhooksWrite => can_write,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Authorizer, InstanceRole, SiteRole};

    #[test]
    fn site_role_policy_matrix() {
        let cases = [
            (SiteRole::Viewer, Action::ContentRead, true),
            (SiteRole::Viewer, Action::ContentWrite, false),
            (SiteRole::Editor, Action::ContentWrite, true),
            (SiteRole::Editor, Action::FilesWrite, true),
            (SiteRole::Editor, Action::SchemaWrite, false),
            (SiteRole::Editor, Action::MembersManage, false),
            (SiteRole::Editor, Action::SiteManage, false),
            (SiteRole::Viewer, Action::MembersManage, false),
        ];

        for (role, action, expected) in cases {
            assert_eq!(Authorizer::allows_site(role, action), expected, "{role:?} {action:?}");
        }
    }

    #[test]
    fn instance_operators_override_all_site_actions() {
        for action in [
            Action::SiteManage,
            Action::SchemaWrite,
            Action::WebhooksWrite,
            Action::ApiKeysManage,
            Action::MembersManage,
            Action::ContentWrite,
        ] {
            assert!(Authorizer::allows_site_as_instance(Some(InstanceRole::InstanceOwner), action));
            assert!(Authorizer::allows_site_as_instance(Some(InstanceRole::InstanceAdmin), action));
            assert!(!Authorizer::allows_site_as_instance(None, action));
        }
    }

    #[test]
    fn only_owner_grants_instance_roles() {
        assert!(Authorizer::allows_instance(
            Some(InstanceRole::InstanceOwner),
            Action::InstanceRolesGrant
        ));
        assert!(!Authorizer::allows_instance(
            Some(InstanceRole::InstanceAdmin),
            Action::InstanceRolesGrant
        ));
        // Admins can still create and delete sites and manage users.
        assert!(Authorizer::allows_instance(Some(InstanceRole::InstanceAdmin), Action::SiteCreate));
        assert!(Authorizer::allows_instance(Some(InstanceRole::InstanceAdmin), Action::SiteDelete));
        assert!(Authorizer::allows_instance(
            Some(InstanceRole::InstanceAdmin),
            Action::InstanceManage
        ));
    }

    #[test]
    fn api_keys_never_receive_dashboard_authority() {
        assert!(Authorizer::allows_api_key(false, Action::ContentRead));
        assert!(!Authorizer::allows_api_key(false, Action::ContentWrite));
        assert!(Authorizer::allows_api_key(true, Action::SchemaWrite));
        assert!(Authorizer::allows_api_key(true, Action::WebhooksWrite));
        assert!(Authorizer::allows_api_key(true, Action::SiteManage));
        assert!(!Authorizer::allows_api_key(true, Action::ApiKeysManage));
        assert!(!Authorizer::allows_api_key(true, Action::MembersManage));
    }

    #[test]
    fn instance_owner_can_manage_instance_and_create_sites() {
        assert!(Authorizer::allows_instance(
            Some(InstanceRole::InstanceOwner),
            Action::InstanceManage
        ));
        assert!(Authorizer::allows_instance(
            Some(InstanceRole::InstanceOwner),
            Action::SiteCreate
        ));
        assert!(!Authorizer::allows_instance(None, Action::SiteCreate));
    }
}
