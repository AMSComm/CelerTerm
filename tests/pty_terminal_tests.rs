use celerterm::term::{TermScreen, TermSize};
use celerterm::pty::PtySession;
use std::io::Read;
use std::time::{Duration, Instant};

#[test]
fn test_screen_grid_ansi_processing() {
    let mut screen = TermScreen::new(80, 24);
    assert_eq!(screen.size, TermSize { columns: 80, lines: 24 });

    // Process ANSI text with red color and bold
    screen.process_bytes(b"\x1b[31mHello\x1b[0m \x1b[1mWorld\x1b[0m");

    assert_eq!(screen.get_cell_char(0, 0), 'H');
    assert_eq!(screen.get_cell_char(1, 0), 'e');
    assert_eq!(screen.get_cell_char(2, 0), 'l');
    assert_eq!(screen.get_cell_char(3, 0), 'l');
    assert_eq!(screen.get_cell_char(4, 0), 'o');
    assert_eq!(screen.get_cell_char(5, 0), ' ');
    assert_eq!(screen.get_cell_char(6, 0), 'W');

    assert_eq!(screen.get_line_string(0), "Hello World");
}

#[test]
fn test_screen_resize() {
    let mut screen = TermScreen::new(80, 24);
    screen.process_bytes(b"Top line text");

    screen.resize(120, 40);
    assert_eq!(screen.size.columns, 120);
    assert_eq!(screen.size.lines, 40);
    assert_eq!(screen.get_line_string(0), "Top line text");
}

#[test]
fn test_pty_spawn_and_shell_communication() {
    let mut pty = PtySession::spawn(80, 24, None).expect("Pty spawn succeeds");
    let mut screen = TermScreen::new(80, 24);

    // Send command to PTY shell
    pty.write_all(b"echo celerterm_live_check\n").expect("Write to pty succeeds");

    // Read output in a non-blocking / timed loop
    let mut buf = [0u8; 1024];
    let start = Instant::now();
    let timeout = Duration::from_secs(8);
    let mut found = false;

    while start.elapsed() < timeout {
        match pty.reader.read(&mut buf) {
            Ok(n) if n > 0 => {
                screen.process_bytes(&buf[..n]);
                let text = screen.get_screen_text().join(" ");
                if text.contains("celerterm_live_check") {
                    found = true;
                    break;
                }
            }
            Ok(_) => break,
            Err(e) => {
                // If WouldBlock or interrupted, sleep briefly
                std::thread::sleep(Duration::from_millis(10));
                let _ = e;
            }
        }
    }

    assert!(found, "PTY output should contain 'celerterm_live_check'");
}

#[test]
fn test_renderable_content_and_scroll() {
    let mut screen = TermScreen::new(80, 24);
    for i in 0..50 {
        screen.process_bytes(format!("Line number {}\r\n", i).as_bytes());
    }

    screen.scroll_display(5);
    assert_eq!(screen.display_offset(), 5);

    screen.scroll_display(-5);
    assert_eq!(screen.display_offset(), 0);
}

#[test]
fn test_get_bootstrapped_path() {
    let path = celerterm::pty::get_bootstrapped_path();
    assert!(!path.is_empty());
    #[cfg(target_os = "macos")]
    if std::path::Path::new("/opt/homebrew/bin").exists() {
        assert!(path.contains("/opt/homebrew/bin"));
    }
}

#[test]
fn test_pty_login_shell_and_path_bootstrap() {
    let mut pty = PtySession::spawn(80, 24, None).expect("Pty spawn succeeds");
    let mut screen = TermScreen::new(80, 24);

    pty.write_all(b"echo PATH_RESULT=$PATH\n").expect("Write to pty succeeds");

    let mut buf = [0u8; 1024];
    let start = Instant::now();
    let timeout = Duration::from_secs(8);
    let mut output = String::new();

    while start.elapsed() < timeout {
        match pty.reader.read(&mut buf) {
            Ok(n) if n > 0 => {
                screen.process_bytes(&buf[..n]);
                let text = screen.get_screen_text().join(" ");
                if text.contains("PATH_RESULT=/") {
                    output = text;
                    break;
                }
            }
            Ok(_) => break,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    assert!(output.contains("PATH_RESULT=/"), "PTY output should have executed command with expanded PATH");
    #[cfg(target_os = "macos")]
    if std::path::Path::new("/opt/homebrew/bin").exists() {
        assert!(output.contains("/opt/homebrew/bin"), "Spawned login shell must have /opt/homebrew/bin in PATH");
    }
}

#[test]
fn test_pty_workspace_env_variables() {
    let mut pty = PtySession::spawn_with_workspace(
        80,
        24,
        None,
        Some("AlphaWs"),
        Some("alpha-id-123"),
    ).expect("Pty spawn with workspace succeeds");
    let mut screen = TermScreen::new(80, 24);

    pty.write_all(b"echo CELER_TEST_OUT=[$CELERTERM_WORKSPACE][$CELER_WORKSPACE][$CELERTERM_WORKSPACE_ID]\n")
        .expect("Write to pty succeeds");

    let mut buf = [0u8; 1024];
    let start = Instant::now();
    let timeout = Duration::from_secs(8);
    let mut output = String::new();

    while start.elapsed() < timeout {
        match pty.reader.read(&mut buf) {
            Ok(n) if n > 0 => {
                screen.process_bytes(&buf[..n]);
                let text = screen.get_screen_text().join(" ");
                if text.contains("CELER_TEST_OUT=[AlphaWs][AlphaWs][alpha-id-123]") {
                    output = text;
                    break;
                }
            }
            Ok(_) => break,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    assert!(
        output.contains("CELER_TEST_OUT=[AlphaWs][AlphaWs][alpha-id-123]"),
        "Spawned PTY must receive CELERTERM_WORKSPACE, CELER_WORKSPACE, and CELERTERM_WORKSPACE_ID"
    );
}
