use celerterm::term::TermScreen;
use alacritty_terminal::term::TermMode;

#[test]
fn test_alternate_screen_buffer_tui() {
    let mut screen = TermScreen::new(80, 24);

    // Write on primary screen
    screen.process_bytes(b"Primary Screen Output");
    assert_eq!(screen.get_line_string(0), "Primary Screen Output");

    // Enter alternate screen (used by Neovim / btop / Claude Code)
    screen.process_bytes(b"\x1b[?1049h");
    assert!(screen.term.mode().contains(TermMode::ALT_SCREEN));

    // Clear and write alternate screen content
    screen.process_bytes(b"\x1b[2J\x1b[HNeovim Buffer Active");
    assert_eq!(screen.get_line_string(0), "Neovim Buffer Active");

    // Exit alternate screen
    screen.process_bytes(b"\x1b[?1049l");
    assert!(!screen.term.mode().contains(TermMode::ALT_SCREEN));

    // Primary screen content is preserved
    assert_eq!(screen.get_line_string(0), "Primary Screen Output");
}

#[test]
fn test_bracketed_paste_and_mouse_modes() {
    let mut screen = TermScreen::new(80, 24);

    // Enable bracketed paste (mode 2004)
    screen.process_bytes(b"\x1b[?2004h");
    assert!(screen.term.mode().contains(TermMode::BRACKETED_PASTE));

    // Enable SGR mouse tracking (mode 1006 + 1000)
    screen.process_bytes(b"\x1b[?1000h\x1b[?1006h");
    assert!(screen.term.mode().contains(TermMode::SGR_MOUSE));

    // Disable
    screen.process_bytes(b"\x1b[?2004l\x1b[?1000l");
    assert!(!screen.term.mode().contains(TermMode::BRACKETED_PASTE));
}

#[test]
fn test_truecolor_24bit_rgb_and_box_drawing() {
    let mut screen = TermScreen::new(80, 24);

    // 24-bit TrueColor escape sequence: \x1b[38;2;120;180;240m
    // Box drawing characters used by Claude Code and Antigravity CLI: ┌ ─ ┐ │ └ ┘
    let box_text = "\x1b[38;2;120;180;240m┌─── CelerTerm Box ───┐\x1b[0m";
    screen.process_bytes(box_text.as_bytes());

    let line = screen.get_line_string(0);
    assert!(line.contains("┌─── CelerTerm Box ───┐"));
}

#[test]
fn test_mouse_drag_and_motion_tracking_modes() {
    let mut screen = TermScreen::new(80, 24);

    // Initial state: no mouse modes
    assert!(!screen.is_mouse_mode());
    assert!(!screen.is_mouse_drag());
    assert!(!screen.is_mouse_motion());

    // Enable mode 1000 (MOUSE_REPORT_CLICK) + 1006 (SGR_MOUSE)
    screen.process_bytes(b"\x1b[?1000h\x1b[?1006h");
    assert!(screen.is_mouse_mode());
    assert!(!screen.is_mouse_drag()); // Click only, not drag
    assert!(!screen.is_mouse_motion());

    // Enable mode 1002 (MOUSE_DRAG - used by Neovim set mouse=a)
    screen.process_bytes(b"\x1b[?1002h");
    assert!(screen.is_mouse_mode());
    assert!(screen.is_mouse_drag());
    assert!(!screen.is_mouse_motion());

    // Enable mode 1003 (MOUSE_MOTION - all motion)
    screen.process_bytes(b"\x1b[?1003h");
    assert!(screen.is_mouse_mode());
    assert!(screen.is_mouse_drag());
    assert!(screen.is_mouse_motion());

    // Disable all mouse modes
    screen.process_bytes(b"\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l");
    assert!(!screen.is_mouse_mode());
    assert!(!screen.is_mouse_drag());
    assert!(!screen.is_mouse_motion());
}

#[test]
fn test_sgr_mouse_formatting_and_drag_encoding() {
    use celerterm::term::{format_sgr_mouse, MouseEventKind};

    // Left Button Press at col 10, line 5
    assert_eq!(
        format_sgr_mouse(0, 10, 5, MouseEventKind::Press, false, false, false),
        "\x1b[<0;10;5M"
    );

    // Left Button Drag at col 15, line 6 (btn 0 + 32 = 32, suffix 'M')
    assert_eq!(
        format_sgr_mouse(0, 15, 6, MouseEventKind::Drag, false, false, false),
        "\x1b[<32;15;6M"
    );

    // Left Button Drag with Alt (+8 -> 40)
    assert_eq!(
        format_sgr_mouse(0, 15, 6, MouseEventKind::Drag, false, true, false),
        "\x1b[<40;15;6M"
    );

    // Left Button Drag with Ctrl (+16 -> 48)
    assert_eq!(
        format_sgr_mouse(0, 15, 6, MouseEventKind::Drag, false, false, true),
        "\x1b[<48;15;6M"
    );

    // Left Button Release at col 15, line 6 (btn 0, suffix 'm')
    assert_eq!(
        format_sgr_mouse(0, 15, 6, MouseEventKind::Release, false, false, false),
        "\x1b[<0;15;6m"
    );

    // Middle Button Press (btn 1, suffix 'M')
    assert_eq!(
        format_sgr_mouse(1, 20, 10, MouseEventKind::Press, false, false, false),
        "\x1b[<1;20;10M"
    );

    // Middle Button Drag (btn 1 + 32 = 33, suffix 'M')
    assert_eq!(
        format_sgr_mouse(1, 22, 10, MouseEventKind::Drag, false, false, false),
        "\x1b[<33;22;10M"
    );

    // Right Button Press (btn 2, suffix 'M')
    assert_eq!(
        format_sgr_mouse(2, 30, 12, MouseEventKind::Press, false, false, false),
        "\x1b[<2;30;12M"
    );

    // Right Button Drag (btn 2 + 32 = 34, suffix 'M')
    assert_eq!(
        format_sgr_mouse(2, 35, 12, MouseEventKind::Drag, false, false, false),
        "\x1b[<34;35;12M"
    );

    // Mouse Move with no button in mode 1003 (code 35, suffix 'M')
    assert_eq!(
        format_sgr_mouse(0, 40, 20, MouseEventKind::Move, false, false, false),
        "\x1b[<35;40;20M"
    );
}

