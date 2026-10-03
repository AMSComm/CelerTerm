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
fn test_workspace_tab_move_left_and_right() {
    let mut manager = WorkspaceManager::new();
    let tab1_id = manager.get_active_workspace().unwrap().active_tab_id.clone();
    let tab2_id = manager.new_tab(PathBuf::from("/tmp/tab2")).unwrap();
    let tab3_id = manager.new_tab(PathBuf::from("/tmp/tab3")).unwrap();

    // Initial order: [tab1, tab2, tab3], active is tab3 (index 2)
    assert_eq!(manager.get_active_workspace().unwrap().tabs[0].id, tab1_id);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[1].id, tab2_id);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[2].id, tab3_id);
    assert_eq!(manager.get_active_workspace().unwrap().active_tab_id, tab3_id);

    // 1. Move tab3 left (from index 2 to index 1): order becomes [tab1, tab3, tab2]
    let pos = manager.move_active_tab_left().expect("Move tab3 left");
    assert_eq!(pos, 1);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[0].id, tab1_id);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[1].id, tab3_id);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[2].id, tab2_id);
    assert_eq!(manager.get_active_workspace().unwrap().active_tab_id, tab3_id);

    // 2. Move tab3 left again (from index 1 to index 0): order becomes [tab3, tab1, tab2]
    let pos = manager.move_active_tab_left().expect("Move tab3 left to 0");
    assert_eq!(pos, 0);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[0].id, tab3_id);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[1].id, tab1_id);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[2].id, tab2_id);
    assert_eq!(manager.get_active_workspace().unwrap().active_tab_id, tab3_id);

    // 3. Move tab3 left at index 0: should clamp at 0 and keep order [tab3, tab1, tab2]
    let pos = manager.move_active_tab_left().expect("Move tab3 left clamped");
    assert_eq!(pos, 0);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[0].id, tab3_id);

    // 4. Move tab3 right (from index 0 to index 1): order becomes [tab1, tab3, tab2]
    let pos = manager.move_active_tab_right().expect("Move tab3 right to 1");
    assert_eq!(pos, 1);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[0].id, tab1_id);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[1].id, tab3_id);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[2].id, tab2_id);

    // 5. Move tab3 right (from index 1 to index 2): order becomes [tab1, tab2, tab3]
    let pos = manager.move_active_tab_right().expect("Move tab3 right to 2");
    assert_eq!(pos, 2);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[0].id, tab1_id);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[1].id, tab2_id);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[2].id, tab3_id);

    // 6. Move tab3 right at index 2: should clamp at last index and keep order [tab1, tab2, tab3]
    let pos = manager.move_active_tab_right().expect("Move tab3 right clamped");
    assert_eq!(pos, 2);
    assert_eq!(manager.get_active_workspace().unwrap().tabs[2].id, tab3_id);

    // 7. Single-tab workspace handles move gracefully
    let mut single_mgr = WorkspaceManager::new();
    assert_eq!(single_mgr.move_active_tab_left().unwrap(), 0);
    assert_eq!(single_mgr.move_active_tab_right().unwrap(), 0);
}

#[test]
fn test_format_tab_title_logic() {
    use celerterm::pty::format_tab_title;

    // 1. Non-shell foreground process overrides folder name
    assert_eq!(
        format_tab_title(Some("nvim"), Some(&PathBuf::from("/Users/test/my_project")), None),
        "nvim"
    );
    assert_eq!(
        format_tab_title(Some("cargo"), Some(&PathBuf::from("/Users/test/my_project")), None),
        "cargo"
    );
    assert_eq!(
        format_tab_title(Some("python3"), Some(&PathBuf::from("/Users/test/my_project")), None),
        "python3"
    );

    // 2. Shell foreground process falls back to folder name
    assert_eq!(
        format_tab_title(Some("zsh"), Some(&PathBuf::from("/Users/test/my_project")), None),
        "my_project"
    );
    assert_eq!(
        format_tab_title(Some("bash"), Some(&PathBuf::from("/Users/test/my_project")), None),
        "my_project"
    );
    assert_eq!(
        format_tab_title(None, Some(&PathBuf::from("/Users/test/my_project")), None),
        "my_project"
    );

    // 3. User home directory displays as ~
    if let Some(base_dirs) = directories::BaseDirs::new() {
        assert_eq!(
            format_tab_title(None, Some(base_dirs.home_dir()), None),
            "~"
        );
    }

    // 4. SSH session formatting with remote process detection
    assert_eq!(
        format_tab_title(Some("ssh"), Some(&PathBuf::from("/Users/test")), None),
        "[🌐ssh]"
    );
    assert_eq!(
        format_tab_title(Some("ssh"), Some(&PathBuf::from("/Users/test")), Some("tail")),
        "[🌐tail]"
    );
    assert_eq!(
        format_tab_title(Some("ssh"), Some(&PathBuf::from("/Users/test")), Some("user@server: tail -f /var/log/syslog")),
        "[🌐tail]"
    );
    assert_eq!(
        format_tab_title(Some("ssh"), Some(&PathBuf::from("/Users/test")), Some("user@server: ~/dev - nvim")),
        "[🌐nvim]"
    );
    assert_eq!(
        format_tab_title(Some("ssh"), Some(&PathBuf::from("/Users/test")), Some("user@server: ~")),
        "[🌐~]"
    );

    // 5. Script and wrapper command formatting
    assert_eq!(
        format_tab_title(Some("psql"), Some(&PathBuf::from("/Users/test")), None),
        "psql"
    );
    assert_eq!(
        format_tab_title(Some("psql9"), Some(&PathBuf::from("/Users/test")), None),
        "psql9"
    );
    assert_eq!(
        format_tab_title(Some("pa2Hbia"), Some(&PathBuf::from("/Users/test")), None),
        "pa2Hbia"
    );
    assert_eq!(
        format_tab_title(Some("gradlew"), Some(&PathBuf::from("/Users/test")), None),
        "gradlew"
    );
    assert_eq!(
        format_tab_title(Some("docker"), Some(&PathBuf::from("/Users/test")), None),
        "docker"
    );

    // 6. Max 20 characters limitation & truncation
    assert_eq!(
        format_tab_title(None, Some(&PathBuf::from("/Users/test/12345678901234567890")), None),
        "12345678901234567890"
    );
    assert_eq!(
        format_tab_title(None, Some(&PathBuf::from("/Users/test/my_very_long_project_directory")), None),
        "my_very_long_projec…"
    );
    assert_eq!(
        format_tab_title(None, Some(&PathBuf::from("/Users/test/my_very_long_project_directory")), None).chars().count(),
        20
    );
}

#[test]
fn test_tab_title_truncation_20_chars() {
    use celerterm::pty::truncate_tab_title;

    // Under limit
    assert_eq!(truncate_tab_title("short", 20), "short");
    // Exactly 20 chars
    assert_eq!(truncate_tab_title("12345678901234567890", 20), "12345678901234567890");
    // Over limit: 21 chars -> 19 chars + ellipsis
    assert_eq!(truncate_tab_title("123456789012345678901", 20), "1234567890123456789…");
    assert_eq!(truncate_tab_title("123456789012345678901", 20).chars().count(), 20);

    // Multi-byte or long strings
    let long_title = "Internationalized project directory";
    let truncated_title = truncate_tab_title(long_title, 20);
    assert_eq!(truncated_title.chars().count(), 20);
    assert!(truncated_title.ends_with('…'));
}

#[test]
fn test_shell_script_and_wrapper_title_extraction() {
    use celerterm::pty::{extract_command_from_shell_args, is_generic_runner, is_shell_name};

    // Shell detection
    assert!(is_shell_name("sh"));
    assert!(is_shell_name("bash"));
    assert!(is_shell_name("zsh"));
    assert!(is_shell_name("fish"));
    assert!(!is_shell_name("psql"));
    assert!(!is_shell_name("psql9"));
    assert!(!is_shell_name("docker"));
    assert!(!is_shell_name("cargo"));

    // Generic runner detection
    assert!(is_generic_runner("docker"));
    assert!(is_generic_runner("podman"));
    assert!(is_generic_runner("sudo"));
    assert!(is_generic_runner("env"));
    assert!(!is_generic_runner("psql"));
    assert!(!is_generic_runner("cargo"));

    // Script wrapper extraction: shebang execution of psql / psql9 / pa2Hbia
    let psql_args = vec!["/bin/sh".to_string(), "/Users/huy/app/bin/psql".to_string()];
    assert_eq!(extract_command_from_shell_args(&psql_args), Some("psql".to_string()));

    let psql9_args = vec![
        "/bin/sh".to_string(),
        "/Users/huy/app/bin/psql9".to_string(),
        "host=172.1.0.141".to_string(),
    ];
    assert_eq!(extract_command_from_shell_args(&psql9_args), Some("psql9".to_string()));

    let pa2hbia_args = vec!["/bin/sh".to_string(), "/Users/huy/app/bin/pa2Hbia".to_string()];
    assert_eq!(extract_command_from_shell_args(&pa2hbia_args), Some("pa2Hbia".to_string()));

    // Script with flags before path
    let flag_args = vec![
        "/bin/sh".to_string(),
        "-e".to_string(),
        "-x".to_string(),
        "/opt/tools/gradlew".to_string(),
        "build".to_string(),
    ];
    assert_eq!(extract_command_from_shell_args(&flag_args), Some("gradlew".to_string()));

    // -c command string extraction
    let c_args = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "docker run --rm -ti psql9 psql".to_string(),
    ];
    assert_eq!(extract_command_from_shell_args(&c_args), Some("docker".to_string()));

    let c_psql_args = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "psql -h localhost -d test".to_string(),
    ];
    assert_eq!(extract_command_from_shell_args(&c_psql_args), Some("psql".to_string()));

    // Idle shell with no script
    let idle_login = vec!["/bin/zsh".to_string(), "-l".to_string()];
    assert_eq!(extract_command_from_shell_args(&idle_login), None);

    let idle_bash = vec!["/bin/bash".to_string()];
    assert_eq!(extract_command_from_shell_args(&idle_bash), None);

    // Live process resolution test
    use celerterm::pty::get_process_name;
    let my_name = get_process_name(std::process::id());
    assert!(my_name.is_some());
    assert!(!my_name.unwrap().is_empty());
}

#[test]
fn test_script_wrapper_live_process_name() {
    use celerterm::pty::get_process_name;
    use std::process::Command;

    // Create a temporary shell script named "psql_dummy_test"
    let tmp_dir = tempdir().expect("tempdir succeeds");
    let script_path = tmp_dir.path().join("psql_dummy_test");
    std::fs::write(&script_path, "#!/bin/sh\nsleep 3\n").expect("write script succeeds");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }

    let mut child = Command::new(&script_path)
        .spawn()
        .expect("spawn script succeeds");

    let pid = child.id();
    let detected_name = get_process_name(pid);
    let _ = child.kill();
    let _ = child.wait();

    assert_eq!(detected_name, Some("psql_dummy_test".to_string()));
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

#[test]
fn test_workspace_delete_and_rename() {
    let mut manager = WorkspaceManager::new();
    let default_id = manager.active_workspace_id.clone();

    // 1. Rename existing default workspace
    manager.rename_workspace(&default_id, "Dev Space").expect("Rename succeeded");
    assert_eq!(manager.get_active_workspace().unwrap().name, "Dev Space");

    // Renaming with empty name should fail
    assert!(manager.rename_workspace(&default_id, "   ").is_err());

    // 2. Add second and third workspaces
    let ws2_id = manager.new_workspace("Project 2").unwrap();
    let ws3_id = manager.new_workspace("Project 3").unwrap();
    assert_eq!(manager.workspaces.len(), 3);
    assert_eq!(manager.active_workspace_id, ws3_id);

    // 3. Rename ws2
    manager.rename_workspace(&ws2_id, "Backend API").unwrap();
    assert_eq!(manager.workspaces.iter().find(|w| w.id == ws2_id).unwrap().name, "Backend API");

    // 4. Delete non-active workspace (ws2)
    manager.delete_workspace(&ws2_id).expect("Delete ws2 succeeded");
    assert_eq!(manager.workspaces.len(), 2);
    assert!(!manager.workspaces.iter().any(|w| w.id == ws2_id));
    assert_eq!(manager.active_workspace_id, ws3_id);

    // 5. Delete currently active workspace (ws3)
    let new_active = manager.delete_workspace(&ws3_id).expect("Delete ws3 succeeded");
    assert_eq!(manager.workspaces.len(), 1);
    assert_eq!(new_active, default_id);
    assert_eq!(manager.active_workspace_id, default_id);

    // 6. Attempting to delete the last remaining workspace must fail
    assert!(manager.delete_workspace(&default_id).is_err());
    assert_eq!(manager.workspaces.len(), 1);
}

#[test]
fn test_merge_workspace_managers_preserves_unowned_workspaces() {
    use celerterm::workspace::storage::merge_workspace_managers;

    // Simulate Window 1 (Main Window)
    let mut win1 = WorkspaceManager::new();
    win1.workspaces[0].name = "Term".to_string();
    let tab1_id = win1.workspaces[0].tabs[0].id.clone();
    let tab2_id = win1.new_tab(PathBuf::from("/Users/huy/work")).unwrap();

    // Window 1 creates Workspace 2 (Agent) initially with 1 default tab
    let ws2_id = win1.new_workspace("Agent").unwrap();
    let win1_ws2_tab_id = win1.workspaces.iter().find(|w| w.id == ws2_id).unwrap().tabs[0].id.clone();
    // Switch Window 1 back to Term
    win1.switch_workspace("ws_default").unwrap();

    // Simulate Window 2 (opened via --workspace Agent)
    let mut win2 = win1.clone();
    win2.switch_workspace(&ws2_id).unwrap();

    // In Window 2, user works in Agent: modifies tab, changes cwd, adds another tab
    if let Some(ws2) = win2.workspaces.iter_mut().find(|w| w.id == ws2_id) {
        ws2.tabs[0].cwd = PathBuf::from("/Users/huy/dev/amktest");
        ws2.tabs[0].title = "amktest".to_string();
        ws2.tabs[0].scrollback_cache = vec!["cd dev/amktest".to_string()];
    }
    let win2_new_tab = win2.new_tab(PathBuf::from("/Users/huy/dev/a2")).unwrap();

    // Window 2 saves to disk first
    let win2_owned_tabs = vec![win1_ws2_tab_id.clone(), win2_new_tab.clone()];
    let disk_snapshot = merge_workspace_managers(
        &win2,
        &win2_owned_tabs,
        &[],
        None,
        true,
    );

    // Verify disk snapshot has updated Agent workspace (2 tabs)
    let saved_ws2 = disk_snapshot.workspaces.iter().find(|w| w.id == ws2_id).unwrap();
    assert_eq!(saved_ws2.tabs.len(), 2);
    assert_eq!(saved_ws2.tabs[0].cwd, PathBuf::from("/Users/huy/dev/amktest"));

    // Now Window 1 closes LATER. Window 1 only owns tab1 and tab2 from Term.
    // Window 1's memory of Agent is still the stale 1-tab version.
    let win1_owned_tabs = vec![tab1_id.clone(), tab2_id.clone()];
    let final_merged = merge_workspace_managers(
        &win1,
        &win1_owned_tabs,
        &[],
        Some(&disk_snapshot),
        false,
    );

    // CRITICAL ASSERTION: Window 1 must NOT overwrite Window 2's Agent workspace!
    assert_eq!(final_merged.workspaces.len(), 2);

    let final_term = final_merged.workspaces.iter().find(|w| w.id == "ws_default").unwrap();
    assert_eq!(final_term.tabs.len(), 2, "Window 1's Term workspace must be fully preserved");

    let final_agent = final_merged.workspaces.iter().find(|w| w.id == ws2_id).unwrap();
    assert_eq!(final_agent.tabs.len(), 2, "Window 2's Agent workspace must NOT be overwritten by Window 1's stale memory");
    assert_eq!(final_agent.tabs[0].cwd, PathBuf::from("/Users/huy/dev/amktest"));
    assert_eq!(final_agent.tabs[0].title, "amktest");
    assert_eq!(final_agent.tabs[0].scrollback_cache, vec!["cd dev/amktest".to_string()]);
    assert_eq!(final_agent.tabs[1].cwd, PathBuf::from("/Users/huy/dev/a2"));

    // Main window was active on Term, so active_workspace_id should be ws_default
    assert_eq!(final_merged.active_workspace_id, "ws_default");
}

#[test]
fn test_merge_workspace_managers_respects_deletions_and_adds_external() {
    use celerterm::workspace::storage::merge_workspace_managers;

    let mut disk_mgr = WorkspaceManager::new();
    let ws2_id = disk_mgr.new_workspace("Obsolete").unwrap();
    let ws3_id = disk_mgr.new_workspace("External From Win2").unwrap();

    let current_mgr = WorkspaceManager::new();
    // current_mgr deleted ws2
    let deleted_ids = vec![ws2_id.clone()];

    let merged = merge_workspace_managers(
        &current_mgr,
        &["tab_1".to_string()],
        &deleted_ids,
        Some(&disk_mgr),
        false,
    );

    // Obsolete workspace should NOT be in merged
    assert!(!merged.workspaces.iter().any(|w| w.id == ws2_id));
    // External workspace from disk should be preserved
    assert!(merged.workspaces.iter().any(|w| w.id == ws3_id));
}

#[test]
fn test_duplicate_instance_detection_and_resolution() {
    use celerterm::workspace::instances::{ActiveInstance, find_other_instance_in};

    let instances = vec![
        ActiveInstance {
            pid: 1001,
            workspace_name: "Dev".to_string(),
            workspace_id: "ws_dev".to_string(),
            updated_at: 100,
        },
        ActiveInstance {
            pid: 1002,
            workspace_name: "Ops".to_string(),
            workspace_id: "ws_ops".to_string(),
            updated_at: 200,
        },
    ];

    let my_pid = 1001;

    // Searching for my own workspace (Dev) should NOT find an "other" instance
    assert!(find_other_instance_in(&instances, my_pid, "ws_dev", "Dev").is_none());

    // Searching for Ops should find PID 1002
    let ops_other = find_other_instance_in(&instances, my_pid, "ws_ops", "Ops");
    assert!(ops_other.is_some());
    assert_eq!(ops_other.unwrap().pid, 1002);

    // From a 3rd window (PID 1003): both Dev and Ops are detected as occupied
    let other_dev = find_other_instance_in(&instances, 1003, "ws_dev", "Dev");
    assert_eq!(other_dev.unwrap().pid, 1001);

    let other_ops = find_other_instance_in(&instances, 1003, "ws_ops", "Ops");
    assert_eq!(other_ops.unwrap().pid, 1002);
}

#[test]
fn test_unoccupied_workspace_fallback_selection() {
    use celerterm::workspace::instances::{ActiveInstance, find_other_instance_in};

    let mut mgr = WorkspaceManager::new();
    let ws1_id = mgr.active_workspace_id.clone();
    let ws1_name = mgr.get_active_workspace().unwrap().name.clone();

    let ws2_id = mgr.new_workspace("Project B").unwrap();
    let ws3_id = mgr.new_workspace("Project C").unwrap();

    // Suppose ws1 is active in another instance (PID 5000)
    let instances = vec![
        ActiveInstance {
            pid: 5000,
            workspace_name: ws1_name,
            workspace_id: ws1_id.clone(),
            updated_at: 100,
        }
    ];

    let my_pid = 6000;
    // Window 2 starts up, ws1 is occupied. Find first unowned workspace:
    let available_id = mgr.workspaces.iter().find(|w| {
        find_other_instance_in(&instances, my_pid, &w.id, &w.name).is_none()
    }).map(|w| w.id.clone());

    assert_eq!(available_id, Some(ws2_id.clone()));

    // Now suppose both ws1 and ws2 are occupied:
    let instances_all = vec![
        ActiveInstance {
            pid: 5000,
            workspace_name: "Default".to_string(),
            workspace_id: ws1_id,
            updated_at: 100,
        },
        ActiveInstance {
            pid: 5001,
            workspace_name: "Project B".to_string(),
            workspace_id: ws2_id,
            updated_at: 110,
        },
    ];

    let next_avail = mgr.workspaces.iter().find(|w| {
        find_other_instance_in(&instances_all, my_pid, &w.id, &w.name).is_none()
    }).map(|w| w.id.clone());

    assert_eq!(next_avail, Some(ws3_id));
}

#[test]
fn test_multi_window_workspace_deletion_sync() {
    use celerterm::workspace::storage::merge_workspace_managers;

    // Both Window 1 and Window 2 initially have 3 workspaces:
    // "Term" (id: ws_default), "Agent" (id: ws_agent), "Testing" (id: ws_test)
    let mut initial_mgr = WorkspaceManager::new();
    initial_mgr.workspaces[0].name = "Term".to_string();
    let term_tab_id = initial_mgr.workspaces[0].tabs[0].id.clone();
    let ws_agent_id = initial_mgr.new_workspace("Agent").unwrap();
    let agent_tab_id = initial_mgr.workspaces.iter().find(|w| w.id == ws_agent_id).unwrap().tabs[0].id.clone();
    let ws_test_id = initial_mgr.new_workspace("Testing").unwrap();

    // Window 1 is active on "Term", owns term_tab_id
    let mut win1 = initial_mgr.clone();
    win1.switch_workspace("ws_default").unwrap();

    // Window 2 is active on "Agent", owns agent_tab_id
    let mut win2 = initial_mgr.clone();
    win2.switch_workspace(&ws_agent_id).unwrap();

    // On disk, all 3 exist
    let disk_snapshot = initial_mgr.clone();

    // Window 1 deletes "Testing"
    win1.delete_workspace(&ws_test_id).unwrap();
    let win1_deleted = vec![ws_test_id.clone()];
    let win1_saved = merge_workspace_managers(
        &win1,
        &[term_tab_id.clone()],
        &win1_deleted,
        Some(&disk_snapshot),
        false,
    );

    // Verify Window 1's saved disk snapshot only contains "Term" and "Agent"
    assert_eq!(win1_saved.workspaces.len(), 2);
    assert!(!win1_saved.workspaces.iter().any(|w| w.id == ws_test_id));
    assert!(win1_saved.workspaces.iter().any(|w| w.id == "ws_default"));
    assert!(win1_saved.workspaces.iter().any(|w| w.id == ws_agent_id));

    // Now Window 2 syncs / reloads from disk (win1_saved):
    // Window 2's memory still had "Testing", but Window 2 does NOT own "Testing",
    // and "Testing" is absent from disk.
    let win2_reloaded = merge_workspace_managers(
        &win2,
        &[agent_tab_id.clone()],
        &[], // Window 2 didn't delete it itself
        Some(&win1_saved),
        true,
    );

    // CRITICAL: Window 2 MUST drop "Testing" from its workspace list!
    assert_eq!(win2_reloaded.workspaces.len(), 2, "Window 2 must drop the workspace deleted by Window 1");
    assert!(!win2_reloaded.workspaces.iter().any(|w| w.id == ws_test_id));
    assert!(win2_reloaded.workspaces.iter().any(|w| w.id == ws_agent_id));
    assert!(win2_reloaded.workspaces.iter().any(|w| w.id == "ws_default"));

    // When Window 2 later saves its state, it must NOT resurrect "Testing" back to disk!
    let win2_saved = merge_workspace_managers(
        &win2_reloaded,
        &[agent_tab_id],
        &[],
        Some(&win1_saved),
        true,
    );
    assert_eq!(win2_saved.workspaces.len(), 2);
    assert!(!win2_saved.workspaces.iter().any(|w| w.id == ws_test_id));
}

#[test]
fn test_workspace_distinct_color_assignment() {
    let mut manager = WorkspaceManager::new();

    // Default workspace has first palette color
    let default_ws = manager.get_active_workspace().unwrap();
    assert!(default_ws.color.is_some());
    assert_eq!(default_ws.color.as_deref(), Some("#7aa2f7"));

    // Create 19 more workspaces - each must have a distinct color from the 20 Tokyo Night palette
    let mut created_ids = Vec::new();
    for i in 2..=20 {
        let id = manager.new_workspace(&format!("Workspace {}", i)).unwrap();
        created_ids.push(id);
    }

    assert_eq!(manager.workspaces.len(), 20);

    // Verify all 20 colors are unique
    let mut colors_set = std::collections::HashSet::new();
    for ws in &manager.workspaces {
        let color = ws.color.as_ref().expect("Each workspace must have an assigned color");
        assert!(colors_set.insert(color.to_lowercase()), "Color {} was assigned more than once!", color);
    }
    assert_eq!(colors_set.len(), 20);
}

#[test]
fn test_workspace_custom_color_and_background_persistence() {
    let mut manager = WorkspaceManager::new();
    let ws_id = manager.workspaces[0].id.clone();

    // Set custom accent color and background
    manager.set_workspace_color(&ws_id, Some("#ff79c6".to_string())).unwrap();
    manager.set_workspace_background(&ws_id, Some("#16161e".to_string())).unwrap();

    let ws = manager.get_active_workspace().unwrap();
    assert_eq!(ws.color.as_deref(), Some("#ff79c6"));
    assert_eq!(ws.background.as_deref(), Some("#16161e"));
    assert_eq!(ws.effective_color_u32(0), 0xff79c6);
    assert_eq!(ws.effective_background_u32(0x1a1b26), 0x16161e);

    // Serialize snapshot to json
    let json = save_snapshot_to_string(&manager).expect("Serialization succeeds");
    assert!(json.contains("#ff79c6"));
    assert!(json.contains("#16161e"));

    // Deserialize and check equality
    let restored = load_snapshot_from_str(&json).expect("Deserialization succeeds");
    let restored_ws = restored.get_active_workspace().unwrap();
    assert_eq!(restored_ws.color.as_deref(), Some("#ff79c6"));
    assert_eq!(restored_ws.background.as_deref(), Some("#16161e"));
}

#[test]
fn test_workspace_legacy_snapshot_backwards_compatibility() {
    // Legacy snapshot without color or background fields
    let legacy_json = r#"{
        "workspaces": [
            {
                "id": "ws_legacy",
                "name": "Legacy",
                "tabs": [
                    {
                        "id": "tab_1",
                        "title": "Shell",
                        "cwd": "/tmp",
                        "scrollback_cache": []
                    }
                ],
                "active_tab_id": "tab_1"
            }
        ],
        "active_workspace_id": "ws_legacy",
        "next_id": 2
    }"#;

    let mut restored = load_snapshot_from_str(legacy_json).expect("Legacy deserialization succeeds");
    assert_eq!(restored.workspaces.len(), 1);
    let ws = &restored.workspaces[0];
    assert!(ws.color.is_none());
    assert!(ws.background.is_none());

    // When ensure_distinct_colors is called, it populates missing color
    restored.ensure_distinct_colors();
    assert!(restored.workspaces[0].color.is_some());
}

#[test]
fn test_tab_color_customization_and_persistence() {
    let mut manager = WorkspaceManager::new();
    let ws = manager.get_active_workspace().unwrap();
    let tab1_id = ws.tabs[0].id.clone();

    // Default tab has no color override
    assert_eq!(ws.tabs[0].color, None);
    assert_eq!(ws.tabs[0].effective_color_u32(0x007AA2F7), 0x007AA2F7);

    // Set custom color for tab 1
    manager.set_tab_color(&tab1_id, Some("#bb9af7".to_string())).expect("Set tab color succeeds");
    let active_ws = manager.get_active_workspace().unwrap();
    assert_eq!(active_ws.tabs[0].color.as_deref(), Some("#bb9af7"));
    assert_eq!(active_ws.tabs[0].effective_color_u32(0x007AA2F7), 0x00BB9AF7);

    // Add a second tab without override
    let _tab2_id = manager.new_tab(PathBuf::from("/tmp")).unwrap();
    let active_ws = manager.get_active_workspace().unwrap();
    assert_eq!(active_ws.tabs[1].color, None);
    assert_eq!(active_ws.tabs[1].effective_color_u32(0x002AC3DE), 0x002AC3DE);

    // Error on non-existent tab
    assert!(manager.set_tab_color("non_existent_tab", Some("#ff007f".to_string())).is_err());

    // Serialize and deserialize snapshot
    let json = save_snapshot_to_string(&manager).expect("Serialize to json succeeds");
    assert!(json.contains("#bb9af7"));

    let restored = load_snapshot_from_str(&json).expect("Deserialize succeeds");
    let r_ws = restored.get_active_workspace().unwrap();
    assert_eq!(r_ws.tabs[0].color.as_deref(), Some("#bb9af7"));
    assert_eq!(r_ws.tabs[1].color, None);

    // Reset tab 1 color to None (inherits workspace)
    manager.set_tab_color(&tab1_id, None).expect("Reset tab color succeeds");
    let active_ws = manager.get_active_workspace().unwrap();
    assert_eq!(active_ws.tabs[0].color, None);
}

#[test]
fn test_split_panel_and_pane_navigation() {
    let mut manager = WorkspaceManager::new();
    let ws = manager.get_active_workspace().unwrap();
    let tab1_id = ws.tabs[0].id.clone();

    // Initial state: 1 pane (which is the tab id)
    assert_eq!(ws.tabs[0].all_pane_ids(), vec![tab1_id.clone()]);
    assert_eq!(ws.tabs[0].active_pane_id(), tab1_id);

    // Split vertical: pane 1 -> pane 1 (left) + pane 2 (right)
    let pane2_id = manager.split_active_pane(
        celerterm::workspace::SplitDirection::Vertical,
        "pane_2",
        "Shell 2",
        PathBuf::from("/tmp/p2"),
    ).expect("Split active pane succeeds");

    assert_eq!(pane2_id, "pane_2");
    let ws = manager.get_active_workspace().unwrap();
    assert_eq!(ws.tabs[0].active_pane_id(), "pane_2");
    assert_eq!(ws.tabs[0].all_pane_ids(), vec![tab1_id.clone(), "pane_2".to_string()]);

    // Split horizontal on pane 2: pane 2 -> pane 2 (top) + pane 3 (bottom)
    let pane3_id = manager.split_active_pane(
        celerterm::workspace::SplitDirection::Horizontal,
        "pane_3",
        "Shell 3",
        PathBuf::from("/tmp/p3"),
    ).expect("Split horizontal succeeds");

    assert_eq!(pane3_id, "pane_3");
    let ws = manager.get_active_workspace().unwrap();
    assert_eq!(ws.tabs[0].active_pane_id(), "pane_3");
    assert_eq!(ws.tabs[0].all_pane_ids(), vec![tab1_id.clone(), "pane_2".to_string(), "pane_3".to_string()]);

    // Navigation: next_pane cycles through panes
    assert!(manager.next_pane());
    let ws = manager.get_active_workspace().unwrap();
    assert_eq!(ws.tabs[0].active_pane_id(), tab1_id); // wrapped to first

    assert!(manager.next_pane());
    let ws = manager.get_active_workspace().unwrap();
    assert_eq!(ws.tabs[0].active_pane_id(), "pane_2");

    // previous_pane
    assert!(manager.previous_pane());
    let ws = manager.get_active_workspace().unwrap();
    assert_eq!(ws.tabs[0].active_pane_id(), tab1_id);

    // Direct focus
    assert!(manager.focus_pane("pane_3"));
    let ws = manager.get_active_workspace().unwrap();
    assert_eq!(ws.tabs[0].active_pane_id(), "pane_3");
}

#[test]
fn test_split_panel_close_pane_and_zoom() {
    let mut manager = WorkspaceManager::new();
    let ws = manager.get_active_workspace().unwrap();
    let tab1_id = ws.tabs[0].id.clone();

    // Splitting
    let _ = manager.split_active_pane(
        celerterm::workspace::SplitDirection::Vertical,
        "pane_2",
        "Shell 2",
        PathBuf::from("/tmp/p2"),
    );

    // Toggle zoom
    assert!(!manager.get_active_workspace().unwrap().tabs[0].is_zoomed);
    assert!(manager.toggle_zoom_active_pane());
    assert!(manager.get_active_workspace().unwrap().tabs[0].is_zoomed);
    assert!(!manager.toggle_zoom_active_pane());
    assert!(!manager.get_active_workspace().unwrap().tabs[0].is_zoomed);

    // Close active pane (pane_2): should return Some("pane_2") and focus tab1_id
    let closed = manager.close_active_pane().expect("Close pane succeeds");
    assert_eq!(closed, Some("pane_2".to_string()));

    let ws = manager.get_active_workspace().unwrap();
    assert_eq!(ws.tabs[0].active_pane_id(), tab1_id);
    assert_eq!(ws.tabs[0].all_pane_ids(), vec![tab1_id.clone()]);

    // When only 1 pane is left, close_active_pane returns Ok(None) so tab can be closed
    let closed_last = manager.close_active_pane().expect("Closing last pane returns None");
    assert_eq!(closed_last, None);
}

#[test]
fn test_split_panel_snapshot_serialization_and_restore() {
    let mut manager = WorkspaceManager::new();
    let _ = manager.split_active_pane(
        celerterm::workspace::SplitDirection::Vertical,
        "pane_b",
        "Build Pane",
        PathBuf::from("/tmp/build"),
    );

    // Set scrollback cache in pane_b
    let ws = manager.get_active_workspace_mut().unwrap();
    let tab = &mut ws.tabs[0];
    let tree = tab.ensure_pane_tree();
    if let Some(p) = tree.find_pane_mut("pane_b") {
        p.scrollback_cache.push("Build successful in 0.4s".to_string());
    }

    let json = save_snapshot_to_string(&manager).expect("Serialization succeeds");
    assert!(json.contains("pane_b"));
    assert!(json.contains("Build successful in 0.4s"));

    // Restore from json
    let restored = load_snapshot_from_str(&json).expect("Deserialization succeeds");
    let r_tab = &restored.workspaces[0].tabs[0];
    assert_eq!(r_tab.active_pane_id(), "pane_b");
    assert_eq!(r_tab.all_pane_ids().len(), 2);
    let tree = r_tab.pane_tree();
    let pane_b = tree.find_pane("pane_b").expect("pane_b exists in restored tree");
    assert_eq!(pane_b.scrollback_cache, vec!["Build successful in 0.4s".to_string()]);
}

#[test]
fn test_pane_inner_padding_and_divider_layout() {
    let mut manager = WorkspaceManager::new();
    let tab1_id = manager.get_active_workspace().unwrap().tabs[0].id.clone();
    let _ = manager.split_active_pane(
        celerterm::workspace::SplitDirection::Vertical,
        "pane_right",
        "Right Shell",
        PathBuf::from("/tmp/right"),
    ).expect("Split succeeds");

    let ws = manager.get_active_workspace().unwrap();
    let tab = &ws.tabs[0];
    let tree = tab.pane_tree();

    let cell_w = 10.0;
    let cell_h = 20.0;
    let (mut panes, dividers) = tree.calculate_layout(0.0, 0.0, 1001.0, 600.0, cell_w, cell_h);

    assert_eq!(panes.len(), 2);
    assert_eq!(dividers.len(), 1);
    assert_eq!(dividers[0].x, 500.0);

    // Apply inner padding as done in compute_tab_layout
    let pad_x = 4.0;
    let pad_y = 3.0;
    for p in &mut panes {
        p.pad_x = pad_x;
        p.pad_y = pad_y;
        let content_w = (p.width - 2.0 * pad_x).max(cell_w);
        let content_h = (p.height - 2.0 * pad_y).max(cell_h);
        p.cols = (content_w / cell_w).floor().max(1.0) as usize;
        p.rows = (content_h / cell_h).floor().max(1.0) as usize;
    }

    // Pane 1 (left)
    assert_eq!(panes[0].pane_id, tab1_id);
    assert_eq!(panes[0].x, 0.0);
    assert_eq!(panes[0].content_x(), 4.0);
    assert_eq!(panes[0].content_y(), 3.0);
    let right_text_end = panes[0].content_x() + (panes[0].cols as f32 * cell_w);
    assert!(right_text_end <= dividers[0].x - pad_x);

    // Pane 2 (right)
    assert_eq!(panes[1].pane_id, "pane_right");
    assert_eq!(panes[1].x, 501.0);
    assert_eq!(panes[1].content_x(), 505.0); // 4px padding away from divider
    assert!(panes[1].content_x() > dividers[0].x + dividers[0].width);
    let right_pane_end = panes[1].content_x() + (panes[1].cols as f32 * cell_w);
    assert!(right_pane_end <= panes[1].x + panes[1].width);
}





