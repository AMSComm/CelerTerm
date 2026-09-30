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
            if rustix::process::test_kill_process(proc_pid).is_err() {
                return false;
            }
            if pid == std::process::id() {
                return true;
            }
            if let Some(name) = crate::pty::get_process_name(pid) {
                let lower = name.to_lowercase();
                lower.contains("celerterm")
            } else {
                true
            }
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
            let original_count = instances.len();
            let alive: Vec<ActiveInstance> = instances.into_iter().filter(|inst| is_process_alive(inst.pid)).collect();
            if alive.len() < original_count {
                let _ = save_instances_to_file(&alive, path);
            }
            return alive;
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

pub fn unregister_all_instances() {
    if let Some(path) = get_default_instances_path() {
        let _ = save_instances_to_file(&[], &path);
    }
}

pub fn close_instance(pid: u32) -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSRunningApplication;
        let success = unsafe {
            if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32) {
                app.terminate()
            } else {
                false
            }
        };
        if success {
            return true;
        }
    }

    #[cfg(unix)]
    {
        if let Some(proc_pid) = rustix::process::Pid::from_raw(pid as i32) {
            return rustix::process::kill_process(proc_pid, rustix::process::Signal::Term).is_ok();
        }
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
    }

    false
}

pub fn close_all_other_instances(my_pid: u32) {
    let instances = get_all_active_instances();
    for inst in instances {
        if inst.pid != my_pid {
            close_instance(inst.pid);
        }
    }
}

pub fn get_all_active_instances() -> Vec<ActiveInstance> {
    if let Some(path) = get_default_instances_path() {
        load_instances_from_file(&path)
    } else {
        Vec::new()
    }
}

pub fn find_other_instance_in<'a>(
    instances: &'a [ActiveInstance],
    my_pid: u32,
    ws_id: &str,
    ws_name: &str,
) -> Option<&'a ActiveInstance> {
    instances.iter().find(|inst| {
        inst.pid != my_pid
            && ((!ws_id.is_empty() && inst.workspace_id == ws_id)
                || (!ws_name.is_empty() && inst.workspace_name == ws_name))
    })
}

pub fn find_other_instance_for_workspace(
    my_pid: u32,
    ws_id: &str,
    ws_name: &str,
) -> Option<ActiveInstance> {
    let instances = get_all_active_instances();
    find_other_instance_in(&instances, my_pid, ws_id, ws_name).cloned()
}

pub fn focus_instance(pid: u32) -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSRunningApplication, NSApplicationActivationOptions};
        unsafe {
            if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32) {
                #[allow(deprecated)]
                let success = app.activateWithOptions(NSApplicationActivationOptions::NSApplicationActivateIgnoringOtherApps);
                if success {
                    return true;
                }
            }
        }
        let script = format!(
            "tell application \"System Events\" to set frontmost of the first process whose unix id is {} to true",
            pid
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn();
        true
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("xdotool search --pid {pid} windowactivate 2>/dev/null || wmctrl -i -a $(wmctrl -lp | awk '$3 == {pid} {{print $1}}') 2>/dev/null", pid = pid))
            .spawn();
        true
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        false
    }
}

pub fn cycle_next_instance_in(instances: &[ActiveInstance], my_pid: u32) -> Option<u32> {
    if instances.len() <= 1 {
        return None;
    }
    let mut sorted = instances.to_vec();
    sorted.sort_by_key(|i| i.pid);
    if let Some(pos) = sorted.iter().position(|i| i.pid == my_pid) {
        let next_idx = (pos + 1) % sorted.len();
        Some(sorted[next_idx].pid)
    } else {
        sorted.first().map(|i| i.pid)
    }
}

pub fn cycle_next_instance(my_pid: u32) -> bool {
    let instances = get_all_active_instances();
    if let Some(next_pid) = cycle_next_instance_in(&instances, my_pid) {
        focus_instance(next_pid)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_other_instance_in() {
        let instances = vec![
            ActiveInstance {
                pid: 1001,
                workspace_name: "Dev".to_string(),
                workspace_id: "ws-1".to_string(),
                updated_at: 100,
            },
            ActiveInstance {
                pid: 1002,
                workspace_name: "Ops".to_string(),
                workspace_id: "ws-2".to_string(),
                updated_at: 200,
            },
        ];

        // Searching from pid 1001 for its own workspace -> None
        assert_eq!(find_other_instance_in(&instances, 1001, "ws-1", "Dev"), None);

        // Searching from pid 1001 for "Ops" -> finds 1002
        let other = find_other_instance_in(&instances, 1001, "ws-2", "Ops");
        assert!(other.is_some());
        assert_eq!(other.unwrap().pid, 1002);

        // Searching by ID only
        let other_by_id = find_other_instance_in(&instances, 1001, "ws-2", "");
        assert_eq!(other_by_id.unwrap().pid, 1002);

        // Searching by Name only
        let other_by_name = find_other_instance_in(&instances, 1001, "", "Ops");
        assert_eq!(other_by_name.unwrap().pid, 1002);

        // Non-existent workspace
        assert_eq!(find_other_instance_in(&instances, 1001, "ws-99", "Unknown"), None);
    }

    #[test]
    fn test_cycle_next_instance_in() {
        let instances = vec![
            ActiveInstance {
                pid: 1001,
                workspace_name: "Dev".to_string(),
                workspace_id: "ws-1".to_string(),
                updated_at: 100,
            },
            ActiveInstance {
                pid: 1002,
                workspace_name: "Backend".to_string(),
                workspace_id: "ws-2".to_string(),
                updated_at: 200,
            },
            ActiveInstance {
                pid: 1003,
                workspace_name: "Frontend".to_string(),
                workspace_id: "ws-3".to_string(),
                updated_at: 300,
            },
        ];

        // Cycling from 1001 goes to 1002
        assert_eq!(cycle_next_instance_in(&instances, 1001), Some(1002));
        // Cycling from 1002 goes to 1003
        assert_eq!(cycle_next_instance_in(&instances, 1002), Some(1003));
        // Cycling from 1003 wraps around to 1001
        assert_eq!(cycle_next_instance_in(&instances, 1003), Some(1001));

        // Single instance should return None (nowhere to cycle)
        assert_eq!(cycle_next_instance_in(&instances[..1], 1001), None);
        // Empty instances returns None
        assert_eq!(cycle_next_instance_in(&[], 1001), None);
    }

    #[test]
    fn test_process_alive_check() {
        let current_pid = std::process::id();
        assert!(is_process_alive(current_pid));

        // Unlikely / dead PID
        let dead_pid = 9_999_999;
        assert!(!is_process_alive(dead_pid));
    }

    #[test]
    fn test_load_and_prune_dead_instances() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let path = temp_dir.path().join("active_instances.json");

        let current_pid = std::process::id();
        let dead_pid = 9_999_999;

        let instances = vec![
            ActiveInstance {
                pid: current_pid,
                workspace_name: "Alive WS".to_string(),
                workspace_id: "alive-1".to_string(),
                updated_at: 1000,
            },
            ActiveInstance {
                pid: dead_pid,
                workspace_name: "Dead WS".to_string(),
                workspace_id: "dead-2".to_string(),
                updated_at: 500,
            },
        ];

        save_instances_to_file(&instances, &path).expect("Failed to save instances");
        assert!(path.exists());

        // Loading should filter out the dead PID and prune the file
        let loaded = load_instances_from_file(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].pid, current_pid);

        // Re-read raw content from file to verify it was pruned on disk
        let disk_content = fs::read_to_string(&path).expect("Read pruned file");
        let pruned: Vec<ActiveInstance> = serde_json::from_str(&disk_content).expect("Parse pruned json");
        assert_eq!(pruned.len(), 1);
        assert_eq!(pruned[0].pid, current_pid);
    }

    #[test]
    fn test_close_instance_safe_on_nonexistent() {
        assert!(!close_instance(9_999_999));
    }
}




