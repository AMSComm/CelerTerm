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
