use super::manager::WorkspaceManager;
use directories::ProjectDirs;
use std::fs;
use std::path::{Path, PathBuf};

pub fn get_default_snapshot_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "celerterm", "celerterm")
        .map(|dirs| dirs.data_local_dir().join("workspace_snapshot.json"))
}

pub fn save_snapshot_to_string(manager: &WorkspaceManager) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(manager)
}

pub fn load_snapshot_from_str(json: &str) -> Result<WorkspaceManager, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn save_snapshot_to_file(manager: &WorkspaceManager, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = save_snapshot_to_string(manager)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn load_snapshot_from_file(path: &Path) -> Result<WorkspaceManager, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let manager = load_snapshot_from_str(&content)?;
    Ok(manager)
}
