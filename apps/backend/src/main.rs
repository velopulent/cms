use std::error::Error;

use clap::{CommandFactory, Parser};
use cms::cli::{AdminAction, BackupAction, Cli, Command, ConfigAction, McpTransport, SecretsAction};
use cms::database::init_db_with_config;
use cms::repository::Repository;
use tracing::info;

fn main() {
    let cli = Cli::parse();
    if cli.command.is_none() {
        let _ = Cli::command().print_help();
        println!();
        return;
    }

    #[cfg(windows)]
    if let Some(Command::Service {
        action: cms::cli::ServiceAction::Run,
    }) = &cli.command
    {
        if let Err(error) = cms::service::run_service_sync(
            match &cli.command {
                Some(Command::Service { action }) => action,
                _ => unreachable!(),
            },
            &cli,
        ) {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
        return;
    }

    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| -> Box<dyn Error> { Box::new(error) })
        .and_then(|runtime| runtime.block_on(dispatch(cli)));
    if let Err(error) = result {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

async fn dispatch(cli: Cli) -> Result<(), Box<dyn Error>> {
    match &cli.command {
        Some(Command::Config { action }) => run_config(action).await,
        Some(Command::Secrets { action }) => run_secrets(action).await,
        Some(Command::Admin { action }) => run_admin(action).await,
        Some(Command::Backup { action }) => run_backup(action).await,
        Some(Command::Restore {
            file,
            scope,
            site,
            import_as_new,
            yes,
        }) => run_restore(file, scope, site, *import_as_new, *yes).await,
        Some(Command::Service { action }) => cms::service::run_service(action, &cli).await,
        Some(Command::Doctor) => cms::diagnostics::run().await,
        Some(Command::Mcp {
            transport: McpTransport::Stdio,
        }) => run_mcp_stdio().await,
        Some(Command::Serve) => {
            if cms::service::is_installed()? {
                return Err("native vcms service is installed; portable `vcms serve` is disabled".into());
            }
            let context = cms::runtime::RuntimeContext::initialize(cms::paths::RuntimeMode::Portable)?;
            cms::server::run(context, cms::server::shutdown_signal(), || {}).await
        }
        None => Ok(()),
    }
}

async fn run_mcp_stdio() -> Result<(), Box<dyn Error>> {
    cms::tracing::init_proxy_tracing();
    let token = std::env::var("VCMS_MCP_TOKEN").map_err(|_| "VCMS_MCP_TOKEN is required for `vcms mcp stdio`")?;
    let token = token.trim().to_owned();
    if token.is_empty() {
        return Err("VCMS_MCP_TOKEN must not be empty".into());
    }
    let base = std::env::var("VCMS_MCP_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_owned());
    let endpoint = format!("{}/mcp", base.trim_end_matches('/'));
    info!(%endpoint, "Starting MCP stdio proxy");
    cms::mcp::transports::stdio::serve(endpoint, token).await
}

async fn run_config(action: &ConfigAction) -> Result<(), Box<dyn Error>> {
    match action {
        ConfigAction::Show => {
            let context = cms::runtime::RuntimeContext::initialize_default()?;
            print!("{}", context.bootstrap.redacted_toml(&context.paths));
            match cms::database::connect_db_without_migrations(&context.bootstrap).await {
                Ok(pool) => {
                    match cms::services::settings::SettingsService::load(pool, &context.secrets.master_key).await {
                        Ok(settings) => println!(
                            "\n[instance_settings]\n{}",
                            serde_json::to_string_pretty(settings.current().as_ref())?
                        ),
                        Err(error) => println!("\ninstance_settings = \"unavailable: {error}\""),
                    }
                }
                Err(_) => println!("\ninstance_settings = \"unavailable until database initialization\""),
            }
        }
    }
    Ok(())
}

async fn run_secrets(action: &SecretsAction) -> Result<(), Box<dyn Error>> {
    match action {
        SecretsAction::Reset { yes } => {
            if !yes {
                return Err(
                    "Secret reset invalidates API tokens and integration credentials, generates a new backup encryption key, and makes existing encrypted backups unreadable. Re-run with --yes.".into(),
                );
            }
            let mode = if cms::service::is_installed()? {
                cms::paths::RuntimeMode::Installed
            } else {
                cms::paths::RuntimeMode::Portable
            };
            let paths = cms::paths::RuntimePaths::for_mode(mode)?;
            paths.ensure()?;
            cms::config::ensure_bootstrap(&paths)?;
            let existing = cms::secrets::load(&paths)?.ok_or("No existing instance secrets were found to reset")?;
            if existing.database_url.is_none() && !paths.database_file().exists() {
                return Err("No existing instance database was found to reset".into());
            }
            let old_database_url = existing.database_url;
            let fresh = cms::secrets::fresh(old_database_url);
            let config = cms::config::Config::load(&paths, &fresh)?;
            let pool = init_db_with_config(&config).await?;
            let (tokens, webhooks, s3_sites) = invalidate_credentials(&pool).await?;
            cms::secrets::replace(&paths, &fresh)?;
            println!("Trust root replaced. Recovery report:");
            println!("- {tokens} API access token(s) invalidated");
            println!("- Active dashboard sessions invalidated");
            println!("- {webhooks} webhook(s) disabled; secret headers cleared");
            println!("- Encrypted storage and backup credentials cleared");
            if s3_sites > 0 {
                println!("- {s3_sites} S3-backed site(s) require new storage credentials");
            }
        }
    }
    Ok(())
}

async fn invalidate_credentials(pool: &cms::database::pool::DbPool) -> Result<(i64, i64, i64), Box<dyn Error>> {
    use cms::database::pool::DbPool;
    macro_rules! invalidate {
        ($pool:expr, $disabled:expr) => {{
            let mut tx = $pool.begin().await?;
            let tokens = sqlx::query_scalar("SELECT COUNT(*) FROM access_tokens")
                .fetch_one(&mut *tx)
                .await?;
            let webhooks = sqlx::query_scalar("SELECT COUNT(*) FROM site_webhooks WHERE headers_encrypted <> ''")
                .fetch_one(&mut *tx)
                .await?;
            let sites = sqlx::query_scalar("SELECT COUNT(*) FROM sites WHERE storage_provider = 's3'")
                .fetch_one(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM access_tokens").execute(&mut *tx).await?;
            sqlx::query("DELETE FROM sessions").execute(&mut *tx).await?;
            sqlx::query($disabled).execute(&mut *tx).await?;
            sqlx::query("UPDATE instance_settings SET credentials_encrypted = NULL")
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            Ok((tokens, webhooks, sites))
        }};
    }
    match pool {
        DbPool::Postgres(pool) => invalidate!(pool, "UPDATE site_webhooks SET headers_encrypted = '', enabled = FALSE"),
        DbPool::MySql(pool) => invalidate!(pool, "UPDATE site_webhooks SET headers_encrypted = '', enabled = FALSE"),
        DbPool::Sqlite(pool) => invalidate!(pool, "UPDATE site_webhooks SET headers_encrypted = '', enabled = 0"),
    }
}

async fn run_admin(action: &AdminAction) -> Result<(), Box<dyn Error>> {
    let context = cms::runtime::RuntimeContext::initialize_default()?;
    let pool = init_db_with_config(&context.bootstrap).await?;
    let repository = Repository::new(&pool);
    match action {
        AdminAction::ResetPassword { email, password } => {
            let id = repository
                .user
                .find_by_email(email)
                .await?
                .ok_or_else(|| format!("User with email '{email}' not found"))?
                .id;
            let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
            repository.user.update_password(&id, &password_hash, false).await?;
            println!("Password updated for user with email '{email}'.");
        }
    }
    Ok(())
}

async fn run_backup(action: &BackupAction) -> Result<(), Box<dyn Error>> {
    use cms::services::backup::{BackupService, CreateBackupOptions, build_backup_destination, meta};

    let context = cms::runtime::RuntimeContext::initialize_default()?;
    let mut config = context.bootstrap;
    let pool = init_db_with_config(&config).await?;
    let settings = cms::services::settings::SettingsService::load(pool.clone(), &context.secrets.master_key).await?;
    settings.apply_to_config(&mut config).await;
    let storage_registry = cms::server::initialize_storage(&config);
    let destination = build_backup_destination(&config)?;
    let service = BackupService::new(pool.clone(), storage_registry, destination, &config).with_settings(settings);

    match action {
        BackupAction::Create {
            scope,
            site,
            out,
            no_files,
            encrypt,
        } => {
            let scope = parse_scope(scope, site.as_deref())?;
            let include_files = !no_files;
            if let Some(out) = out {
                let (manifest, bytes) = service.build_artifact(&scope, include_files, *encrypt).await?;
                std::fs::write(out, &bytes)?;
                println!(
                    "Wrote backup to {} ({} bytes, {} tables, {} files)",
                    out.display(),
                    bytes.len(),
                    manifest.tables.len(),
                    manifest.files.len()
                );
            } else {
                let row = service
                    .create_backup(CreateBackupOptions {
                        scope,
                        include_files,
                        encrypt: *encrypt,
                        schedule_id: None,
                        created_by: None,
                    })
                    .await?;
                println!(
                    "Backup {} created ({} bytes) -> {}",
                    row.id,
                    row.size_bytes,
                    row.destination_key.unwrap_or_default()
                );
            }
        }
        BackupAction::List => {
            let rows = meta::list_backups(service.pool(), None, None).await?;
            if rows.is_empty() {
                println!("No backups recorded.");
            }
            for row in rows {
                println!(
                    "{}  {:8}  scope={:8} site={:36}  {}  {} bytes  {}",
                    row.id,
                    row.status,
                    row.scope,
                    row.site_id.unwrap_or_else(|| "-".into()),
                    if row.encrypted != 0 { "encrypted" } else { "plaintext" },
                    row.size_bytes,
                    row.created_at
                );
            }
        }
    }
    Ok(())
}

async fn run_restore(
    file: &std::path::Path,
    scope: &str,
    site: &Option<String>,
    import_as_new: bool,
    yes: bool,
) -> Result<(), Box<dyn Error>> {
    use cms::services::backup::{
        BackupService, RestoreRequest, RestoreSource, RestoreTarget, build_backup_destination,
    };
    if !yes {
        return Err("Restore replaces data within the chosen scope. Re-run with --yes.".into());
    }

    let context = cms::runtime::RuntimeContext::initialize_default()?;
    let mut config = context.bootstrap;
    let pool = init_db_with_config(&config).await?;
    let settings = cms::services::settings::SettingsService::load(pool.clone(), &context.secrets.master_key).await?;
    settings.apply_to_config(&mut config).await;
    let storage_registry = cms::server::initialize_storage(&config);
    let destination = build_backup_destination(&config)?;
    let service = BackupService::new(pool, storage_registry, destination, &config).with_settings(settings);
    let target = match scope {
        "instance" => RestoreTarget::WholeInstance,
        "site" => RestoreTarget::Site {
            site_id: site.clone().ok_or("--site <SITE_ID> is required for --scope site")?,
            import_as_new,
        },
        other => return Err(format!("unknown scope '{other}' (use instance|site)").into()),
    };
    let report = service
        .restore(RestoreRequest {
            source: RestoreSource::Bytes(std::fs::read(file)?),
            target,
            created_by: None,
        })
        .await?;
    println!("Restore complete. Recovery required:");
    for item in report.recovery_required {
        println!("- {item}");
    }
    Ok(())
}

fn parse_scope(scope: &str, site: Option<&str>) -> Result<cms::services::backup::Scope, Box<dyn Error>> {
    use cms::services::backup::Scope;
    match scope {
        "instance" => Ok(Scope::Instance),
        "site" => site
            .map(|value| Scope::Site(value.to_owned()))
            .ok_or_else(|| "--site <SITE_ID> is required for --scope site".into()),
        other => Err(format!("unknown scope '{other}' (use instance|site)").into()),
    }
}
