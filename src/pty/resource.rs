use std::collections::{HashSet, VecDeque};
use super::process::{get_child_pids, get_raw_process_name};

#[derive(Debug, Clone, Default)]
pub struct ProcessUsage {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub memory_bytes: u64,
    pub cpu_time_ns: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct PaneResource {
    pub pane_id: String,
    pub title: String,
    pub root_pid: Option<u32>,
    pub total_memory_bytes: u64,
    pub total_cpu_time_ns: u64,
    pub total_disk_read_bytes: u64,
    pub total_disk_write_bytes: u64,
    pub processes: Vec<ProcessUsage>,
}

#[derive(Debug, Clone, Default)]
pub struct TabResource {
    pub tab_id: String,
    pub title: String,
    pub total_memory_bytes: u64,
    pub total_cpu_time_ns: u64,
    pub total_disk_read_bytes: u64,
    pub total_disk_write_bytes: u64,
    pub panes: Vec<PaneResource>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceResource {
    pub workspace_id: String,
    pub name: String,
    pub color: Option<String>,
    pub total_memory_bytes: u64,
    pub total_cpu_time_ns: u64,
    pub total_disk_read_bytes: u64,
    pub total_disk_write_bytes: u64,
    pub tabs: Vec<TabResource>,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceSnapshot {
    pub app_pid: u32,
    pub app_memory_bytes: u64,
    pub app_cpu_time_ns: u64,
    pub total_tree_memory_bytes: u64,
    pub total_tree_cpu_time_ns: u64,
    pub total_tree_disk_read_bytes: u64,
    pub total_tree_disk_write_bytes: u64,
    pub workspaces: Vec<WorkspaceResource>,
    pub captured_at: Option<std::time::Instant>,
}

#[cfg(target_os = "macos")]
mod sys {
    use std::os::raw::{c_int, c_void};
    use super::ProcessUsage;

    const RUSAGE_INFO_V4: c_int = 4;

    #[repr(C)]
    struct RUsageInfoV4 {
        ri_uuid: [u8; 16],
        ri_user_time: u64,
        ri_system_time: u64,
        ri_pkg_idle_wkups: u64,
        ri_interrupt_wkups: u64,
        ri_pageins: u64,
        ri_wired_size: u64,
        ri_resident_size: u64,
        ri_phys_footprint: u64,
        ri_proc_start_abstime: u64,
        ri_proc_exit_abstime: u64,
        ri_child_user_time: u64,
        ri_child_system_time: u64,
        ri_child_pkg_idle_wkups: u64,
        ri_child_interrupt_wkups: u64,
        ri_child_pageins: u64,
        ri_child_elapsed_abstime: u64,
        ri_diskio_bytesread: u64,
        ri_diskio_byteswritten: u64,
        ri_cpu_time_qos_default: u64,
        ri_cpu_time_qos_maintenance: u64,
        ri_cpu_time_qos_background: u64,
        ri_cpu_time_qos_utility: u64,
        ri_cpu_time_qos_legacy: u64,
        ri_cpu_time_qos_user_initiated: u64,
        ri_cpu_time_qos_user_interactive: u64,
        ri_billed_system_time: u64,
        ri_serviced_system_time: u64,
    }

    unsafe extern "C" {
        fn proc_pid_rusage(pid: c_int, flavor: c_int, buffer: *mut c_void) -> c_int;
    }

    pub fn get_process_usage(pid: u32, ppid: u32) -> Option<ProcessUsage> {
        let mut rusage = std::mem::MaybeUninit::<RUsageInfoV4>::zeroed();
        let ret = unsafe {
            proc_pid_rusage(pid as c_int, RUSAGE_INFO_V4, rusage.as_mut_ptr() as *mut c_void)
        };
        if ret == 0 {
            let info = unsafe { rusage.assume_init() };
            let raw_name = super::get_raw_process_name(pid)
                .unwrap_or_else(|| format!("PID:{}", pid));
            let memory = if info.ri_phys_footprint > 0 {
                info.ri_phys_footprint
            } else {
                info.ri_resident_size
            };
            Some(ProcessUsage {
                pid,
                ppid,
                name: raw_name,
                memory_bytes: memory,
                cpu_time_ns: info.ri_user_time.saturating_add(info.ri_system_time),
                disk_read_bytes: info.ri_diskio_bytesread,
                disk_write_bytes: info.ri_diskio_byteswritten,
            })
        } else {
            None
        }
    }
}

#[cfg(target_os = "linux")]
mod sys {
    use std::fs;
    use super::ProcessUsage;

    pub fn get_process_usage(pid: u32, ppid: u32) -> Option<ProcessUsage> {
        let name = super::get_raw_process_name(pid)
            .unwrap_or_else(|| format!("PID:{}", pid));

        let mut cpu_time_ns: u64 = 0;
        if let Ok(stat_content) = fs::read_to_string(format!("/proc/{}/stat", pid)) {
            // Find end of comm in stat: "(comm)"
            if let Some(idx) = stat_content.rfind(')') {
                let rest = &stat_content[idx + 1..];
                let fields: Vec<&str> = rest.split_whitespace().collect();
                // After 'comm' and 'state', fields are 0-indexed:
                // fields[0] is state, fields[1] is ppid,
                // fields[11] is utime, fields[12] is stime (clock ticks)
                if fields.len() > 12 {
                    let utime: u64 = fields[11].parse().unwrap_or(0);
                    let stime: u64 = fields[12].parse().unwrap_or(0);
                    // Approx 100 ticks per second (10ms per tick)
                    cpu_time_ns = utime.saturating_add(stime).saturating_mul(10_000_000);
                }
            }
        }

        let mut memory_bytes: u64 = 0;
        if let Ok(statm_content) = fs::read_to_string(format!("/proc/{}/statm", pid)) {
            let fields: Vec<&str> = statm_content.split_whitespace().collect();
            if fields.len() > 1 {
                let resident_pages: u64 = fields[1].parse().unwrap_or(0);
                memory_bytes = resident_pages.saturating_mul(4096);
            }
        }

        let mut disk_read_bytes: u64 = 0;
        let mut disk_write_bytes: u64 = 0;
        if let Ok(io_content) = fs::read_to_string(format!("/proc/{}/io", pid)) {
            for line in io_content.lines() {
                if let Some(val_str) = line.strip_prefix("read_bytes:") {
                    disk_read_bytes = val_str.trim().parse().unwrap_or(0);
                } else if let Some(val_str) = line.strip_prefix("write_bytes:") {
                    disk_write_bytes = val_str.trim().parse().unwrap_or(0);
                }
            }
        }

        Some(ProcessUsage {
            pid,
            ppid,
            name,
            memory_bytes,
            cpu_time_ns,
            disk_read_bytes,
            disk_write_bytes,
        })
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod sys {
    use super::ProcessUsage;
    pub fn get_process_usage(_pid: u32, _ppid: u32) -> Option<ProcessUsage> {
        None
    }
}

/// Retrieves the process resource stats for a PID.
pub fn get_single_process_usage(pid: u32, ppid: u32) -> Option<ProcessUsage> {
    sys::get_process_usage(pid, ppid)
}

/// Walks the process tree starting from `root_pid` and collects stats for all children.
pub fn collect_process_tree(root_pid: u32) -> Vec<ProcessUsage> {
    let mut queue = VecDeque::new();
    queue.push_back((root_pid, 0));
    let mut visited = HashSet::new();
    let mut results = Vec::new();

    while let Some((pid, ppid)) = queue.pop_front() {
        if !visited.insert(pid) {
            continue;
        }

        if let Some(usage) = sys::get_process_usage(pid, ppid) {
            results.push(usage);
        }

        let children = get_child_pids(pid);
        for child in children {
            if !visited.contains(&child) {
                queue.push_back((child, pid));
            }
        }
    }

    results
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_duration_ns(ns: u64) -> String {
    let total_secs = (ns as f64) / 1_000_000_000.0;
    if total_secs < 60.0 {
        format!("{:.1}s", total_secs)
    } else {
        let mins = (total_secs / 60.0).floor() as u64;
        let secs = (total_secs % 60.0).round() as u64;
        format!("{}m {:02}s", mins, secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(2048), "2 KB");
        assert_eq!(format_bytes(10 * 1024 * 1024), "10.0 MB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration_ns(500_000_000), "0.5s");
        assert_eq!(format_duration_ns(65_000_000_000), "1m 05s");
    }

    #[test]
    fn test_current_process_usage() {
        let pid = std::process::id();
        let usage = get_single_process_usage(pid, 0);
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            assert!(usage.is_some(), "Process usage should be available on macOS/Linux");
            let u = usage.unwrap();
            assert_eq!(u.pid, pid);
            assert!(u.memory_bytes > 0, "Memory usage should be greater than 0");
        }
    }
}
