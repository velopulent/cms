use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::membership_service_server::MembershipService;
use crate::grpc::cms::v1::{
    DeleteResponse, InviteMemberRequest, ListMembersRequest, ListMembersResponse, RemoveMemberRequest,
    SiteMember as ProtoSiteMember, UpdateMemberRoleRequest,
};
use crate::grpc::interceptor::get_auth_context;
use crate::middleware::auth::{SCOPE_MEMBERS_READ, SCOPE_MEMBERS_WRITE};
use crate::models::site::SiteMember;
use crate::repository::Repository;
use crate::services::site::SiteService;

#[derive(Clone)]
pub struct MembershipServiceImpl {
    app_site_service: Arc<SiteService>,
    repository: Arc<Repository>,
}

impl MembershipServiceImpl {
    pub fn new(site_service: Arc<SiteService>, repository: Arc<Repository>) -> Self {
        Self {
            app_site_service: site_service,
            repository,
        }
    }
}

#[tonic::async_trait]
impl MembershipService for MembershipServiceImpl {
    async fn list_members(
        &self,
        mut request: Request<ListMembersRequest>,
    ) -> Result<Response<ListMembersResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_instance_scope(SCOPE_MEMBERS_READ)?;
        let site_id = request.into_inner().site_id;

        let members = self
            .app_site_service
            .list_members(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(ListMembersResponse {
            members: members.into_iter().map(ProtoSiteMember::from).collect(),
        }))
    }

    async fn invite_member(&self, mut request: Request<InviteMemberRequest>) -> Result<Response<ProtoSiteMember>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_instance_scope(SCOPE_MEMBERS_WRITE)?;
        let req = request.into_inner();

        let member = self
            .app_site_service
            .invite_member(&req.site_id, &req.username, &req.role)
            .await
            .map_err(|e| match e {
                crate::services::site::SiteError::UserNotFound => Status::not_found("User not found"),
                crate::services::site::SiteError::AlreadyMember => {
                    Status::already_exists("User is already a member of this site")
                }
                crate::services::site::SiteError::InvalidRole(msg) => Status::invalid_argument(msg),
                _ => Status::internal(format!("Error: {}", e)),
            })?;

        Ok(Response::new(ProtoSiteMember::from(member)))
    }

    async fn update_member_role(
        &self,
        mut request: Request<UpdateMemberRoleRequest>,
    ) -> Result<Response<ProtoSiteMember>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_instance_scope(SCOPE_MEMBERS_WRITE)?;
        let req = request.into_inner();

        let member = self
            .app_site_service
            .update_member_role(&req.site_id, &req.user_id, &req.role)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?
            .ok_or_else(|| Status::not_found("Member not found"))?;

        Ok(Response::new(ProtoSiteMember::from(member)))
    }

    async fn remove_member(&self, mut request: Request<RemoveMemberRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&mut request, &self.repository).await?;
        auth.require_instance_scope(SCOPE_MEMBERS_WRITE)?;
        let req = request.into_inner();

        let deleted = self
            .app_site_service
            .remove_member(&req.site_id, &req.user_id, "grpc-call")
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

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
