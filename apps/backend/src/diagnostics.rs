use std::net::SocketAddr;
use std::path::Path;

use crate::cli::Cli;
use crate::config::{self, Config};
use crate::database::connect_db_without_migrations;

pub async fn run(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let mut failed = false;
    let config_path = config::resolve_config_path(cli);
    let loaded = Config::load(cli).map_err(|error| format!("configuration invalid: {error}"))?;
    report(
        "config",
        config_path.as_deref().is_none_or(Path::is_file),
        config_path
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "built-in defaults (run `vcms config init`)".to_owned()),
        &mut failed,
    );

    let directories = [
        (
            "config-dir",
            crate::paths::config_file().parent().map(Path::to_path_buf),
        ),
        (
            "data-dir",
            crate::paths::default_db_path().parent().map(Path::to_path_buf),
        ),
        ("storage-dir", Some(crate::paths::storage_dir())),
    ];
    for (name, path) in directories {
        if let Some(path) = path {
            let result = std::fs::metadata(&path);
            let ok = result
                .as_ref()
                .is_ok_and(|metadata| metadata.is_dir() && !metadata.permissions().readonly());
            report(name, ok, path.display().to_string(), &mut failed);
        }
    }

    match connect_db_without_migrations(&loaded).await {
        Ok(pool) => {
            report(
                "database",
                pool.ping().await.is_ok(),
                "reachable; schema current".to_owned(),
                &mut failed,
            );
        }
        Err(_) if loaded.database_url.starts_with("sqlite:") && !crate::paths::default_db_path().exists() => {
            report(
                "database",
                true,
                "not initialized; first service start will create and migrate it".to_owned(),
                &mut failed,
            );
        }
        Err(error) => report("database", false, sanitize(&error.to_string()), &mut failed),
    }
    check_bind("rest-bind", &loaded.bind_address, &mut failed).await;
    check_bind("grpc-bind", &loaded.grpc_bind_address, &mut failed).await;
    report(
        "service-identity",
        true,
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_owned()),
        &mut failed,
    );
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
        format!("{}…", &message[..240])
    } else {
        message.to_owned()
    }
}
