use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Implementation, InitializeRequestParams, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult, ServerCapabilities,
    ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};

use crate::config::Config;
use crate::middleware::auth::{Actor, ScopeSet};
use crate::repository::Repository;
use crate::services::Services;
use crate::services::scope::ScopeChecker;
use crate::storage::StorageRegistry;

use crate::mcp::resources::site_schema;
use crate::mcp::tools::{collection, entry, file, singleton, site, webhook};

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

    #[tool(description = "Get details of a specific site by ID")]
    async fn get_site(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<site::GetSiteParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        site::get_site(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Update a site's name")]
    async fn update_site(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<site::UpdateSiteParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        site::update_site(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "List collections in a site")]
    async fn list_collections(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<collection::ListCollectionsParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        collection::list_collections(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Get a collection by slug")]
    async fn get_collection(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<collection::GetCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        collection::get_collection(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Create a new collection")]
    async fn create_collection(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<collection::CreateCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        collection::create_collection(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Update a collection's definition")]
    async fn update_collection(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<collection::UpdateCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        collection::update_collection(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Delete a collection")]
    async fn delete_collection(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<collection::DeleteCollectionParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        collection::delete_collection(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "List entries in a site, optionally filtered by collection and status")]
    async fn list_entries(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::ListEntriesParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        entry::list_entries(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Get an entry by ID")]
    async fn get_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::GetEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        entry::get_entry(&self.scope_checker, &self.services, &self.storage_registry, &actor, params).await
    }

    #[tool(description = "Create a new entry")]
    async fn create_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::CreateEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        entry::create_entry(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Update an entry")]
    async fn update_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::UpdateEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        entry::update_entry(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Delete an entry")]
    async fn delete_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::DeleteEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        entry::delete_entry(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Publish an entry")]
    async fn publish_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::PublishEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        entry::publish_entry(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Unpublish an entry")]
    async fn unpublish_entry(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<entry::UnpublishEntryParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        entry::unpublish_entry(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "List singletons in a site")]
    async fn list_singletons(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<singleton::ListSingletonsParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        singleton::list_singletons(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Get a singleton by slug")]
    async fn get_singleton(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<singleton::GetSingletonParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        singleton::get_singleton(&self.scope_checker, &self.services, &self.storage_registry, &actor, params).await
    }

    #[tool(description = "Update a singleton's data")]
    async fn update_singleton(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<singleton::UpdateSingletonParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        singleton::update_singleton(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "List files in a site")]
    async fn list_files(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<file::ListFilesParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        file::list_files(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Get file details by ID")]
    async fn get_file(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<file::GetFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        file::get_file(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Create a signed upload URL for uploading a file")]
    async fn create_upload_url(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<file::CreateUploadUrlParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        let public_base_url = self.public_base_url(&ctx);
        file::create_upload_url(&self.scope_checker, &self.config, &actor, public_base_url, params).await
    }

    #[tool(description = "Delete a file (soft delete)")]
    async fn delete_file(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<file::DeleteFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        file::delete_file(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "List webhooks for a site")]
    async fn list_webhooks(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<webhook::ListWebhooksParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        webhook::list_webhooks(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Create a webhook")]
    async fn create_webhook(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<webhook::CreateWebhookParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        webhook::create_webhook(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Trigger a webhook")]
    async fn trigger_webhook(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<webhook::TriggerWebhookParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        webhook::trigger_webhook(&self.scope_checker, &self.services, &actor, params).await
    }

    #[tool(description = "Delete a webhook")]
    async fn delete_webhook(
        &self,
        ctx: RequestContext<RoleServer>,
        params: Parameters<webhook::DeleteWebhookParams>,
    ) -> Result<CallToolResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        webhook::delete_webhook(&self.scope_checker, &self.services, &actor, params).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::CmsServer;
    use crate::mcp::schema::clean_input_schema;

    #[test]
    fn tool_router_lists_registered_tools() {
        let tools = CmsServer::tool_router().list_all();
        assert!(tools.iter().any(|tool| tool.name == "get_site"));
        assert!(!tools.iter().any(|tool| tool.name.contains("token")));
        assert!(!tools.iter().any(|tool| tool.name == "list_sites"));
        assert!(tools.len() > 10);
    }

    fn all_tools() -> Vec<rmcp::model::Tool> {
        CmsServer::tool_router().list_all()
    }

    #[test]
    fn no_type_null_in_schemas() {
        for tool in all_tools() {
            let cleaned = clean_input_schema(tool.input_schema);
            assert_eq!(
                cleaned.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "tool '{}' input_schema must have type 'object'",
                tool.name
            );
        }
    }

    #[test]
    fn no_boolean_properties_in_schemas() {
        for tool in all_tools() {
            let cleaned = clean_input_schema(tool.input_schema);
            if let Some(props) = cleaned.get("properties").and_then(|v| v.as_object()) {
                for (key, value) in props {
                    assert!(
                        value.is_object() || value.is_array(),
                        "tool '{}' property '{}' must be a schema object, got {:?}",
                        tool.name,
                        key,
                        value
                    );
                }
            }
        }
    }

    #[test]
    fn all_input_schemas_are_valid_objects() {
        for tool in all_tools() {
            let cleaned = clean_input_schema(tool.input_schema);
            assert!(
                cleaned.contains_key("type"),
                "tool '{}' input_schema missing 'type'",
                tool.name
            );
            assert!(
                cleaned.contains_key("properties"),
                "tool '{}' input_schema missing 'properties'",
                tool.name
            );
        }
    }

    #[test]
    fn no_type_null_anywhere() {
        for tool in all_tools() {
            let schema_str = serde_json::to_string(&*tool.input_schema).unwrap();
            if schema_str.contains(r#""type":"null""#) {
                panic!(
                    "tool '{}' still contains \"type\":\"null\"",
                    tool.name,
                );
            }
        }
    }

    #[test]
    fn no_boolean_schema_values() {
        for tool in all_tools() {
            if let Some(props) = tool.input_schema.get("properties").and_then(|v| v.as_object()) {
                for (key, value) in props {
                    if value.is_boolean() {
                        panic!(
                            "tool '{}' property '{}' is boolean {:?}",
                            tool.name, key, value
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cleaned_schemas_have_no_schema_or_title() {
        for tool in all_tools() {
            let cleaned = clean_input_schema(tool.input_schema);
            assert!(
                !cleaned.contains_key("$schema"),
                "tool '{}' still has $schema",
                tool.name
            );
            assert!(
                !cleaned.contains_key("title"),
                "tool '{}' still has title",
                tool.name
            );
        }
    }

    #[test]
    fn required_fields_are_subset_of_properties() {
        for tool in all_tools() {
            let cleaned = clean_input_schema(tool.input_schema);
            let props: HashSet<&str> = cleaned
                .get("properties")
                .and_then(|v| v.as_object())
                .map(|m| m.keys().map(|k| k.as_str()).collect())
                .unwrap_or_default();
            if let Some(required) = cleaned.get("required").and_then(|v| v.as_array()) {
                for req in required {
                    let req_str = req.as_str().unwrap_or("");
                    assert!(
                        props.contains(req_str),
                        "tool '{}' required field '{}' not in properties",
                        tool.name,
                        req_str
                    );
                }
            }
        }
    }
}

use crate::mcp::schema::clean_input_schema;

#[tool_handler]
impl ServerHandler for CmsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().enable_resources().build())
            .with_server_info(Implementation::new("cms", env!("CARGO_PKG_VERSION")))
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ServerInfo, McpError> {
        if context.peer.peer_info().is_none() {
            context.peer.set_peer_info(request.clone());
        }
        Ok(self.get_info().with_protocol_version(request.protocol_version))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = Self::tool_router()
            .list_all()
            .into_iter()
            .map(|mut tool| {
                tool.input_schema = clean_input_schema(tool.input_schema);
                tool
            })
            .collect();
        Ok(ListToolsResult {
            tools,
            meta: None,
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let tool_context = rmcp::handler::server::tool::ToolCallContext::new(self, request, ctx);
        Self::tool_router().call(tool_context).await
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        site_schema::list_resources(&self.scope_checker, &self.services, &actor, request).await
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let (actor, _scopes) = self.resolve_actor(&ctx)?;
        site_schema::read_resource(&self.scope_checker, &self.services, &actor, &request.uri).await
    }
}

impl CmsServer {
    fn resolve_actor(&self, ctx: &RequestContext<RoleServer>) -> Result<(Actor, ScopeSet), McpError> {
        crate::mcp::auth::resolve_actor(ctx)
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
