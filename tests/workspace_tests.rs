use celerterm::workspace::manager::WorkspaceManager;
use celerterm::workspace::storage::{save_snapshot_to_string, load_snapshot_from_str, save_snapshot_to_file, load_snapshot_from_file};
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_workspace_creation_and_tab_management() {
    let mut manager = WorkspaceManager::new();

    // Default workspace should exist with 1 default tab
    assert_eq!(manager.workspaces.len(), 1);
    let active_ws = manager.get_active_workspace().expect("Active workspace should exist");
    assert_eq!(active_ws.tabs.len(), 1);

    // Add a new tab to current workspace
    let tab2_id = manager.new_tab(PathBuf::from("/Users/test/project")).expect("New tab created");
    let active_ws = manager.get_active_workspace().unwrap();
    assert_eq!(active_ws.tabs.len(), 2);
    assert_eq!(active_ws.active_tab_id, tab2_id);

    // Create another workspace
    let ws2_id = manager.new_workspace("Secondary").expect("New workspace created");
    assert_eq!(manager.workspaces.len(), 2);
    assert_eq!(manager.active_workspace_id, ws2_id);

    // Switch back to first workspace
    let ws1_id = manager.workspaces[0].id.clone();
    manager.switch_workspace(&ws1_id).expect("Switched workspace");
    assert_eq!(manager.active_workspace_id, ws1_id);
}

#[test]
fn test_workspace_snapshot_serialization_and_restore() {
    let mut manager = WorkspaceManager::new();
    
    // Setup workspace 1 with custom scrollback and tab
    let ws = manager.get_active_workspace_mut().unwrap();
    ws.name = "Primary Workspace".to_string();
    let tab = &mut ws.tabs[0];
    tab.title = "Neovim - core".to_string();
    tab.cwd = PathBuf::from("/Users/huy/dev/celerterm");
    tab.scrollback_cache = vec![
        "Line 1: cargo check".to_string(),
        "Line 2: Finished dev".to_string(),
    ];

    // Add tab 2
    let _ = manager.new_tab(PathBuf::from("/Users/huy/dev/amktest"));
    
    // Add workspace 2
    let _ = manager.new_workspace("Docs");

    // Serialize snapshot
    let snapshot_json = save_snapshot_to_string(&manager).expect("Serialization to string succeeds");
    assert!(!snapshot_json.is_empty());

    // Restore from snapshot
    let restored_manager = load_snapshot_from_str(&snapshot_json).expect("Deserialization succeeds");

    assert_eq!(restored_manager.workspaces.len(), 2);
    let primary = &restored_manager.workspaces[0];
    assert_eq!(primary.name, "Primary Workspace");
    assert_eq!(primary.tabs.len(), 2);
    assert_eq!(primary.tabs[0].title, "Neovim - core");
    assert_eq!(primary.tabs[0].cwd, PathBuf::from("/Users/huy/dev/celerterm"));
    assert_eq!(
        primary.tabs[0].scrollback_cache,
        vec![
            "Line 1: cargo check".to_string(),
            "Line 2: Finished dev".to_string(),
        ]
    );
}

#[test]
fn test_disk_snapshot_persistence() {
    let dir = tempdir().expect("Create tempdir");
    let file_path = dir.path().join("workspace_snapshot.json");

    let mut manager = WorkspaceManager::new();
    let _ = manager.new_tab(PathBuf::from("/tmp/test"));

    save_snapshot_to_file(&manager, &file_path).expect("Saved to disk");
    assert!(file_path.exists());

    let restored = load_snapshot_from_file(&file_path).expect("Loaded from disk");
    assert_eq!(restored.workspaces.len(), 1);
    assert_eq!(restored.workspaces[0].tabs.len(), 2);
}

#[test]
fn test_workspace_tab_switching_by_index_and_direction() {
    let mut manager = WorkspaceManager::new();
    let tab1_id = manager.get_active_workspace().unwrap().active_tab_id.clone();
    let tab2_id = manager.new_tab(PathBuf::from("/tmp/tab2")).unwrap();
    let tab3_id = manager.new_tab(PathBuf::from("/tmp/tab3")).unwrap();

    let ws = manager.get_active_workspace().unwrap();
    assert_eq!(ws.tabs.len(), 3);
    assert_eq!(ws.active_tab_id, tab3_id);

    // Switch to tab 1 (1-indexed: index 1 -> tab1_id)
    manager.select_tab_by_1_index(1).expect("Select tab 1");
    assert_eq!(manager.get_active_workspace().unwrap().active_tab_id, tab1_id);

    // Switch to tab 2
    manager.select_tab_by_1_index(2).expect("Select tab 2");
    assert_eq!(manager.get_active_workspace().unwrap().active_tab_id, tab2_id);

    // Switch to tab 3
    manager.select_tab_by_1_index(3).expect("Select tab 3");
    assert_eq!(manager.get_active_workspace().unwrap().active_tab_id, tab3_id);

    // Out of bounds (e.g. 5) returns error or no-op
    assert!(manager.select_tab_by_1_index(5).is_err());

    // Next tab wraps around from 3 to 1
    manager.select_next_tab().expect("Next tab");
    assert_eq!(manager.get_active_workspace().unwrap().active_tab_id, tab1_id);

    // Previous tab wraps around from 1 to 3
    manager.select_previous_tab().expect("Previous tab");
    assert_eq!(manager.get_active_workspace().unwrap().active_tab_id, tab3_id);

    // Previous tab from 3 to 2
    manager.select_previous_tab().expect("Previous tab");
    assert_eq!(manager.get_active_workspace().unwrap().active_tab_id, tab2_id);
}
