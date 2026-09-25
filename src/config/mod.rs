pub mod schema;

use std::fs;
use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use log::{info, warn};

pub use schema::Config;

pub fn get_config_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "celerterm", "celerterm")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

pub fn load_config() -> Config {
    if let Some(path) = get_config_path()
        && path.exists()
    {
        match load_from_file(&path) {
            Ok(cfg) => {
                info!("Loaded configuration from {}", path.display());
                return cfg;
            }
            Err(err) => {
                warn!("Failed to load configuration at {}: {}. Falling back to default.", path.display(), err);
            }
        }
    }
    Config::default()
}

pub fn load_from_file(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
