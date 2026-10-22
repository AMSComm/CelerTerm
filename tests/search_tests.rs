use alacritty_terminal::index::{Column, Line};
use celerterm::term::{
    TermScreen, SearchState, MatchHighlightType,
    find_matches, calculate_scroll_offset_for_line,
};
use celerterm::window::{calculate_search_bar_layout, Rect};

#[test]
fn test_search_empty_query() {
    let screen = TermScreen::new(80, 24);
    let matches = find_matches(&screen, "");
    assert!(matches.is_empty());
}

#[test]
fn test_search_viewport_plain_text() {
    let mut screen = TermScreen::new(80, 24);
    // Write text to terminal
    screen.process_bytes(b"Hello world! This is a test error in terminal.\r\nSecond line with Error again.\r\n");

    let matches = find_matches(&screen, "error");
    // Case-insensitive: should find "error" and "Error"
    assert_eq!(matches.len(), 2);

    // First match on line 0
    assert_eq!(matches[0].line, Line(0));
    assert_eq!(matches[0].start_col, 28);
    assert_eq!(matches[0].end_col, 33);

    // Second match on line 1
    assert_eq!(matches[1].line, Line(1));
    assert_eq!(matches[1].start_col, 17);
    assert_eq!(matches[1].end_col, 22);
}

#[test]
fn test_search_scrollback_history() {
    let mut screen = TermScreen::new(80, 10);
    // Write enough lines to push content into scrollback history
    for i in 0..25 {
        screen.process_bytes(format!("Log entry {:02} [keyword_xyz]\r\n", i).as_bytes());
    }

    let matches = find_matches(&screen, "keyword_xyz");
    // All 25 lines should have matches, spanning negative scrollback lines up to visible lines
    assert_eq!(matches.len(), 25);

    // Some matches should be in negative scrollback lines
    let scrollback_matches: Vec<_> = matches.iter().filter(|m| m.line.0 < 0).collect();
    assert!(!scrollback_matches.is_empty(), "Should contain matches in scrollback history");

    // Check first match is in the deepest history line
    assert!(matches[0].line.0 < 0);
    assert_eq!(matches[0].start_col, 14);
    assert_eq!(matches[0].end_col, 25);
}

#[test]
fn test_search_cjk_wide_characters() {
    let mut screen = TermScreen::new(80, 24);
    // Write Japanese full-width characters
    screen.process_bytes("Prefix 東京タワー Suffix\r\n".as_bytes());

    let matches = find_matches(&screen, "東京");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].line, Line(0));
    // "Prefix " is 7 ASCII columns (0..7).
    // "東京" starts at col 7. Each CJK char is 2 columns, so "東" (7..9), "京" (9..11).
    assert_eq!(matches[0].start_col, 7);
    assert_eq!(matches[0].end_col, 11);
}

#[test]
fn test_search_state_navigation_and_wrapping() {
    let mut screen = TermScreen::new(80, 24);
    screen.process_bytes(b"item 1\r\nitem 2\r\nitem 3\r\n");

    let mut state = SearchState::new();
    state.open();
    state.set_query("item".to_string(), &screen);

    assert_eq!(state.matches.len(), 3);
    assert_eq!(state.current_match_idx, Some(0));
    assert_eq!(state.match_counter_string(), "1/3");

    // Next match -> 1
    state.next_match();
    assert_eq!(state.current_match_idx, Some(1));
    assert_eq!(state.match_counter_string(), "2/3");

    // Next match -> 2
    state.next_match();
    assert_eq!(state.current_match_idx, Some(2));
    assert_eq!(state.match_counter_string(), "3/3");

    // Next match wraps around -> 0
    state.next_match();
    assert_eq!(state.current_match_idx, Some(0));
    assert_eq!(state.match_counter_string(), "1/3");

    // Prev match wraps backwards -> 2
    state.prev_match();
    assert_eq!(state.current_match_idx, Some(2));
    assert_eq!(state.match_counter_string(), "3/3");
}

#[test]
fn test_search_cell_highlighting() {
    let mut screen = TermScreen::new(80, 24);
    screen.process_bytes(b"foo bar foo\r\n");

    let mut state = SearchState::new();
    state.open();
    state.set_query("foo".to_string(), &screen);

    assert_eq!(state.matches.len(), 2);
    // Active match is 0: line 0, cols 0..3
    assert_eq!(state.check_cell_highlight(Line(0), 0), Some(MatchHighlightType::Active));
    assert_eq!(state.check_cell_highlight(Line(0), 1), Some(MatchHighlightType::Active));
    assert_eq!(state.check_cell_highlight(Line(0), 2), Some(MatchHighlightType::Active));

    // Space between: not highlighted
    assert_eq!(state.check_cell_highlight(Line(0), 3), None);

    // Second match: cols 8..11 (Normal highlight)
    assert_eq!(state.check_cell_highlight(Line(0), 8), Some(MatchHighlightType::Normal));
    assert_eq!(state.check_cell_highlight(Line(0), 9), Some(MatchHighlightType::Normal));
    assert_eq!(state.check_cell_highlight(Line(0), 10), Some(MatchHighlightType::Normal));

    // Switch to next match -> match 1 becomes Active, match 0 becomes Normal
    state.next_match();
    assert_eq!(state.check_cell_highlight(Line(0), 0), Some(MatchHighlightType::Normal));
    assert_eq!(state.check_cell_highlight(Line(0), 8), Some(MatchHighlightType::Active));
}

#[test]
fn test_search_scroll_offset_calculation() {
    // If target line is at -15 in history and visible_lines is 24, history_size is 50:
    // center_row = 24 / 3 = 8
    // desired_offset = 8 - (-15) = 23
    let offset = calculate_scroll_offset_for_line(Line(-15), 24, 50);
    assert_eq!(offset, 23);

    // If target line is in current visible screen (line 5):
    // center_row = 8
    // desired_offset = 8 - 5 = 3 (clamped within range)
    let offset_visible = calculate_scroll_offset_for_line(Line(5), 24, 50);
    assert_eq!(offset_visible, 3);
}

#[test]
fn test_search_bar_layout() {
    let pane_rect = Rect {
        x: 10.0,
        y: 20.0,
        width: 800.0,
        height: 600.0,
    };
    let scale = 1.0;
    let layout = calculate_search_bar_layout(pane_rect, scale);

    assert!(layout.container.width > 0.0);
    assert!(layout.container.height > 0.0);
    assert!(layout.container.x >= pane_rect.x);
    assert!(layout.container.y >= pane_rect.y);

    // Buttons must be positioned inside container
    assert!(layout.container.contains(layout.close_btn.x, layout.close_btn.y));
    assert!(layout.container.contains(layout.next_btn.x, layout.next_btn.y));
    assert!(layout.container.contains(layout.prev_btn.x, layout.prev_btn.y));
    assert!(layout.container.contains(layout.input_box.x, layout.input_box.y));
}
