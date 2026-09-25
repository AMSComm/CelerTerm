pub mod schema;

use std::fs;
use std::path::{Path, PathBuf};
use directories::BaseDirs;
use log::{info, warn};

pub use schema::Config;

pub fn get_config_path() -> Option<PathBuf> {
    if let Some(base_dirs) = BaseDirs::new() {
        let p = base_dirs.home_dir().join(".config").join("celerterm").join("config.toml");
        return Some(p);
    }
    directories::ProjectDirs::from("com", "celerterm", "celerterm")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

pub fn load_config() -> Config {
    if let Some(path) = get_config_path() {
        if path.exists() {
            match load_from_file(&path) {
                Ok(cfg) => {
                    info!("Loaded configuration from {}", path.display());
                    return cfg;
                }
                Err(err) => {
                    warn!("Failed to load configuration at {}: {}. Falling back to default.", path.display(), err);
                }
            }
        } else {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let default_toml = r##"# CelerTerm Configuration
# Location: ~/.config/celerterm/config.toml

[font]
# Primary monospace font family (e.g. "Firple", "JetBrainsMono NF", "CaskaydiaMono NF")
family = "Firple"
size = 13.0
ligatures = true
line_height = 1.2
fallback_families = [
    "CaskaydiaCove Nerd Font Mono",
    "CaskaydiaMono NF",
    "JetBrainsMono NF",
    "JetBrains Mono",
    "Menlo",
]

[window]
padding_x = 8.0
padding_y = 4.0
hide_traffic_lights = true
tabs_in_titlebar = true
opacity = 1.0

[macos]
# Treat Option key as Alt in Neovim / readline navigation
option_as_alt = true

[colors]
background = "#1a1b26"
foreground = "#c0caf5"
cursor = "#7aa2f7"
"##;
            let _ = fs::write(&path, default_toml);
            if let Ok(cfg) = load_from_file(&path) {
                info!("Created default configuration at {}", path.display());
                return cfg;
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
