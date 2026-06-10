use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstanceRole {
    InstanceOwner,
}

impl InstanceRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstanceOwner => "instance_owner",
        }
    }
}

impl FromStr for InstanceRole {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SiteRole {
    Viewer,
    Editor,
    Admin,
    Owner,
}

impl SiteRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Editor => "editor",
            Self::Admin => "admin",
            Self::Owner => "owner",
        }
    }
}

impl FromStr for SiteRole {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "viewer" => Ok(Self::Viewer),
            "editor" => Ok(Self::Editor),
            "admin" => Ok(Self::Admin),
            "owner" => Ok(Self::Owner),
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
    AdminsManage,
    OwnershipTransfer,
}

pub struct Authorizer;

impl Authorizer {
    pub const fn allows_instance(role: Option<InstanceRole>, action: Action) -> bool {
        matches!(
            (role, action),
            (
                Some(InstanceRole::InstanceOwner),
                Action::InstanceManage | Action::SiteCreate
            )
        )
    }

    pub const fn allows_site(role: SiteRole, action: Action) -> bool {
        match action {
            Action::SiteRead
            | Action::ContentRead
            | Action::SchemaRead
            | Action::FilesRead
            | Action::WebhooksRead
            | Action::MembersRead => true,
            Action::ContentWrite | Action::FilesWrite => {
                matches!(role, SiteRole::Owner | SiteRole::Admin | SiteRole::Editor)
            }
            Action::SiteManage | Action::SchemaWrite | Action::WebhooksWrite | Action::ApiKeysManage => {
                matches!(role, SiteRole::Owner | SiteRole::Admin)
            }
            Action::MembersManage => matches!(role, SiteRole::Owner | SiteRole::Admin),
            Action::AdminsManage | Action::OwnershipTransfer | Action::SiteDelete => matches!(role, SiteRole::Owner),
            Action::InstanceManage | Action::SiteCreate => false,
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
            (SiteRole::Editor, Action::SchemaWrite, false),
            (SiteRole::Admin, Action::SchemaWrite, true),
            (SiteRole::Admin, Action::AdminsManage, false),
            (SiteRole::Owner, Action::AdminsManage, true),
            (SiteRole::Owner, Action::SiteDelete, true),
        ];

        for (role, action, expected) in cases {
            assert_eq!(Authorizer::allows_site(role, action), expected, "{role:?} {action:?}");
        }
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
