use crate::config::Config;
use crate::paths::{RuntimeMode, RuntimePaths};
use crate::secrets::PersistedSecrets;

#[derive(Clone, Debug)]
pub struct RuntimeContext {
    pub mode: RuntimeMode,
    pub paths: RuntimePaths,
    pub bootstrap: Config,
    pub secrets: PersistedSecrets,
}

impl RuntimeContext {
    pub fn initialize(mode: RuntimeMode) -> Result<Self, Box<dyn std::error::Error>> {
        let paths = RuntimePaths::for_mode(mode)?;
        paths
            .ensure()
            .map_err(|error| format!("preparing {}: {error}", paths.root().display()))?;
        crate::config::ensure_bootstrap(&paths)?;
        let secrets = crate::secrets::ensure(&paths)?;
        let bootstrap = Config::load(&paths)?;
        Ok(Self {
            mode,
            paths,
            bootstrap,
            secrets,
        })
    }

    pub fn initialize_default() -> Result<Self, Box<dyn std::error::Error>> {
        let mode = if crate::service::is_installed()? {
            RuntimeMode::Installed
        } else {
            RuntimeMode::Portable
        };
        Self::initialize(mode)
    }
}
