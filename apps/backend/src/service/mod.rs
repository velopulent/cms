//! Native service hosting and read-only service inspection.
//!
//! Registration and lifecycle mutations remain owned by OS installers/tools.

use crate::cli::{Cli, ServiceAction};

pub const SERVICE_NAME: &str = "vcms";
pub const SERVICE_DISPLAY_NAME: &str = "Velopulent CMS";

mod status;
#[cfg(windows)]
mod windows;

pub async fn run_service(action: &ServiceAction, _cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ServiceAction::Status => status::print(),
        ServiceAction::Run => {
            #[cfg(windows)]
            {
                windows::dispatch(action, _cli)
            }
            #[cfg(not(windows))]
            {
                let context = crate::runtime::RuntimeContext::initialize(crate::paths::RuntimeMode::Installed)?;
                crate::server::run(context, crate::server::shutdown_signal(), || {}).await
            }
        }
    }
}

pub fn is_installed() -> Result<bool, Box<dyn std::error::Error>> {
    status::is_installed()
}

#[cfg(windows)]
pub fn run_service_sync(action: &ServiceAction, cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    windows::dispatch(action, cli)
}
