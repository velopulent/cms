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
            "Service '{SERVICE_NAME}' was installed but did not reach Running.{} Full logs are under \
             {}\\logs (read them from an elevated terminal).",
            recent_service_errors(),
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
    let service = open_service(ServiceAccess::START | ServiceAccess::QUERY_STATUS)?;
    service.start::<&str>(&[])?;
    // Wait for Running before claiming success (same rationale as install()).
    if !wait_for_state(&service, ServiceState::Running, SCM_WAIT) {
        return Err(format!(
            "Service '{SERVICE_NAME}' was told to start but did not reach Running.{} Full logs are under \
             {}\\logs (read them from an elevated terminal).",
            recent_service_errors(),
            service_home().display()
        )
        .into());
    }
    println!("Service '{SERVICE_NAME}' started.");
    Ok(())
}

fn stop() -> Result<(), Box<dyn std::error::Error>> {
    let service = open_service(ServiceAccess::STOP | ServiceAccess::QUERY_STATUS)?;
    // Unlike stop_and_wait (which tolerates already-stopped during teardown), an
    // explicit user stop should surface the SCM error.
    service.stop()?;
    if wait_for_state(&service, ServiceState::Stopped, SCM_WAIT) {
        println!("Service '{SERVICE_NAME}' stopped.");
    } else {
        println!("Service '{SERVICE_NAME}' stop requested; still stopping after 30s — check `vcms service status`.");
    }
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
    harden_acl_for(path, &["*S-1-5-18:(OI)(CI)F", "*S-1-5-32-544:(OI)(CI)F"])
}

/// The two-pass icacls sequence behind [`harden_acl`], parameterized over the grant
/// specs so a non-elevated unit test can exercise it with the current user.
///
/// Pass 1 hardens the **root directory only**: strip inherited ACEs, drop broad
/// explicit ACEs (Everyone `S-1-1-0`, Users `S-1-5-32-545`, Authenticated Users
/// `S-1-5-11`), grant the given principals as inheritable ACEs. Pass 2 `/reset`s
/// every child so it re-inherits from the root.
///
/// Never apply pass 1's inheritance-flagged grants to files (the old code's `/T`):
/// icacls silently *drops* an `(OI)(CI)` grant on a file while `/inheritance:r`
/// still strips its existing ACEs, leaving an **empty DACL — deny everyone,
/// including SYSTEM**. That bricked every pre-existing file (db, secrets.toml) on
/// reinstall, which is why the service booted fresh but died with "Access is
/// denied (os error 5)" after an uninstall/install cycle. Pass 2 also *repairs*
/// trees damaged that way.
fn harden_acl_for(path: &std::path::Path, grant_specs: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let mut root = std::process::Command::new("icacls");
    root.arg(path)
        .arg("/inheritance:r")
        .args(["/remove:g", "*S-1-1-0", "*S-1-5-32-545", "*S-1-5-11"]);
    for spec in grant_specs {
        root.args(["/grant:r", spec]);
    }
    let status = root.status()?;
    if !status.success() {
        return Err(format!("icacls failed to harden {} ({status})", path.display()).into());
    }

    // `<path>\*` matches the immediate children; `/T` recurses below them. An empty
    // dir makes icacls fail with "file not found" — fine, nothing to reset.
    let mut children = std::ffi::OsString::from(path);
    children.push(r"\*");
    let reset = std::process::Command::new("icacls")
        .arg(children)
        .args(["/reset", "/T", "/Q"])
        .status()?;
    if !reset.success() && path.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Err(format!(
            "icacls failed to reset file ACLs under {} ({reset}); if files there were left \
             inaccessible by an older vcms install, run `takeown /f {} /r /d y` once and re-run \
             `vcms service install`",
            path.display(),
            path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DOMAIN\user:(OI)(CI)F` for the current account — lets the tests run the real
    /// icacls sequence on a user-owned temp dir without elevation.
    fn current_user_grant() -> String {
        let domain = std::env::var("USERDOMAIN").unwrap_or_default();
        let user = std::env::var("USERNAME").expect("USERNAME is always set on Windows");
        if domain.is_empty() {
            format!("{user}:(OI)(CI)F")
        } else {
            format!("{domain}\\{user}:(OI)(CI)F")
        }
    }

    /// Regression: the old single-pass `icacls ... /grant:r X:(OI)(CI)F /T` left every
    /// *pre-existing file* with an empty DACL (deny-everyone) because icacls drops
    /// inheritance-flagged grants on files. The two-pass sequence must keep such files
    /// readable — this is the exact bug that made the Windows service fail to start
    /// after an uninstall/reinstall cycle.
    #[test]
    fn harden_keeps_existing_files_readable() {
        let dir = tempfile::tempdir().expect("temp dir");
        let pre_existing = dir.path().join("secrets.toml");
        std::fs::write(&pre_existing, "hmac_secret = \"x\"").expect("write");

        let grant = current_user_grant();
        harden_acl_for(dir.path(), &[grant.as_str()]).expect("harden");

        let read = std::fs::read_to_string(&pre_existing).expect("pre-existing file must stay readable");
        assert_eq!(read, "hmac_secret = \"x\"");

        // Files created after hardening inherit from the root and stay readable too.
        let created_after = dir.path().join("vcms.db");
        std::fs::write(&created_after, "db").expect("write after harden");
        assert_eq!(std::fs::read_to_string(&created_after).expect("readable"), "db");
    }

    /// The children `/reset` pass must also *repair* a tree bricked by the old
    /// sequence (files left with an empty DACL by a previous vcms install).
    #[test]
    fn harden_repairs_files_bricked_by_old_sequence() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("vcms.db");
        std::fs::write(&file, "data").expect("write");

        // Reproduce the old bug: inheritance-flagged grant with /T strips the file's
        // ACEs without adding effective ones.
        let grant = current_user_grant();
        let status = std::process::Command::new("icacls")
            .arg(dir.path())
            .arg("/inheritance:r")
            .args(["/grant:r", &grant])
            .arg("/T")
            .status()
            .expect("icacls runs");
        assert!(status.success());
        assert!(
            std::fs::read_to_string(&file).is_err(),
            "old sequence should brick the file (if this fails, icacls semantics changed)"
        );

        harden_acl_for(dir.path(), &[grant.as_str()]).expect("harden");
        assert_eq!(std::fs::read_to_string(&file).expect("repaired and readable"), "data");
    }
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
    // A service has no console, so the default stdout logging goes nowhere. Default
    // to file output (lands in the home's logs/ — where install() tells the user to
    // look); an explicit LOG_OUTPUT (env or .env) still wins.
    if std::env::var_os("LOG_OUTPUT").is_none() {
        // SAFETY: same single-threaded process-start window as above.
        unsafe {
            std::env::set_var("LOG_OUTPUT", "file");
        }
    }
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service_session() {
        // No console exists here — stderr goes nowhere — so failures land in a file.
        log_service_error(&format!("vcms service error: {e}"));
    }
}

/// Tail of `service-error.log`, pre-formatted for appending to a start-failure
/// message. The installer runs elevated (it can read the ACL-locked home), so it
/// surfaces the service's own boot error directly instead of making the user go
/// find it. Empty string when the file is absent/unreadable.
fn recent_service_errors() -> String {
    let path = service_home().join("logs").join("service-error.log");
    let Ok(contents) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let mut tail: Vec<&str> = contents.lines().rev().take(5).collect();
    tail.reverse();
    if tail.is_empty() {
        return String::new();
    }
    format!(" Recent service errors:\n  {}\n", tail.join("\n  "))
}

/// Append a fatal service error to `<home>\logs\service-error.log`. Deliberately
/// plain `std::fs` (no tracing): it must work even when startup failed before —
/// or because — tracing/config initialization broke. Best-effort: a service can't
/// report its reporter failing.
fn log_service_error(msg: &str) {
    use std::io::Write;
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

    // Report StartPending while the server actually boots (home preflight, secrets,
    // config, DB open + migrations, port binds). Running is published only from the
    // readiness hook below — the old code set Running up front, so install() saw a
    // healthy service even when boot failed a moment later.
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
        // ServiceSpecific(1) (instead of a generic Win32 code) marks "vcms itself
        // failed" in the SCM/Event Log; details are in logs\service-error.log.
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
