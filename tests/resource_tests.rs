use celerterm::pty::resource::{
    collect_process_tree, format_bytes, format_duration_ns, get_single_process_usage,
    PaneResource, ResourceSnapshot, TabResource, WorkspaceResource,
};

#[test]
fn test_resource_formatters() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1023), "1023 B");
    assert_eq!(format_bytes(1024), "1 KB");
    assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(format_bytes(1536 * 1024 * 1024), "1.50 GB");

    assert_eq!(format_duration_ns(0), "0.0s");
    assert_eq!(format_duration_ns(1_500_000_000), "1.5s");
    assert_eq!(format_duration_ns(60_000_000_000), "1m 00s");
    assert_eq!(format_duration_ns(125_000_000_000), "2m 05s");
}

#[test]
fn test_current_process_introspection() {
    let pid = std::process::id();
    let usage = get_single_process_usage(pid, 0);

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        assert!(usage.is_some(), "Current process usage should be resolvable");
        let u = usage.unwrap();
        assert_eq!(u.pid, pid);
        assert!(u.memory_bytes > 0, "Memory footprint must be non-zero");
        assert!(!u.name.is_empty(), "Process name must not be empty");
    }

    let tree = collect_process_tree(pid);
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        assert!(!tree.is_empty(), "Process tree of current test process must contain at least self");
        assert_eq!(tree[0].pid, pid);
    }
}

#[test]
fn test_resource_snapshot_structure() {
    let mut snapshot = ResourceSnapshot {
        app_pid: 100,
        app_memory_bytes: 50 * 1024 * 1024,
        app_cpu_time_ns: 2_000_000_000,
        total_tree_memory_bytes: 150 * 1024 * 1024,
        total_tree_cpu_time_ns: 5_000_000_000,
        total_tree_disk_read_bytes: 1024 * 1024,
        total_tree_disk_write_bytes: 512 * 1024,
        workspaces: Vec::new(),
        captured_at: Some(std::time::Instant::now()),
    };

    let ws = WorkspaceResource {
        workspace_id: "ws_1".to_string(),
        name: "Dev Workspace".to_string(),
        color: Some("#7AA2F7".to_string()),
        total_memory_bytes: 100 * 1024 * 1024,
        total_cpu_time_ns: 3_000_000_000,
        total_disk_read_bytes: 1024 * 1024,
        total_disk_write_bytes: 512 * 1024,
        tabs: vec![TabResource {
            tab_id: "tab_1".to_string(),
            title: "Editor".to_string(),
            total_memory_bytes: 100 * 1024 * 1024,
            total_cpu_time_ns: 3_000_000_000,
            total_disk_read_bytes: 1024 * 1024,
            total_disk_write_bytes: 512 * 1024,
            panes: vec![PaneResource {
                pane_id: "pane_1".to_string(),
                title: "Editor Pane".to_string(),
                root_pid: Some(101),
                total_memory_bytes: 100 * 1024 * 1024,
                total_cpu_time_ns: 3_000_000_000,
                total_disk_read_bytes: 1024 * 1024,
                total_disk_write_bytes: 512 * 1024,
                processes: Vec::new(),
            }],
        }],
    };

    snapshot.workspaces.push(ws);

    assert_eq!(snapshot.workspaces.len(), 1);
    assert_eq!(snapshot.workspaces[0].tabs.len(), 1);
    assert_eq!(snapshot.workspaces[0].tabs[0].panes.len(), 1);
    assert_eq!(snapshot.total_tree_memory_bytes, 150 * 1024 * 1024);
}
