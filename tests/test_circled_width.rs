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
}
