use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    ServerInfo, ServerCapabilities, ReadResourceResult,
    ListResourcesResult, CallToolResult, PaginatedRequestParams,
    ReadResourceRequestParams, Implementation,
};
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};


use crate::config::Config;
use crate::middleware::auth::Principal;
use crate::repository::Repository;
use crate::services::Services;
use crate::services::scope::ScopeChecker;
use crate::storage::StorageRegistry;

use crate::mcp::tools::{
    site, member, collection, entry, singleton, file, webhook, token,
};
use crate::mcp::resources::site_schema;

#[derive(Clone)]
pub struct CmsServer {
    pub services: Arc<Services>,
    pub repository: Arc<Repository>,
    pub storage_registry: Arc<StorageRegistry>,
    pub config: Arc<Config>,
    pub scope_checker: Arc<ScopeChecker>,
}

#[tool_router]
impl CmsServer {
    pub fn new(
        services: Arc<Services>,
        repository: Arc<Repository>,
        storage_registry: Arc<StorageRegistry>,
        config: Arc<Config>,
    ) -> Self {
        let scope_checker = Arc::new(ScopeChecker::new(repository.user.clone()));
        Self {
            services,
            repository,
            storage_registry,
            config,
            scope_checker,
        }
    }

    pub async fn principal_from_env(&self) -> Result<Principal, McpError> {
        crate::mcp::auth::resolve_principal_from_env(&self.config, &self.repository).await
    }

    #[tool(description = "List all sites accessible to the authenticated user")]
    async fn list_sites(
        &self,
        params: Parameters<site::ListSitesParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        site::list_sites(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Get details of a specific site by ID")]
    async fn get_site(
        &self,
        params: Parameters<site::GetSiteParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        site::get_site(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create a new site")]
    async fn create_site(
        &self,
        params: Parameters<site::CreateSiteParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        site::create_site(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Update a site's name")]
    async fn update_site(
        &self,
        params: Parameters<site::UpdateSiteParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        site::update_site(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete a site")]
    async fn delete_site(
        &self,
        params: Parameters<site::DeleteSiteParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        site::delete_site(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List members of a site")]
    async fn list_members(
        &self,
        params: Parameters<member::ListMembersParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        member::list_members(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Invite a member to a site")]
    async fn invite_member(
        &self,
        params: Parameters<member::InviteMemberParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        member::invite_member(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Remove a member from a site")]
    async fn remove_member(
        &self,
        params: Parameters<member::RemoveMemberParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        member::remove_member(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List collections in a site")]
    async fn list_collections(
        &self,
        params: Parameters<collection::ListCollectionsParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        collection::list_collections(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Get a collection by slug")]
    async fn get_collection(
        &self,
        params: Parameters<collection::GetCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        collection::get_collection(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create a new collection")]
    async fn create_collection(
        &self,
        params: Parameters<collection::CreateCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        collection::create_collection(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Update a collection's definition")]
    async fn update_collection(
        &self,
        params: Parameters<collection::UpdateCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        collection::update_collection(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete a collection")]
    async fn delete_collection(
        &self,
        params: Parameters<collection::DeleteCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        collection::delete_collection(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List entries in a site, optionally filtered by collection and status")]
    async fn list_entries(
        &self,
        params: Parameters<entry::ListEntriesParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        entry::list_entries(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Get an entry by ID")]
    async fn get_entry(
        &self,
        params: Parameters<entry::GetEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        entry::get_entry(&self.scope_checker, &self.services, &self.storage_registry, &principal, params).await
    }

    #[tool(description = "Create a new entry")]
    async fn create_entry(
        &self,
        params: Parameters<entry::CreateEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        entry::create_entry(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Update an entry")]
    async fn update_entry(
        &self,
        params: Parameters<entry::UpdateEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        entry::update_entry(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete an entry")]
    async fn delete_entry(
        &self,
        params: Parameters<entry::DeleteEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        entry::delete_entry(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Publish an entry")]
    async fn publish_entry(
        &self,
        params: Parameters<entry::PublishEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        entry::publish_entry(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Unpublish an entry")]
    async fn unpublish_entry(
        &self,
        params: Parameters<entry::UnpublishEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        entry::unpublish_entry(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List singletons in a site")]
    async fn list_singletons(
        &self,
        params: Parameters<singleton::ListSingletonsParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        singleton::list_singletons(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Get a singleton by slug")]
    async fn get_singleton(
        &self,
        params: Parameters<singleton::GetSingletonParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        singleton::get_singleton(&self.scope_checker, &self.services, &self.storage_registry, &principal, params).await
    }

    #[tool(description = "Update a singleton's data")]
    async fn update_singleton(
        &self,
        params: Parameters<singleton::UpdateSingletonParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        singleton::update_singleton(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List files in a site")]
    async fn list_files(
        &self,
        params: Parameters<file::ListFilesParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        file::list_files(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Get file details by ID")]
    async fn get_file(
        &self,
        params: Parameters<file::GetFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        file::get_file(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create a signed upload URL for uploading a file")]
    async fn create_upload_url(
        &self,
        params: Parameters<file::CreateUploadUrlParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        file::create_upload_url(&self.scope_checker, &self.config, &principal, params).await
    }

    #[tool(description = "Delete a file (soft delete)")]
    async fn delete_file(
        &self,
        params: Parameters<file::DeleteFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        file::delete_file(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List webhooks for a site")]
    async fn list_webhooks(
        &self,
        params: Parameters<webhook::ListWebhooksParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        webhook::list_webhooks(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create a webhook")]
    async fn create_webhook(
        &self,
        params: Parameters<webhook::CreateWebhookParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        webhook::create_webhook(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Trigger a webhook")]
    async fn trigger_webhook(
        &self,
        params: Parameters<webhook::TriggerWebhookParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        webhook::trigger_webhook(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete a webhook")]
    async fn delete_webhook(
        &self,
        params: Parameters<webhook::DeleteWebhookParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        webhook::delete_webhook(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List instance-level access tokens")]
    async fn list_instance_tokens(
        &self,
        params: Parameters<token::ListInstanceTokensParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        token::list_instance_tokens(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create an instance-level access token")]
    async fn create_instance_token(
        &self,
        params: Parameters<token::CreateInstanceTokenParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        token::create_instance_token(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete an instance-level access token")]
    async fn delete_instance_token(
        &self,
        params: Parameters<token::DeleteInstanceTokenParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        token::delete_instance_token(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List site-level access tokens")]
    async fn list_site_tokens(
        &self,
        params: Parameters<token::ListSiteTokensParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        token::list_site_tokens(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create a site-level access token")]
    async fn create_site_token(
        &self,
        params: Parameters<token::CreateSiteTokenParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        token::create_site_token(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete a site-level access token")]
    async fn delete_site_token(
        &self,
        params: Parameters<token::DeleteSiteTokenParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal().await?;
        token::delete_site_token(&self.scope_checker, &self.services, &principal, params).await
    }
}

#[tool_handler]
impl ServerHandler for CmsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new("cms", env!("CARGO_PKG_VERSION")))
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let principal = self.resolve_principal().await?;
        site_schema::list_resources(&self.scope_checker, &self.services, &principal, request)
            .await
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let principal = self.resolve_principal().await?;
        site_schema::read_resource(&self.scope_checker, &self.services, &principal, &request.uri)
            .await
    }
}

impl CmsServer {
    async fn resolve_principal(&self) -> Result<Principal, McpError> {
        self.principal_from_env().await
    }
}