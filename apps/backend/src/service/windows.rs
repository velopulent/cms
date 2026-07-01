//! Windows Service Control Manager integration for `vcms service`.
//!
//! `install`/`uninstall`/`status`/`start`/`stop` drive the SCM via the
//! `windows-service` crate. `Run` is the hidden entry point the SCM launches
//! (registered with `launch_arguments = ["service", "run"]`): it hands the thread
//! to the SCM dispatcher, hosts the shared server on a fresh tokio runtime, and
//! translates a Stop/Shutdown control into the server's graceful-shutdown signal.

use std::ffi::OsString;
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows_service::service::{
    Service, ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode, ServiceInfo,
    ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_service::{define_windows_service, service_dispatcher};

use super::{ENV_TEMPLATE, SERVICE_DISPLAY_NAME, SERVICE_NAME};
use crate::cli::{Cli, ServiceAction};

/// Fixed home for the LocalSystem service (Windows convention is ProgramData).
/// Defined once in `paths::system_home()` so a plain CLI invocation resolves to the
/// same store; always `Some` on Windows.
fn service_home() -> std::path::PathBuf {
    crate::paths::system_home().expect("system_home is always set on Windows")
}

/// `ERROR_SERVICE_DOES_NOT_EXIST` — returned by `OpenServiceW` once the SCM has actually
/// removed a record (deletion is deferred until every handle closes and the host exits).
const ERROR_SERVICE_DOES_NOT_EXIST: i32 = 1060;

/// Upper bound on any SCM state transition we wait for (stop, delete-finalize, start).
const SCM_WAIT: Duration = Duration::from_secs(30);
/// Poll interval while waiting on the SCM.
const SCM_POLL: Duration = Duration::from_millis(200);

pub fn dispatch(action: &ServiceAction, _cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ServiceAction::Install { user } => {
            if user.as_deref().is_some_and(|u| !u.is_empty()) {
                return Err(
                    "custom user accounts are not supported on Windows; the service runs as LocalSystem".into(),
                );
            }
            install()
        }
        ServiceAction::Uninstall => uninstall(),
        ServiceAction::Status => status(),
        ServiceAction::Start => start(),
        ServiceAction::Stop => stop(),
        ServiceAction::Run => run_dispatcher(),
    }
}

fn install() -> Result<(), Box<dyn std::error::Error>> {
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(elevation_hint)?;

    let exe_path = std::env::current_exe()?;
    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe_path,
        launch_arguments: vec![OsString::from("service"), OsString::from("run")],
        dependencies: vec![],
        account_name: None, // LocalSystem
        account_password: None,
    };

    // Start from a clean slate: tear down any lingering record first so we always
    // `create_service` fresh. Reusing an existing entry is the reinstall trap — after an
    // uninstall the old record can still be present (Windows defers deletion), and a
    // reused/DELETE_PENDING service ends up Stopped instead of running the new config.
    if let Ok(existing) = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        stop_and_wait(&existing);
        let _ = existing.delete(); // marks for deletion; finalizes once handles close
        drop(existing); // close our handle so the SCM can finish removing the record
        if !wait_until_deleted(&manager, SCM_WAIT) {
            return Err("a previous 'vcms' service is still being removed by Windows; \
                        wait a few seconds and re-run `vcms service install`"
                .into());
        }
    }

    let access = ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::QUERY_STATUS;
    let service = manager.create_service(&info, access).map_err(elevation_hint)?;
    let _ = service.set_description(SERVICE_DISPLAY_NAME);

    prepare_home()?;

    service.start::<&str>(&[]).map_err(elevation_hint)?;
    // Verify the service actually reached Running before claiming success — the old code
    // printed "started" unconditionally, which is how a crash-on-boot slipped through.
    if !wait_for_state(&service, ServiceState::Running, SCM_WAIT) {
        return Err(format!(
            "Service '{SERVICE_NAME}' was installed but did not reach Running. Check the logs under \
             {}\\logs (read them from an elevated terminal). A common cause is another process already \
             bound to the REST/gRPC ports.",
            service_home().display()
        )
        .into());
    }
    println!("Service '{SERVICE_NAME}' installed (auto-start, LocalSystem) and started.");
    println!("Home + secrets (.env): {}", service_home().display());
    println!("Check it with:  vcms service status");
    Ok(())
}

fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).map_err(elevation_hint)?;
    let service = match manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    ) {
        Ok(service) => service,
        Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST) => {
            println!("Service '{SERVICE_NAME}' is not installed.");
            return Ok(());
        }
        Err(e) => return Err(elevation_hint(e)),
    };

    stop_and_wait(&service);
    service.delete()?;
    drop(service); // close our handle so the SCM can finalize removal

    if wait_until_deleted(&manager, SCM_WAIT) {
        println!(
            "Service '{SERVICE_NAME}' uninstalled. Your data under {} was left intact.",
            service_home().display()
        );
    } else {
        // Deletion is queued but a handle elsewhere (e.g. an open Services.msc) is holding
        // it; it will vanish once that closes. Report honestly instead of implying it's gone.
        println!(
            "Service '{SERVICE_NAME}' stop + delete requested; Windows will finish removing it once all \
             handles close. Your data under {} was left intact.",
            service_home().display()
        );
    }
    Ok(())
}

/// Request a stop and wait until the service reports `Stopped` (bounded). Tolerates a
/// service that is already stopped or marked for deletion — both surface as `stop()`
/// errors we intentionally ignore.
fn stop_and_wait(service: &Service) {
    let _ = service.stop();
    wait_for_state(service, ServiceState::Stopped, SCM_WAIT);
}

/// Poll `query_status` until the service reaches `target` or the timeout elapses.
/// Returns whether the target state was observed.
fn wait_for_state(service: &Service, target: ServiceState, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if matches!(service.query_status(), Ok(status) if status.current_state == target) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(SCM_POLL);
    }
}

/// Poll until opening the service fails with `ERROR_SERVICE_DOES_NOT_EXIST` — i.e. the
/// SCM has actually removed the record — or the timeout elapses. Each probe drops its
/// handle immediately so it never itself keeps the record alive.
fn wait_until_deleted(manager: &ServiceManager, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
            Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST) => {
                return true;
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(SCM_POLL);
    }
}

fn start() -> Result<(), Box<dyn std::error::Error>> {
    let service = open_service(ServiceAccess::START)?;
    service.start::<&str>(&[])?;
    println!("Service '{SERVICE_NAME}' started.");
    Ok(())
}

fn stop() -> Result<(), Box<dyn std::error::Error>> {
    let service = open_service(ServiceAccess::STOP)?;
    service.stop()?;
    println!("Service '{SERVICE_NAME}' stop requested.");
    Ok(())
}

fn status() -> Result<(), Box<dyn std::error::Error>> {
    let service = open_service(ServiceAccess::QUERY_STATUS)?;
    let status = service.query_status()?;
    println!("Service: {SERVICE_NAME}");
    println!("  state: {:?}", status.current_state);
    Ok(())
}

fn open_service(access: ServiceAccess) -> Result<windows_service::service::Service, Box<dyn std::error::Error>> {
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).map_err(elevation_hint)?;
    manager.open_service(SERVICE_NAME, access).map_err(|e| -> Box<dyn std::error::Error> {
        format!("could not open service '{SERVICE_NAME}' ({e}). Is it installed, and is this terminal running as Administrator?").into()
    })
}

fn elevation_hint(e: windows_service::Error) -> Box<dyn std::error::Error> {
    format!("{e}. This action requires Administrator — run your terminal as Administrator.").into()
}

/// Create the service home + optional `.env` under ProgramData, locked to
/// SYSTEM and Administrators only.
fn prepare_home() -> Result<(), Box<dyn std::error::Error>> {
    let home = service_home();
    std::fs::create_dir_all(&home)?;
    let env_file = home.join(".env");
    if !env_file.exists() {
        std::fs::write(&env_file, ENV_TEMPLATE)?;
    }
    // Harden after the tree exists; `/T` covers `home` and the `.env` we just wrote.
    harden_acl(&home)?;
    Ok(())
}

/// Lock a directory tree to LocalSystem + Administrators (Full), dropping inherited
/// ACEs so the `.env` (which may hold `DATABASE_URL`/S3 secrets) is not world-readable.
///
/// Shells out to `icacls` — same "use the native tool" pattern as the unix `chown`
/// path — rather than hand-rolling Win32 security FFI. Well-known SIDs are used so it
/// works regardless of the OS display language (`S-1-5-18` = LocalSystem,
/// `S-1-5-32-544` = Administrators).
fn harden_acl(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = std::process::Command::new("icacls")
        .arg(path)
        .arg("/inheritance:r")
        // Drop explicit broad ACEs a pre-existing dir may carry (`/inheritance:r` only
        // removes *inherited* ones): Everyone, Users, Authenticated Users.
        .args(["/remove:g", "*S-1-1-0", "*S-1-5-32-545", "*S-1-5-11"])
        .args(["/grant:r", "*S-1-5-18:(OI)(CI)F"])
        .args(["/grant:r", "*S-1-5-32-544:(OI)(CI)F"])
        .arg("/T")
        .status()?;
    if !status.success() {
        return Err(format!("icacls failed to harden {} ({status})", path.display()).into());
    }
    Ok(())
}

// ----- SCM-hosted run path -----

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
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service_session() {
        eprintln!("vcms service error: {e}");
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
            if let Ok(guard) = handle_cell.lock()
                && let Some(h) = guard.as_ref()
            {
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
            handler_notify.notify_one();
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })?;
    *status_handle_cell.lock().map_err(|e| format!("{e}"))? = Some(status_handle);

    let running = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    };
    status_handle.set_service_status(running.clone())?;

    // This thread is owned by the SCM dispatcher (not a tokio worker), so building
    // a fresh runtime here is safe — no nested-runtime panic.
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let shutdown = async move { notify.notified().await };
    let result = runtime.block_on(crate::server::run(&Cli::default(), shutdown));

    let stopped = ServiceStatus {
        current_state: ServiceState::Stopped,
        exit_code: if result.is_ok() {
            ServiceExitCode::Win32(0)
        } else {
            ServiceExitCode::Win32(1)
        },
        ..running
    };
    status_handle.set_service_status(stopped)?;
    result
}
