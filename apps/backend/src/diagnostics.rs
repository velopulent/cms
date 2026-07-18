use std::net::SocketAddr;

use crate::database::connect_db_without_migrations;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut failed = false;
    let context = crate::runtime::RuntimeContext::initialize_default()?;
    let config = &context.bootstrap;

    report("mode", true, context.mode.to_string(), &mut failed);
    report(
        "root",
        context.paths.root().is_dir(),
        context.paths.root().display().to_string(),
        &mut failed,
    );
    report(
        "config",
        context.paths.config_file().is_file(),
        context.paths.config_file().display().to_string(),
        &mut failed,
    );
    report(
        "secrets",
        context.paths.secrets_file().is_file(),
        "present; values redacted".to_owned(),
        &mut failed,
    );
    report(
        "root-permissions",
        crate::paths::permissions_secure(context.paths.root(), 0o700),
        "private root expected".to_owned(),
        &mut failed,
    );
    report(
        "secret-permissions",
        crate::paths::permissions_secure(&context.paths.secrets_file(), 0o600),
        "private secrets expected".to_owned(),
        &mut failed,
    );
    for (name, path) in [
        ("storage-dir", context.paths.storage_dir()),
        ("backup-dir", context.paths.backups_dir()),
        ("log-dir", context.paths.logs_dir()),
        ("search-dir", context.paths.search_dir()),
    ] {
        report(name, path.is_dir(), path.display().to_string(), &mut failed);
    }

    match connect_db_without_migrations(config).await {
        Ok(pool) => report(
            "database",
            pool.ping().await.is_ok(),
            "reachable".to_owned(),
            &mut failed,
        ),
        Err(_) if config.database_url.starts_with("sqlite:") && !context.paths.database_file().exists() => report(
            "database",
            true,
            "not initialized; first serve will create it".to_owned(),
            &mut failed,
        ),
        Err(error) => report("database", false, sanitize(&error.to_string()), &mut failed),
    }
    check_bind("rest-bind", &config.bind_address, &mut failed).await;
    check_bind("grpc-bind", &config.grpc_bind_address, &mut failed).await;

    if failed {
        Err("doctor found one or more failures".into())
    } else {
        println!("result=healthy");
        Ok(())
    }
}

async fn check_bind(name: &str, raw: &str, failed: &mut bool) {
    match raw.parse::<SocketAddr>() {
        Ok(address) => match tokio::net::TcpListener::bind(address).await {
            Ok(listener) => {
                drop(listener);
                report(name, true, "available".to_owned(), failed);
            }
            Err(error) => report(name, false, format!("unavailable: {}", error.kind()), failed),
        },
        Err(_) => report(name, false, "invalid socket address".to_owned(), failed),
    }
}

fn report(name: &str, ok: bool, detail: String, failed: &mut bool) {
    println!("{name}={} detail={detail}", if ok { "ok" } else { "fail" });
    *failed |= !ok;
}

fn sanitize(message: &str) -> String {
    if message.len() > 240 {
        format!("{}â€¦", &message[..240])
    } else {
        message.to_owned()
    }
}
