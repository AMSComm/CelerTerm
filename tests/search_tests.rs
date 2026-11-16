use alacritty_terminal::index::Line;
use celerterm::term::{
    TermScreen, SearchState, MatchHighlightType,
    find_matches, find_matches_with_cap, calculate_scroll_offset_for_line,
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

#[test]
fn test_search_matches_for_line() {
    let mut screen = TermScreen::new(80, 24);
    screen.process_bytes(b"hello world\r\nfoo hello bar hello\r\nthird line\r\nhello end\r\n");

    let mut state = SearchState::new();
    state.open();
    state.set_query("hello".to_string(), &screen);

    // Line 0 has 1 "hello"
    let line0_matches = state.matches_for_line(Line(0));
    assert_eq!(line0_matches.len(), 1);
    assert_eq!(line0_matches[0].start_col, 0);
    assert_eq!(line0_matches[0].end_col, 5);

    // Line 1 has 2 "hello"s
    let line1_matches = state.matches_for_line(Line(1));
    assert_eq!(line1_matches.len(), 2);
    assert_eq!(line1_matches[0].start_col, 4);
    assert_eq!(line1_matches[0].end_col, 9);
    assert_eq!(line1_matches[1].start_col, 14);
    assert_eq!(line1_matches[1].end_col, 19);

    // Line 2 has 0 "hello"s
    let line2_matches = state.matches_for_line(Line(2));
    assert!(line2_matches.is_empty());

    // Line 3 has 1 "hello"
    let line3_matches = state.matches_for_line(Line(3));
    assert_eq!(line3_matches.len(), 1);
    assert_eq!(line3_matches[0].start_col, 0);
    assert_eq!(line3_matches[0].end_col, 5);
}

#[test]
fn test_search_capping_max_matches() {
    let mut screen = TermScreen::new(80, 30);
    for i in 0..20 {
        screen.process_bytes(format!("test line {}\r\n", i).as_bytes());
    }

    // Cap at 5 matches
    let (matches, is_capped) = find_matches_with_cap(&screen, "test", 5);
    assert_eq!(matches.len(), 5);
    assert!(is_capped);

    let mut state = SearchState::new();
    state.open();
    state.matches = matches;
    state.is_capped = is_capped;
    state.current_match_idx = Some(0);

    assert_eq!(state.match_counter_string(), "1/5+");
}

#[test]
fn test_search_cursor_navigation_and_editing() {
    let mut state = SearchState::new();
    state.open();
    assert_eq!(state.cursor_pos, 0);

    // Typing "hello"
    state.insert_str("hello");
    assert_eq!(state.query, "hello");
    assert_eq!(state.cursor_pos, 5);

    // Move left twice -> cursor at 3 (before second 'l')
    state.move_cursor_left();
    state.move_cursor_left();
    assert_eq!(state.cursor_pos, 3);

    // Insert "XYZ" in the middle -> "helXYZlo"
    state.insert_str("XYZ");
    assert_eq!(state.query, "helXYZlo");
    assert_eq!(state.cursor_pos, 6);

    // Delete backward (Backspace deletes 'Z') -> "helXYlo"
    assert!(state.delete_backward());
    assert_eq!(state.query, "helXYlo");
    assert_eq!(state.cursor_pos, 5);

    // Delete forward (Delete deletes 'l') -> "helXYo"
    assert!(state.delete_forward());
    assert_eq!(state.query, "helXYo");
    assert_eq!(state.cursor_pos, 5);

    // Home -> cursor at 0
    state.move_cursor_home();
    assert_eq!(state.cursor_pos, 0);
    assert!(!state.delete_backward()); // Cannot delete backward at index 0

    // End -> cursor at 6
    state.move_cursor_end();
    assert_eq!(state.cursor_pos, 6);
    assert!(!state.delete_forward()); // Cannot delete forward at end
}

#[test]
fn test_search_multiline_wrapped_content() {
    // 10 columns wide
    let mut screen = TermScreen::new(10, 5);
    // Writing 20 bytes wraps across Line(0) and Line(1) with WRAPLINE
    screen.process_bytes(b"1234567890abcdefghij");

    let matches = find_matches(&screen, "90ab");
    // Should find the wrapped match spanning Line(0) and Line(1)
    assert_eq!(matches.len(), 2, "Should have 2 match pieces across wrapped lines");

    // First piece on Line(0), columns 8..10
    assert_eq!(matches[0].line, Line(0));
    assert_eq!(matches[0].start_col, 8);
    assert_eq!(matches[0].end_col, 10);
    assert_eq!(matches[0].match_group_id, 0);

    // Second piece on Line(1), columns 0..2
    assert_eq!(matches[1].line, Line(1));
    assert_eq!(matches[1].start_col, 0);
    assert_eq!(matches[1].end_col, 2);
    assert_eq!(matches[1].match_group_id, 0);
}

#[test]
fn test_search_unicode_decomposed_vietnamese() {
    let mut screen = TermScreen::new(30, 5);
    // Decomposed Vietnamese text with combining accents in zerowidth cells
    screen.process_bytes("tie\u{0302}\u{0301}ng vie\u{0302}\u{0323}t".as_bytes());

    // Search for precomposed NFC string ("ti\u{1EBF}ng")
    let matches_tieng = find_matches(&screen, "ti\u{1EBF}ng");
    assert_eq!(matches_tieng.len(), 1, "Should find precomposed search in decomposed buffer");
    assert_eq!(matches_tieng[0].line, Line(0));
    assert_eq!(matches_tieng[0].start_col, 0);
    assert_eq!(matches_tieng[0].end_col, 5);

    // Search for precomposed NFC string ("vi\u{1EC7}t")
    let matches_viet = find_matches(&screen, "vi\u{1EC7}t");
    assert_eq!(matches_viet.len(), 1, "Should find precomposed search in decomposed buffer");
    assert_eq!(matches_viet[0].line, Line(0));
    assert_eq!(matches_viet[0].start_col, 6);
    assert_eq!(matches_viet[0].end_col, 10);
}

#[test]
fn test_search_scroll_offset_viewport_aware() {
    let mut screen = TermScreen::new(80, 24);
    screen.process_bytes(b"first line\r\nmiddle error test\r\nlast line\r\n");

    let mut state = SearchState::new();
    state.open();
    state.set_query("error".to_string(), &screen);

    // When display_offset is 0, Line(1) is visible on screen (0..24).
    // Viewport-aware scroll offset should return None (no unnecessary jumping).
    let offset_visible = state.scroll_offset_for_current_match(0, 24, 100);
    assert_eq!(offset_visible, None, "Should not scroll when match is already visible");

    // If viewport was scrolled to display_offset = 50, Line(1) is outside the viewport:
    let offset_outside = state.scroll_offset_for_current_match(50, 24, 100);
    assert!(offset_outside.is_some(), "Should compute target offset when match is outside viewport");
}
