use super::manager::WorkspaceManager;
use directories::ProjectDirs;
use std::fs;
use std::path::{Path, PathBuf};

pub fn get_default_snapshot_path() -> Option<PathBuf> {
    if let Some(base_dirs) = directories::BaseDirs::new() {
        return Some(base_dirs.home_dir().join(".config").join("celerterm").join("workspace_snapshot.json"));
    }
    ProjectDirs::from("com", "celerterm", "celerterm")
        .map(|dirs| dirs.config_dir().join("workspace_snapshot.json"))
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
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, content)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

pub fn load_snapshot_from_file(path: &Path) -> Result<WorkspaceManager, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let manager = load_snapshot_from_str(&content)?;
    Ok(manager)
}

pub fn merge_workspace_managers(
    current: &WorkspaceManager,
    owned_tab_ids: &[String],
    deleted_workspace_ids: &[String],
    disk: Option<&WorkspaceManager>,
    is_secondary_window: bool,
) -> WorkspaceManager {
    let Some(disk) = disk else {
        return current.clone();
    };

    let mut merged_workspaces = Vec::new();

    // 1. Process workspaces in current instance
    for cur_ws in &current.workspaces {
        if deleted_workspace_ids.contains(&cur_ws.id) {
            continue;
        }

        let is_owned = cur_ws.id == current.active_workspace_id
            || cur_ws.tabs.iter().any(|t| owned_tab_ids.contains(&t.id));

        if is_owned {
            merged_workspaces.push(cur_ws.clone());
        } else if let Some(disk_ws) = disk.workspaces.iter().find(|w| w.id == cur_ws.id) {
            merged_workspaces.push(disk_ws.clone());
        }
        // If cur_ws is not owned and not found on disk, it was deleted by another instance.
        // We intentionally omit it so the deletion is respected across windows.
    }

    // 2. Add workspaces from disk that current instance doesn't have in memory
    for disk_ws in &disk.workspaces {
        if deleted_workspace_ids.contains(&disk_ws.id) {
            continue;
        }
        if !merged_workspaces.iter().any(|w| w.id == disk_ws.id) {
            merged_workspaces.push(disk_ws.clone());
        }
    }

    // Determine active workspace id
    let active_workspace_id = if is_secondary_window {
        if merged_workspaces.iter().any(|w| w.id == disk.active_workspace_id) {
            disk.active_workspace_id.clone()
        } else {
            current.active_workspace_id.clone()
        }
    } else if merged_workspaces.iter().any(|w| w.id == current.active_workspace_id) {
        current.active_workspace_id.clone()
    } else if let Some(first) = merged_workspaces.first() {
        first.id.clone()
    } else {
        current.active_workspace_id.clone()
    };

    let mut next_id = current.next_id.max(disk.next_id);
    for ws in &merged_workspaces {
        if let Some(num_str) = ws.id.strip_prefix("ws_") {
            if let Ok(num) = num_str.parse::<usize>() {
                next_id = next_id.max(num + 1);
            }
        }
        for tab in &ws.tabs {
            if let Some(num_str) = tab.id.strip_prefix("tab_") {
                if let Ok(num) = num_str.parse::<usize>() {
                    next_id = next_id.max(num + 1);
                }
            }
        }
    }

    WorkspaceManager {
        workspaces: merged_workspaces,
        active_workspace_id,
        next_id,
    }
}
