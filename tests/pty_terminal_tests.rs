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
    let timeout = Duration::from_secs(3);
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
