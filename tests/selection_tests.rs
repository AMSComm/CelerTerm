use celerterm::term::TermScreen;
use celerterm::renderer::parse_hex_color;
use celerterm::term::keymap::{translate_key, Modifiers, KeyAction};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::SelectionType;
use winit::keyboard::Key;

#[test]
fn test_hex_color_parsing() {
    assert_eq!(parse_hex_color("#33467c", 0), 0x0033467c);
    assert_eq!(parse_hex_color("33467c", 0), 0x0033467c);
    assert_eq!(parse_hex_color("#C0CAF5", 0), 0x00c0caf5);
    assert_eq!(parse_hex_color("invalid_hex", 0x123456), 0x123456);
}

#[test]
fn test_ctrl_shift_c_and_v_shortcuts() {
    let mods = Modifiers {
        alt: false,
        ctrl: true,
        shift: true,
        logo: false,
    };

    assert_eq!(
        translate_key(&Key::Character("C".into()), mods, true),
        Some(KeyAction::Copy)
    );
    assert_eq!(
        translate_key(&Key::Character("c".into()), mods, true),
        Some(KeyAction::Copy)
    );
    assert_eq!(
        translate_key(&Key::Character("V".into()), mods, true),
        Some(KeyAction::Paste)
    );
    assert_eq!(
        translate_key(&Key::Character("v".into()), mods, true),
        Some(KeyAction::Paste)
    );
}

#[test]
fn test_screen_selection_lifecycle() {
    let mut screen = TermScreen::new(80, 24);
    screen.process_bytes(b"CelerTerm blazing fast terminal emulator\r\nSecond line");

    // Initially no selection
    assert_eq!(screen.selection_range(), None);
    assert_eq!(screen.copy_selection_text(), None);
    assert!(!screen.is_point_selected(0, 0));

    // Start simple selection from column 0 to column 8 ("CelerTerm")
    let start_pt = Point::new(Line(0), Column(0));
    screen.start_selection(SelectionType::Simple, start_pt, Side::Left);

    let end_pt = Point::new(Line(0), Column(8));
    screen.update_selection(end_pt, Side::Right);

    // Verify selection is present and contains the points
    assert!(screen.selection_range().is_some());
    assert!(screen.is_point_selected(0, 0));
    assert!(screen.is_point_selected(5, 0));
    assert!(screen.is_point_selected(8, 0));
    assert!(!screen.is_point_selected(15, 0));

    // Verify copied text
    let selected_text = screen.copy_selection_text();
    assert_eq!(selected_text.as_deref(), Some("CelerTerm"));

    // Clear selection
    screen.clear_selection();
    assert_eq!(screen.selection_range(), None);
    assert_eq!(screen.copy_selection_text(), None);
    assert!(!screen.is_point_selected(0, 0));
}

#[test]
fn test_screen_semantic_word_selection() {
    let mut screen = TermScreen::new(80, 24);
    screen.process_bytes(b"cargo build --release\r\n");

    // Double-click on "build" (col 6)
    let pt = Point::new(Line(0), Column(6));
    screen.start_selection(SelectionType::Semantic, pt, Side::Left);

    let text = screen.copy_selection_text();
    assert_eq!(text.as_deref(), Some("build"));
}

#[test]
fn test_screen_lines_selection() {
    let mut screen = TermScreen::new(80, 24);
    screen.process_bytes(b"Line one text\r\nLine two text\r\n");

    // Triple-click on line 0
    let pt = Point::new(Line(0), Column(5));
    screen.start_selection(SelectionType::Lines, pt, Side::Left);

    let text = screen.copy_selection_text();
    assert!(text.is_some());
    let s = text.unwrap();
    assert!(s.contains("Line one text"));
}
