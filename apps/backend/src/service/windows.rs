//! Windows Service Control Manager host for `vcms service run`.
//!
//! The SCM launches this hidden entry point (registered with
//! `launch_arguments = ["service", "run"]`): it hands the thread to the SCM
//! dispatcher, hosts the shared server on a fresh tokio runtime, and translates a
//! Stop/Shutdown control into the server's graceful-shutdown signal.
//!
//! Service *registration* (CreateService, dir + ACL hardening) lives in the
//! Windows installer (.msi), not here.

use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

use super::SERVICE_NAME;
use crate::cli::{Cli, ServiceAction};

/// Fixed home for the LocalSystem service (Windows convention is ProgramData).
/// The service host sets this before runtime startup so direct SCM launches always
/// use the machine-wide data directory.
fn service_home() -> std::path::PathBuf {
    crate::paths::system_home().expect("system_home is always set on Windows")
}

pub fn dispatch(action: &ServiceAction, _cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ServiceAction::Run => run_dispatcher(),
        ServiceAction::Status => unreachable!("status is dispatched by service::run_service"),
    }
}

define_windows_service!(ffi_service_main, service_main);

/// Entry point for `vcms service run`: hand the thread to the SCM dispatcher.
fn run_dispatcher() -> Result<(), Box<dyn std::error::Error>> {
    // Pin the service to the ProgramData home so it doesn't fall back to the
    // LocalSystem profile. Set before any thread/runtime reads the environment.
    if std::env::var_os(crate::paths::CMS_HOME_ENV).is_none() {
        // SAFETY: called once at process start, before tokio/threads spin up.
        unsafe {
            std::env::set_var(crate::paths::CMS_HOME_ENV, service_home());
        }
    }
    // A service has no console, so the default stdout logging goes nowhere. Default
    // to file output (lands in the home's logs/ — where install tells the user to
    // look); an explicit LOG_OUTPUT (env or .env) still wins.
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service_session() {
        // No console exists here — stderr goes nowhere — so failures land in a file.
        log_service_error(&format!("vcms service error: {e}"));
    }
}

/// Append a fatal service error to `<home>\logs\service-error.log`. Deliberately
/// plain `std::fs` (no tracing): it must work even when startup failed before —
/// or because — tracing/config initialization broke. Best-effort: a service can't
/// report its reporter failing.
fn log_service_error(msg: &str) {
    use std::io::Write;
    let _ = std::process::Command::new("eventcreate.exe")
        .args(["/T", "ERROR", "/ID", "1", "/L", "APPLICATION", "/SO", "vcms", "/D", msg])
        .status();
    let dir = service_home().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("service-error.log"))
    {
        let _ = writeln!(file, "[{}] {msg}", crate::services::backup::now_iso());
    }
}

fn run_service_session() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Mutex;
    use tokio::sync::Notify;

    // A Stop/Shutdown control fires this; the server's injected shutdown future
    // awaits it. `notify_one` stores a permit, so an early stop is not lost.
    let notify = Arc::new(Notify::new());
    let handler_notify = notify.clone();

    // The handle is created by register() but needed inside the handler closure to
    // publish StopPending. Store it behind Mutex so the closure can grab it.
    let status_handle_cell: Arc<Mutex<Option<service_control_handler::ServiceStatusHandle>>> =
        Arc::new(Mutex::new(None));
    let handle_cell = status_handle_cell.clone();

    let status_handle = service_control_handler::register(SERVICE_NAME, move |control| match control {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            if let Ok(guard) = handle_cell.lock() {
                if let Some(h) = guard.as_ref() {
                    let _ = h.set_service_status(ServiceStatus {
                        service_type: ServiceType::OWN_PROCESS,
                        current_state: ServiceState::StopPending,
                        controls_accepted: ServiceControlAccept::empty(),
                        exit_code: ServiceExitCode::Win32(0),
                        checkpoint: 1,
                        wait_hint: Duration::from_secs(30),
                        process_id: None,
                    });
                }
            }
            handler_notify.notify_one();
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })?;
    *status_handle_cell.lock().map_err(|e| format!("{e}"))? = Some(status_handle);

    // Report StartPending while the server actually boots (home preflight, secrets,
    // config, DB open + migrations, port binds). Running is published only from the
    // readiness hook below — setting Running up front would hide a boot failure.
    let start_pending = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(60),
        process_id: None,
    };
    status_handle.set_service_status(start_pending)?;

    let ready_handle = status_handle;
    let on_ready = move || {
        let _ = ready_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        });
    };

    // This thread is owned by the SCM dispatcher (not a tokio worker), so building
    // a fresh runtime here is safe — no nested-runtime panic.
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let shutdown = async move { notify.notified().await };
    let result = runtime.block_on(crate::server::run(&Cli::default(), shutdown, on_ready));

    if let Err(e) = &result {
        log_service_error(&format!("server startup/run failed: {e}"));
    }
    let stopped = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        // ServiceSpecific(1) marks "vcms itself failed" in the SCM/Event Log;
        // details are in logs\service-error.log.
        exit_code: if result.is_ok() {
            ServiceExitCode::Win32(0)
        } else {
            ServiceExitCode::ServiceSpecific(1)
        },
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    };
    status_handle.set_service_status(stopped)?;
    result
}
