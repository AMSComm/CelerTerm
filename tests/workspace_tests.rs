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

#[test]
fn test_format_tab_title_logic() {
    use celerterm::pty::format_tab_title;

    // 1. Non-shell foreground process overrides folder name
    assert_eq!(
        format_tab_title(Some("nvim"), Some(&PathBuf::from("/Users/test/my_project"))),
        "nvim"
    );
    assert_eq!(
        format_tab_title(Some("cargo"), Some(&PathBuf::from("/Users/test/my_project"))),
        "cargo"
    );
    assert_eq!(
        format_tab_title(Some("python3"), Some(&PathBuf::from("/Users/test/my_project"))),
        "python3"
    );

    // 2. Shell foreground process falls back to folder name
    assert_eq!(
        format_tab_title(Some("zsh"), Some(&PathBuf::from("/Users/test/my_project"))),
        "my_project"
    );
    assert_eq!(
        format_tab_title(Some("bash"), Some(&PathBuf::from("/Users/test/my_project"))),
        "my_project"
    );
    assert_eq!(
        format_tab_title(None, Some(&PathBuf::from("/Users/test/my_project"))),
        "my_project"
    );

    // 3. User home directory displays as ~
    if let Some(base_dirs) = directories::BaseDirs::new() {
        assert_eq!(
            format_tab_title(None, Some(base_dirs.home_dir())),
            "~"
        );
    }
}

#[test]
fn test_scrollback_export_and_restore_cycle() {
    use celerterm::term::TermScreen;

    // Setup an initial screen and write simulated terminal output
    let mut screen1 = TermScreen::new(80, 24);
    screen1.process_bytes(b"celerterm v0.1.0\r\n");
    screen1.process_bytes(b"huy@mac:~/dev/amktest/celerterm $ cargo check\r\n");
    screen1.process_bytes(b"Finished dev [unoptimized + debuginfo] target(s) in 0.5s\r\n");

    // Export scrollback lines
    let saved_lines = screen1.get_scrollback_lines(100);
    assert!(!saved_lines.is_empty());
    assert!(saved_lines.iter().any(|l| l.contains("celerterm v0.1.0")));
    assert!(saved_lines.iter().any(|l| l.contains("cargo check")));
    assert!(saved_lines.iter().any(|l| l.contains("Finished dev")));

    // Restore into a fresh screen
    let mut screen2 = TermScreen::new(80, 24);
    for line in &saved_lines {
        screen2.process_bytes(line.as_bytes());
        screen2.process_bytes(b"\r\n");
    }

    let restored_lines = screen2.get_scrollback_lines(100);
    assert!(restored_lines.iter().any(|l| l.contains("celerterm v0.1.0")));
    assert!(restored_lines.iter().any(|l| l.contains("cargo check")));
    assert!(restored_lines.iter().any(|l| l.contains("Finished dev")));
}

#[test]
fn test_scrollback_export_and_restore_colors_and_styles() {
    use celerterm::term::TermScreen;
    use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};

    let mut screen1 = TermScreen::new(80, 24);
    // Write styled line: Green "PASS", Bold Red "FAIL", Blue background "BLUE", 24-bit TrueColor "RGB"
    screen1.process_bytes(b"\x1b[32mPASS\x1b[0m \x1b[1;31mFAIL\x1b[0m \x1b[44mBLUE\x1b[0m \x1b[38;2;120;180;240mRGB\x1b[0m\r\n");

    let saved_lines = screen1.get_scrollback_lines(100);
    assert_eq!(saved_lines.len(), 1);
    // Ensure escape sequences were serialized in saved string
    assert!(saved_lines[0].contains("PASS"));
    assert!(saved_lines[0].contains("FAIL"));
    assert!(saved_lines[0].contains("BLUE"));
    assert!(saved_lines[0].contains("RGB"));

    // Restore into a fresh screen
    let mut screen2 = TermScreen::new(80, 24);
    for line in &saved_lines {
        screen2.process_bytes(line.as_bytes());
        screen2.process_bytes(b"\r\n");
    }

    // Check cell colors on restored screen2
    let (c0, fg0, _) = screen2.get_render_cell(0, 0); // 'P'
    assert_eq!(c0, 'P');
    assert_eq!(fg0, Color::Named(NamedColor::Green));

    let (c5, fg5, _) = screen2.get_render_cell(5, 0); // 'F' (bold red brightens to BrightRed)
    assert_eq!(c5, 'F');
    assert_eq!(fg5, Color::Named(NamedColor::BrightRed));

    let (c10, _, bg10) = screen2.get_render_cell(10, 0); // 'B'
    assert_eq!(c10, 'B');
    assert_eq!(bg10, Color::Named(NamedColor::Blue));

    let (c15, fg15, _) = screen2.get_render_cell(15, 0); // 'R'
    assert_eq!(c15, 'R');
    assert_eq!(fg15, Color::Spec(Rgb { r: 120, g: 180, b: 240 }));
}

#[test]
fn test_tab_scrollback_preservation_on_close_or_exit() {
    use celerterm::term::TermScreen;
    use tempfile::tempdir;

    let dir = tempdir().expect("Create tempdir");
    let file_path = dir.path().join("workspace_snapshot.json");

    let mut manager = WorkspaceManager::new();
    let tab1_id = manager.get_active_workspace().unwrap().active_tab_id.clone();
    let tab2_id = manager.new_tab(PathBuf::from("/Users/test/tab2")).unwrap();

    // Simulate tab1 having active scrollback output
    let mut screen1 = TermScreen::new(80, 24);
    screen1.process_bytes(b"huy@mac:~$ git status\r\nOn branch dev\r\nnothing to commit\r\n");
    let scrollback1 = screen1.get_scrollback_lines(100);

    // Save scrollback to tab1 in workspace manager
    for ws in &mut manager.workspaces {
        if let Some(tab) = ws.tabs.iter_mut().find(|t| t.id == tab1_id) {
            tab.scrollback_cache = scrollback1.clone();
        }
    }

    // Save to disk snapshot
    save_snapshot_to_file(&manager, &file_path).expect("Save snapshot");

    // Load from disk and verify tab1 has the preserved scrollback
    let restored = load_snapshot_from_file(&file_path).expect("Load snapshot");
    let restored_ws = restored.get_active_workspace().unwrap();
    assert_eq!(restored_ws.tabs.len(), 2);
    let restored_tab1 = restored_ws.tabs.iter().find(|t| t.id == tab1_id).unwrap();
    assert!(!restored_tab1.scrollback_cache.is_empty());
    assert!(restored_tab1.scrollback_cache.iter().any(|l| l.contains("On branch dev")));
    assert_eq!(restored_ws.tabs.iter().find(|t| t.id == tab2_id).unwrap().cwd, PathBuf::from("/Users/test/tab2"));
}

#[test]
fn test_pty_sink_forwards_dsr_cursor_report() {
    use celerterm::term::TermScreen;
    use parking_lot::Mutex;
    use std::sync::Arc;
    use std::io::Write;

    #[derive(Clone)]
    struct MockWriter {
        data: Arc<Mutex<Vec<u8>>>,
    }
    impl Write for MockWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.data.lock().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let shared_data = Arc::new(Mutex::new(Vec::new()));
    let mock = MockWriter { data: shared_data.clone() };

    let mut screen = TermScreen::new(80, 24);
    let writer = Arc::new(Mutex::new(Box::new(mock) as Box<dyn Write + Send>));
    screen.set_pty_writer(writer);

    // Print 3 lines of restored scrollback
    screen.process_bytes(b"Restored line 1\r\nRestored line 2\r\nRestored line 3\r\n");

    // Shell sends DSR \x1b[6n to query cursor position
    screen.process_bytes(b"\x1b[6n");

    // Verify PtySink wrote the cursor response \x1b[4;1R (cursor is at line 4, col 1)
    let data = shared_data.lock();
    let written = std::str::from_utf8(&data).unwrap_or("");
    assert!(written.contains("\x1b[4;1R"), "PtySink should reply with cursor position at line 4, col 1");
}

