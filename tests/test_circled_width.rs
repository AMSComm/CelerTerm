use celerterm::term::screen::TermScreen;
use unicode_width::UnicodeWidthChar;

#[test]
fn test_circled_number_width_in_terminal() {
    let ch = '③';
    println!("③ width: {:?}", ch.width());
    assert_eq!(ch.width(), Some(2));

    let mut screen = TermScreen::new(20, 5);
    screen.process_bytes("③'".as_bytes());

    let (c0, _, _) = screen.get_render_cell(0, 0);
    let (c1, _, _) = screen.get_render_cell(1, 0);
    let (c2, _, _) = screen.get_render_cell(2, 0);

    let cell0 = &screen.term.grid()[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(0)];
    let cell1 = &screen.term.grid()[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(1)];
    let cell2 = &screen.term.grid()[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(2)];

    println!("Col 0: '{}', flags={:?}", c0, cell0.flags);
    println!("Col 1: '{}', flags={:?}", c1, cell1.flags);
    println!("Col 2: '{}', flags={:?}", c2, cell2.flags);

    assert_eq!(c0, '③');
    assert!(cell0.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR));
    assert!(cell1.flags.contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER));
    assert_eq!(c2, '\'');
    assert!(screen.is_wide_cell(0, 0));
}

#[test]
fn test_is_wide_cell_in_scrollback_history() {
    let mut screen = TermScreen::new(20, 5);
    // Write a wide character on line 0
    screen.process_bytes("③'\r\n".as_bytes());

    // Push 10 lines so that line 0 scrolls into history
    for i in 1..=10 {
        screen.process_bytes(format!("Line {}\r\n", i).as_bytes());
    }

    assert!(screen.history_size() > 0);

    // Scroll all the way to the top of scrollback
    screen.scroll_to_top();
    assert!(screen.display_offset() > 0);

    // At top of scrollback, line 0 is the original line with '③'
    assert!(screen.is_wide_cell(0, 0));
    assert!(!screen.is_wide_cell(1, 0)); // Column 1 is the spacer, not wide
    assert!(!screen.is_wide_cell(2, 0)); // Column 2 is '\'', not wide
}
