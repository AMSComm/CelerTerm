use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub window: WindowConfig,
    pub macos: MacOsConfig,
    pub font: FontConfig,
    pub workspace: WorkspaceConfig,
    pub colors: ColorScheme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub hide_traffic_lights: bool,
    pub tabs_in_titlebar: bool,
    pub decorations: bool,
    pub opacity: f32,
    pub padding_x: f32,
    pub padding_y: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            hide_traffic_lights: false,
            tabs_in_titlebar: true,
            decorations: true,
            opacity: 1.0,
            padding_x: 8.0,
            padding_y: 4.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MacOsConfig {
    pub option_as_alt: bool,
}

impl Default for MacOsConfig {
    fn default() -> Self {
        Self {
            option_as_alt: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    pub ligatures: bool,
    pub line_height: f32,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: "JetBrainsMono Nerd Font".to_string(),
            size: 14.0,
            ligatures: true,
            line_height: 1.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub restore_on_startup: bool,
    pub save_scrollback: bool,
    pub max_scrollback_lines: usize,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            restore_on_startup: true,
            save_scrollback: true,
            max_scrollback_lines: 10000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorScheme {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub selection_background: String,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            foreground: "#c0caf5".to_string(),
            background: "#1a1b26".to_string(),
            cursor: "#c0caf5".to_string(),
            selection_background: "#33467c".to_string(),
        }
    }
}
