use std::sync::Arc;

use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::grpc::cms::v1::membership_service_server::MembershipService;
use crate::grpc::cms::v1::{
    DeleteResponse, InviteMemberRequest, ListMembersRequest, ListMembersResponse, RemoveMemberRequest,
    SiteMember as ProtoSiteMember, UpdateMemberRoleRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::middleware::auth::{SCOPE_MEMBERS_READ, SCOPE_MEMBERS_WRITE};
use crate::models::site::SiteMember;
use crate::repository::Repository;
use crate::repository::error::RepositoryError;

#[derive(Clone)]
pub struct MembershipServiceImpl {
    repository: Arc<Repository>,
}

impl MembershipServiceImpl {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self { repository }
    }
}

#[tonic::async_trait]
impl MembershipService for MembershipServiceImpl {
    async fn list_members(&self, request: Request<ListMembersRequest>) -> Result<Response<ListMembersResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_instance_scope(SCOPE_MEMBERS_READ)?;
        let site_id = request.into_inner().site_id;

        let members = self
            .repository
            .site
            .list_members(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ListMembersResponse {
            members: members.into_iter().map(ProtoSiteMember::from).collect(),
        }))
    }

    async fn invite_member(&self, request: Request<InviteMemberRequest>) -> Result<Response<ProtoSiteMember>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_instance_scope(SCOPE_MEMBERS_WRITE)?;
        let req = request.into_inner();

        let valid_roles = ["owner", "admin", "editor", "viewer"];
        if !valid_roles.contains(&req.role.as_str()) {
            return Err(Status::invalid_argument(
                "Invalid role. Must be owner, admin, editor, or viewer",
            ));
        }

        let user_id = self
            .repository
            .user
            .find_id_by_username(&req.username)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        let member = self
            .repository
            .site
            .add_member(&Uuid::now_v7().to_string(), &req.site_id, &user_id, &req.role)
            .await
            .map_err(|e| match e {
                RepositoryError::UniqueViolation(_) => Status::already_exists("User is already a member of this site"),
                other => Status::internal(format!("Database error: {}", other)),
            })?;

        Ok(Response::new(ProtoSiteMember::from(member)))
    }

    async fn update_member_role(
        &self,
        request: Request<UpdateMemberRoleRequest>,
    ) -> Result<Response<ProtoSiteMember>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_instance_scope(SCOPE_MEMBERS_WRITE)?;
        let req = request.into_inner();

        let valid_roles = ["owner", "admin", "editor", "viewer"];
        if !valid_roles.contains(&req.role.as_str()) {
            return Err(Status::invalid_argument("Invalid role"));
        }

        let member = self
            .repository
            .site
            .update_member_role(&req.site_id, &req.user_id, &req.role)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found("Member not found"))?;

        Ok(Response::new(ProtoSiteMember::from(member)))
    }

    async fn remove_member(&self, request: Request<RemoveMemberRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_instance_scope(SCOPE_MEMBERS_WRITE)?;
        let req = request.into_inner();

        let deleted = self
            .repository
            .site
            .remove_member(&req.site_id, &req.user_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "Member removed".to_string()
            } else {
                "Member not found".to_string()
            },
        }))
    }
}

impl From<SiteMember> for ProtoSiteMember {
    fn from(member: SiteMember) -> Self {
        Self {
            id: member.id,
            site_id: member.site_id,
            user_id: member.user_id,
            username: member.username,
            email: member.email,
            role: member.role,
            created_at: member.created_at,
        }
    }
}
