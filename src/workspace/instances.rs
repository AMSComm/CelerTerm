use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveInstance {
    pub pid: u32,
    pub workspace_name: String,
    pub workspace_id: String,
    #[serde(default)]
    pub updated_at: u64,
}

pub fn get_default_instances_path() -> Option<PathBuf> {
    if let Some(base_dirs) = directories::BaseDirs::new() {
        return Some(base_dirs.home_dir().join(".config").join("celerterm").join("active_instances.json"));
    }
    directories::ProjectDirs::from("com", "celerterm", "celerterm")
        .map(|dirs| dirs.config_dir().join("active_instances.json"))
}

pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        if let Some(proc_pid) = rustix::process::Pid::from_raw(pid as i32) {
            rustix::process::test_kill_process(proc_pid).is_ok()
        } else {
            false
        }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub fn load_instances_from_file(path: &Path) -> Vec<ActiveInstance> {
    if !path.exists() {
        return Vec::new();
    }
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(instances) = serde_json::from_str::<Vec<ActiveInstance>>(&content) {
            return instances.into_iter().filter(|inst| is_process_alive(inst.pid)).collect();
        }
    }
    Vec::new()
}

pub fn save_instances_to_file(instances: &[ActiveInstance], path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(instances)?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, content)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

pub fn register_active_instance(pid: u32, ws_name: &str, ws_id: &str) {
    if let Some(path) = get_default_instances_path() {
        let mut instances = load_instances_from_file(&path);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if let Some(existing) = instances.iter_mut().find(|i| i.pid == pid) {
            existing.workspace_name = ws_name.to_string();
            existing.workspace_id = ws_id.to_string();
            existing.updated_at = now;
        } else {
            instances.push(ActiveInstance {
                pid,
                workspace_name: ws_name.to_string(),
                workspace_id: ws_id.to_string(),
                updated_at: now,
            });
        }
        let _ = save_instances_to_file(&instances, &path);
    }
}

pub fn unregister_active_instance(pid: u32) {
    if let Some(path) = get_default_instances_path() {
        let mut instances = load_instances_from_file(&path);
        instances.retain(|i| i.pid != pid);
        let _ = save_instances_to_file(&instances, &path);
    }
}

pub fn get_all_active_instances() -> Vec<ActiveInstance> {
    if let Some(path) = get_default_instances_path() {
        load_instances_from_file(&path)
    } else {
        Vec::new()
    }
}
