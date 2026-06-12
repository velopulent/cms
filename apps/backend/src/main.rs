use clap::Parser;
use cms::cli::{AdminAction, Cli, Command, ConfigAction};
use cms::config::{self, Config};
use cms::database::init_db_with_config;
use cms::grpc::server::spawn_grpc_server;
use cms::repository::Repository;
use cms::router::create_router;
use cms::services::Services;
use cms::storage::{self, STORAGE_KIND_FILESYSTEM, STORAGE_KIND_S3, StorageRegistry};

use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, info, warn};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    let result = match &cli.command {
        Some(Command::Config { action }) => run_config(action, &cli),
        Some(Command::Admin { action }) => run_admin(action, &cli).await,
        Some(Command::Serve) | None => run_serve(&cli).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run_serve(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let config = Config::load(cli)?;

    let _guard = cms::tracing::init_tracing(&config);

    if let Err(e) = config.validate_security() {
        return Err(format!("Invalid production security configuration: {e}").into());
    }

    let pool = init_db_with_config(&config).await?;

    let repository = Repository::new(&pool);

    seed_admin(&repository).await;

    let mut storage_registry = StorageRegistry::new();

    if let Some(ref fs_path) = config.storage_fs_path {
        match storage::FileSystemStorage::new(fs_path) {
            Ok(fs) => {
                storage_registry.register(STORAGE_KIND_FILESYSTEM, Arc::new(fs));
                info!("Filesystem storage initialized at {}", fs_path);
            }
            Err(e) => warn!("Failed to init filesystem storage: {}", e),
        }
    }

    if config.has_s3() {
        match storage::S3Storage::new(
            config.s3_access_key_id.as_deref().unwrap(),
            config.s3_secret_access_key.as_deref().unwrap(),
            config.s3_bucket.as_deref().unwrap(),
            config.s3_region.as_deref().unwrap_or("us-east-1"),
            config.s3_endpoint.as_deref(),
            config.s3_public_url.as_deref(),
        ) {
            Ok(s3) => {
                storage_registry.register(STORAGE_KIND_S3, Arc::new(s3));
                info!("S3 storage initialized");
            }
            Err(e) => warn!("Failed to init S3 storage: {}", e),
        }
    }

    if storage_registry.get(STORAGE_KIND_FILESYSTEM).is_none() && storage_registry.get(STORAGE_KIND_S3).is_none() {
        warn!("No storage providers configured. Set STORAGE_FS_PATH or S3_* env vars.");
    }

    let storage_registry = Arc::new(storage_registry);
    let services = Services::new(Arc::new(repository.clone()), &config);
    let app = create_router(
        repository.clone(),
        config.clone(),
        storage_registry.clone(),
        services.clone(),
    );

    let addr: SocketAddr = config.bind_address.parse().expect("Invalid BIND_ADDRESS");
    info!("REST API server running on {}", addr);

    let grpc_addr: SocketAddr = config.grpc_bind_address.parse().expect("Invalid GRPC_BIND_ADDRESS");
    info!("gRPC server running on {}", grpc_addr);

    let rest_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind address");

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
    });

    let grpc_handle = tokio::spawn(spawn_grpc_server(
        repository.clone(),
        config.clone(),
        storage_registry.clone(),
        grpc_addr,
    ));

    tokio::select! {
        result = rest_handle => {
            if let Err(e) = result {
                tracing::error!("REST server error: {}", e);
            }
        }
        result = grpc_handle => {
            if let Err(e) = result {
                tracing::error!("gRPC server error: {}", e);
            }
        }
    }

    Ok(())
}

fn run_config(action: &ConfigAction, cli: &Cli) -> Result<(), Box<dyn Error>> {
    match action {
        ConfigAction::Init { force, path } => {
            let target = match path {
                Some(p) => p.clone(),
                None => config::user_config_path().ok_or("Could not determine user config directory")?,
            };
            if target.exists() && !force {
                return Err(format!("{} already exists (use --force to overwrite)", target.display()).into());
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, config::default_config_toml())?;
            println!("Wrote default config to {}", target.display());
        }
        ConfigAction::Show => {
            let config = Config::load(cli)?;
            print!("{}", config.redacted_toml());
        }
        ConfigAction::Path => {
            match config::resolve_config_path(cli) {
                Some(p) => println!("Active config file: {}", p.display()),
                None => println!("Active config file: <none found; using built-in defaults>"),
            }
            println!("\nSearch order (first existing wins):");
            match &cli.config {
                Some(explicit) => println!("  1. --config / CMS_CONFIG: {}", explicit.display()),
                None => println!("  1. --config / CMS_CONFIG: <not set>"),
            }
            for (i, p) in config::config_search_paths().iter().enumerate() {
                let marker = if p.exists() { " (exists)" } else { "" };
                println!("  {}. {}{}", i + 2, p.display(), marker);
            }
        }
    }
    Ok(())
}

async fn run_admin(action: &AdminAction, cli: &Cli) -> Result<(), Box<dyn Error>> {
    let config = Config::load(cli)?;
    let pool = init_db_with_config(&config).await?;
    let repository = Repository::new(&pool);

    match action {
        AdminAction::ResetPassword { username, password } => {
            let id = match repository.user.find_id_by_username(username).await? {
                Some(id) => id,
                None => return Err(format!("User '{username}' not found").into()),
            };
            let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
            repository.user.update_password(&id, &password_hash, false).await?;
            println!("Password updated for user '{username}'.");
        }
    }
    Ok(())
}

async fn seed_admin(repository: &Repository) {
    debug!("Checking if admin user needs to be seeded");
    if !repository.user.exists("admin").await.unwrap_or(false) {
        info!("Seeding default admin user");
        let id = uuid::Uuid::now_v7().to_string();
        let password_hash = bcrypt::hash("admin", bcrypt::DEFAULT_COST).expect("Failed to hash password");
        repository
            .user
            .create(&id, "admin", "admin@cms.local", &password_hash)
            .await
            .expect("Failed to seed admin user");
        repository
            .user
            .set_instance_role(&id, Some("instance_owner"))
            .await
            .expect("Failed to assign instance owner");
        repository
            .user
            .update_password(&id, &password_hash, true)
            .await
            .expect("Failed to require password change");

        warn!("Seeded default admin user (admin/admin) — CHANGE THE PASSWORD IMMEDIATELY!");
        eprintln!(
            "\n\
             ============================ SECURITY WARNING ============================\n\
             A default admin account was created:  username 'admin'  password 'admin'\n\
             Anyone who can reach this server can log in until you change it. Run:\n\
             \n    cms admin reset-password --username admin --password <new-strong-password>\n\n\
             or change it from the dashboard now. Do NOT expose this server until done.\n\
             =========================================================================\n"
        );
    } else {
        debug!("Admin user already exists, skipping seeding");
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C, shutting down..."),
        _ = terminate => info!("Received SIGTERM, shutting down..."),
    }
}
