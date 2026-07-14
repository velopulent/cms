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
        #[cfg(windows)]
        ServiceAction::Run => windows::dispatch(action, _cli),
    }
}
