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

#[test]
fn test_shift_click_extend_selection() {
    let mut screen = TermScreen::new(80, 24);
    screen.process_bytes(b"Hello World from CelerTerm\r\nSecond line here");

    // 1. Initial click at 'W' of "World" (col 6)
    let pt_w = Point::new(Line(0), Column(6));
    screen.start_selection(SelectionType::Simple, pt_w, Side::Left);

    // Simulating mouse release without dragging (empty selection cleared, anchor preserved)
    screen.clear_selection();
    assert_eq!(screen.selection_range(), None);
    assert_eq!(screen.selection_anchor, Some(pt_w));

    // 2. Shift + Click at 'd' of "World" (col 10)
    let pt_d = Point::new(Line(0), Column(10));
    screen.extend_selection_to(pt_d, Side::Right);

    assert!(screen.selection_range().is_some());
    assert_eq!(screen.copy_selection_text().as_deref(), Some("World"));

    // 3. Sequential Shift + Click further to the right ('m' of "CelerTerm", col 25)
    let pt_m = Point::new(Line(0), Column(25));
    screen.extend_selection_to(pt_m, Side::Right);
    assert_eq!(screen.copy_selection_text().as_deref(), Some("World from CelerTerm"));

    // 4. Shift + Click backwards from anchor ('H' of "Hello", col 0)
    let pt_h = Point::new(Line(0), Column(0));
    screen.extend_selection_to(pt_h, Side::Left);
    assert_eq!(screen.copy_selection_text().as_deref(), Some("Hello W"));
}

#[test]
fn test_shift_arrow_right_left_lifecycle() {
    let mut screen = TermScreen::new(80, 24);
    // Write text and send carriage return '\r' so cursor returns to column 0
    screen.process_bytes(b"ABCDEFGH\r");

    // Initially at column 0 (cursor is at 0, 0)
    assert_eq!(screen.selection_range(), None);

    // 1. Shift + Right: selects first character 'A'
    screen.select_right();
    assert!(screen.selection_range().is_some());
    assert_eq!(screen.copy_selection_text().as_deref(), Some("A"));

    // 2. Shift + Right: selects 'AB'
    screen.select_right();
    assert_eq!(screen.copy_selection_text().as_deref(), Some("AB"));

    // 3. Shift + Right: selects 'ABC'
    screen.select_right();
    assert_eq!(screen.copy_selection_text().as_deref(), Some("ABC"));

    // 4. Shift + Left: shrinks back to 'AB'
    screen.select_left();
    assert_eq!(screen.copy_selection_text().as_deref(), Some("AB"));

    // 5. Shift + Left: shrinks back to 'A'
    screen.select_left();
    assert_eq!(screen.copy_selection_text().as_deref(), Some("A"));

    // 6. Shift + Left: collapses to empty selection
    screen.select_left();
    assert_eq!(screen.selection_range(), None);
    assert_eq!(screen.copy_selection_text(), None);

    // 7. Another Shift + Right restarts selection forward
    screen.select_right();
    assert_eq!(screen.copy_selection_text().as_deref(), Some("A"));
}

#[test]
fn test_shift_arrow_up_down() {
    let mut screen = TermScreen::new(80, 24);
    screen.process_bytes(b"Row 0 text\r\nRow 1 text\r\nRow 2 text");

    // Set anchor at (Line 0, Column 9) - end of "Row 0 text"
    screen.start_selection(SelectionType::Simple, Point::new(Line(0), Column(9)), Side::Left);

    // Shift + Down expands to Row 1 Column 9
    screen.select_down();
    let text1 = screen.copy_selection_text().unwrap();
    assert!(text1.contains("Row 1 text"));

    // Shift + Down expands to Row 2 Column 9
    screen.select_down();
    let text2 = screen.copy_selection_text().unwrap();
    assert!(text2.contains("Row 2 text"));

    // Shift + Up shrinks back to Row 1
    screen.select_up();
    let text3 = screen.copy_selection_text().unwrap();
    assert!(text3.contains("Row 1 text"));

    // Shift + Up shrinks back to Row 0 (exactly cell at Column 9)
    screen.select_up();
    let text4 = screen.copy_selection_text().unwrap();
    assert_eq!(text4.trim(), "t");
}

#[test]
fn test_keyaction_parse_and_translate_shift_arrows() {
    use celerterm::term::keymap::parse_action_string;

    // Parse action strings
    assert_eq!(parse_action_string("select_left", false), Some(KeyAction::SelectLeft));
    assert_eq!(parse_action_string("select_right", false), Some(KeyAction::SelectRight));
    assert_eq!(parse_action_string("select_up", false), Some(KeyAction::SelectUp));
    assert_eq!(parse_action_string("select_down", false), Some(KeyAction::SelectDown));

    // Shift + Arrows translation
    let shift_mods = Modifiers {
        alt: false,
        ctrl: false,
        shift: true,
        logo: false,
    };
    assert_eq!(
        translate_key(&Key::Named(winit::keyboard::NamedKey::ArrowLeft), shift_mods, true),
        Some(KeyAction::SelectLeft)
    );
    assert_eq!(
        translate_key(&Key::Named(winit::keyboard::NamedKey::ArrowRight), shift_mods, true),
        Some(KeyAction::SelectRight)
    );
    assert_eq!(
        translate_key(&Key::Named(winit::keyboard::NamedKey::ArrowUp), shift_mods, true),
        Some(KeyAction::SelectUp)
    );
    assert_eq!(
        translate_key(&Key::Named(winit::keyboard::NamedKey::ArrowDown), shift_mods, true),
        Some(KeyAction::SelectDown)
    );
}

