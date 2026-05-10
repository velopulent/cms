use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Implementation, ListResourcesResult, PaginatedRequestParams, ReadResourceRequestParams,
    ReadResourceResult, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};

use crate::config::Config;
use crate::repository::Repository;
use crate::services::Services;
use crate::services::scope::ScopeChecker;
use crate::storage::StorageRegistry;

use crate::mcp::resources::site_schema;
use crate::mcp::tools::{collection, entry, file, member, singleton, site, token, webhook};

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

    #[tool(description = "List all sites accessible to the authenticated user")]
    async fn list_sites(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<site::ListSitesParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        site::list_sites(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Get details of a specific site by ID")]
    async fn get_site(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<site::GetSiteParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        site::get_site(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create a new site")]
    async fn create_site(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<site::CreateSiteParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        site::create_site(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Update a site's name")]
    async fn update_site(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<site::UpdateSiteParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        site::update_site(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete a site")]
    async fn delete_site(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<site::DeleteSiteParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        site::delete_site(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List members of a site")]
    async fn list_members(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<member::ListMembersParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        member::list_members(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Invite a member to a site")]
    async fn invite_member(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<member::InviteMemberParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        member::invite_member(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Remove a member from a site")]
    async fn remove_member(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<member::RemoveMemberParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        member::remove_member(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List collections in a site")]
    async fn list_collections(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<collection::ListCollectionsParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        collection::list_collections(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Get a collection by slug")]
    async fn get_collection(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<collection::GetCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        collection::get_collection(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create a new collection")]
    async fn create_collection(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<collection::CreateCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        collection::create_collection(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Update a collection's definition")]
    async fn update_collection(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<collection::UpdateCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        collection::update_collection(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete a collection")]
    async fn delete_collection(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<collection::DeleteCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        collection::delete_collection(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List entries in a site, optionally filtered by collection and status")]
    async fn list_entries(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::ListEntriesParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        entry::list_entries(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Get an entry by ID")]
    async fn get_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::GetEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        entry::get_entry(
            &self.scope_checker,
            &self.services,
            &self.storage_registry,
            &principal,
            params,
        )
        .await
    }

    #[tool(description = "Create a new entry")]
    async fn create_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::CreateEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        entry::create_entry(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Update an entry")]
    async fn update_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::UpdateEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        entry::update_entry(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete an entry")]
    async fn delete_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::DeleteEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        entry::delete_entry(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Publish an entry")]
    async fn publish_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::PublishEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        entry::publish_entry(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Unpublish an entry")]
    async fn unpublish_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::UnpublishEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        entry::unpublish_entry(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List singletons in a site")]
    async fn list_singletons(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<singleton::ListSingletonsParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        singleton::list_singletons(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Get a singleton by slug")]
    async fn get_singleton(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<singleton::GetSingletonParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        singleton::get_singleton(
            &self.scope_checker,
            &self.services,
            &self.storage_registry,
            &principal,
            params,
        )
        .await
    }

    #[tool(description = "Update a singleton's data")]
    async fn update_singleton(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<singleton::UpdateSingletonParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        singleton::update_singleton(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List files in a site")]
    async fn list_files(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<file::ListFilesParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        file::list_files(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Get file details by ID")]
    async fn get_file(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<file::GetFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        file::get_file(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create a signed upload URL for uploading a file")]
    async fn create_upload_url(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<file::CreateUploadUrlParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        let public_base_url = self.public_base_url(&ctx);
        file::create_upload_url(&self.scope_checker, &self.config, &principal, public_base_url, params).await
    }

    #[tool(description = "Delete a file (soft delete)")]
    async fn delete_file(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<file::DeleteFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        file::delete_file(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List webhooks for a site")]
    async fn list_webhooks(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<webhook::ListWebhooksParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        webhook::list_webhooks(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create a webhook")]
    async fn create_webhook(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<webhook::CreateWebhookParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        webhook::create_webhook(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Trigger a webhook")]
    async fn trigger_webhook(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<webhook::TriggerWebhookParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        webhook::trigger_webhook(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete a webhook")]
    async fn delete_webhook(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<webhook::DeleteWebhookParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        webhook::delete_webhook(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List instance-level access tokens")]
    async fn list_instance_tokens(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<token::ListInstanceTokensParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        token::list_instance_tokens(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create an instance-level access token")]
    async fn create_instance_token(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<token::CreateInstanceTokenParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        token::create_instance_token(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete an instance-level access token")]
    async fn delete_instance_token(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<token::DeleteInstanceTokenParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        token::delete_instance_token(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "List site-level access tokens")]
    async fn list_site_tokens(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<token::ListSiteTokensParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        token::list_site_tokens(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Create a site-level access token")]
    async fn create_site_token(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<token::CreateSiteTokenParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        token::create_site_token(&self.scope_checker, &self.services, &principal, params).await
    }

    #[tool(description = "Delete a site-level access token")]
    async fn delete_site_token(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<token::DeleteSiteTokenParams>,
    ) -> Result<CallToolResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        token::delete_site_token(&self.scope_checker, &self.services, &principal, params).await
    }
}

#[tool_handler]
impl ServerHandler for CmsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().enable_resources().build())
            .with_server_info(Implementation::new("cms", env!("CARGO_PKG_VERSION")))
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        site_schema::list_resources(&self.scope_checker, &self.services, &principal, request).await
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let principal = self.resolve_principal(&ctx)?;
        site_schema::read_resource(&self.scope_checker, &self.services, &principal, &request.uri).await
    }
}

impl CmsServer {
    fn resolve_principal(
        &self,
        ctx: &RequestContext<RoleServer>,
    ) -> Result<crate::middleware::auth::Principal, McpError> {
        crate::mcp::auth::resolve_principal(ctx)
    }

    fn public_base_url(&self, ctx: &RequestContext<RoleServer>) -> Option<String> {
        if let Some(public_url) = &self.config.public_url {
            return Some(public_url.clone());
        }

        let parts = ctx.extensions.get::<http::request::Parts>()?;
        let headers = &parts.headers;
        let host = headers
            .get("x-forwarded-host")
            .or_else(|| headers.get("host"))?
            .to_str()
            .ok()?;
        let proto = headers
            .get("x-forwarded-proto")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("http");

        Some(format!("{}://{}", proto, host).trim_end_matches('/').to_string())
    }
}
