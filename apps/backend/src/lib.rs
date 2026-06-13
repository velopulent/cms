#![deny(dead_code, unused_imports, unused_variables, unused_mut)]

pub mod cli;
pub mod config;
pub mod database;
pub mod error;
pub mod graphql;
pub mod mcp;
pub mod paths;
pub mod secrets;
pub mod signed_upload;
pub mod test_helpers;
pub mod tracing;
pub mod middleware {
    pub mod api_auth;
    pub mod auth;
    pub mod authz;
    pub mod dashboard_auth;
    pub mod error;
    pub mod rate_limit;
    pub mod site_resolver;
}
pub mod models {
    pub mod access_token;
    pub mod authorization;
    pub mod collection;
    pub mod entry;
    pub mod file;
    pub mod session;
    pub mod site;
    pub mod user;
    pub mod webhook;
}
pub mod handlers {
    pub mod access_token_handler;
    pub mod auth_handler;
    pub mod collection_handler;
    pub mod dashboard_handler;
    pub mod entry_handler;
    pub mod file_handler;
    pub mod instance_handler;
    pub mod singleton_handler;
    pub mod site_handler;
    pub mod webhook_handler;
}
pub mod grpc;
pub mod repository;
pub mod router;
pub mod services;
pub mod storage;
pub mod utils;
