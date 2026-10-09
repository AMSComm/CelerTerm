use celerterm::config::schema::Config;
use celerterm::term::TermScreen;
use celerterm::window::scrollbar::calculate_scrollbar_layout;

#[test]
fn test_scrollbar_config_defaults_and_override() {
    let default_cfg = Config::default();
    assert!(default_cfg.window.show_scrollbar);

    let toml_str = r#"
        [window]
        show_scrollbar = false
    "#;
    let custom_cfg: Config = toml::from_str(toml_str).expect("Parse TOML");
    assert!(!custom_cfg.window.show_scrollbar);
}

#[test]
fn test_screen_history_size_and_scroll_to_offset() {
    let mut screen = TermScreen::new(80, 10);
    assert_eq!(screen.history_size(), 0);
    assert_eq!(screen.display_offset(), 0);

    // Feed 30 lines so that 20 lines overflow into history (screen lines = 10)
    for i in 1..=30 {
        screen.process_bytes(format!("Line {}\r\n", i).as_bytes());
    }

    let hist = screen.history_size();
    assert_eq!(hist, 21);
    assert_eq!(screen.display_offset(), 0); // At bottom by default

    // Scroll to top
    screen.scroll_to_top();
    assert_eq!(screen.display_offset(), 21);

    // Scroll to bottom
    screen.scroll_to_bottom();
    assert_eq!(screen.display_offset(), 0);

    // Scroll to specific offset
    screen.scroll_to_offset(10);
    assert_eq!(screen.display_offset(), 10);

    screen.scroll_to_offset(5);
    assert_eq!(screen.display_offset(), 5);

    // Clamping past max history
    screen.scroll_to_offset(50);
    assert_eq!(screen.display_offset(), 21);
}

#[test]
fn test_scrollbar_layout_and_thumb_proportions() {
    let layout = calculate_scrollbar_layout(
        "pane-0",
        8.0,
        30.0,
        800.0,
        600.0,
        100, // 100 history lines
        25,  // 25 visible lines
        0,   // at bottom
        1.0,
    );

    assert!(layout.is_scrollable);
    assert_eq!(layout.history_lines, 100);
    assert_eq!(layout.display_offset, 0);

    // Track should be located near the right edge of the pane
    assert!(layout.track_rect.x > 800.0 - 20.0);
    assert_eq!(layout.track_rect.y, 32.0);
    assert_eq!(layout.track_rect.height, 600.0 - 4.0);

    // When at bottom, thumb bottom matches track bottom
    let thumb_bottom = layout.thumb_rect.y + layout.thumb_rect.height;
    let track_bottom = layout.track_rect.y + layout.track_rect.height;
    assert!((thumb_bottom - track_bottom).abs() <= 1.0);

    // When at top (display_offset == 100)
    let top_layout = calculate_scrollbar_layout(
        "pane-0",
        8.0,
        30.0,
        800.0,
        600.0,
        100,
        25,
        100,
        1.0,
    );
    assert!((top_layout.thumb_rect.y - top_layout.track_rect.y).abs() <= 1.0);

    // Hit rect contains track rect
    assert!(layout.hit_rect.contains(layout.track_rect.x, layout.track_rect.y + 10.0));
}

#[test]
fn test_scrollbar_drag_math() {
    let history_lines = 100usize;
    let track_y = 30.0f32;
    let track_h = 500.0f32;
    let thumb_h = 50.0f32;
    let travel = track_h - thumb_h; // 450.0

    // User clicks center of thumb (drag_offset_y = 25.0) and drags mouse to y = 30.0 + 25.0 = 55.0 (top)
    let mouse_y = 55.0f32;
    let drag_offset_y = 25.0f32;
    let thumb_y = (mouse_y - drag_offset_y).clamp(track_y, track_y + travel);
    assert_eq!(thumb_y, track_y);
    let scroll_progress = (thumb_y - track_y) / travel;
    assert_eq!(scroll_progress, 0.0);
    let target_offset = ((1.0 - scroll_progress) * history_lines as f32).round() as usize;
    assert_eq!(target_offset, 100); // Scrolled all the way to top of history

    // Drag mouse to bottom
    let mouse_y_bottom = track_y + travel + drag_offset_y;
    let thumb_y_bottom = (mouse_y_bottom - drag_offset_y).clamp(track_y, track_y + travel);
    assert_eq!(thumb_y_bottom, track_y + travel);
    let scroll_progress_bottom = (thumb_y_bottom - track_y) / travel;
    assert_eq!(scroll_progress_bottom, 1.0);
    let target_offset_bottom = ((1.0 - scroll_progress_bottom) * history_lines as f32).round() as usize;
    assert_eq!(target_offset_bottom, 0); // Scrolled to prompt / bottom
}

#[test]
fn test_scrollbar_margin_and_gap() {
    let pane_x = 0.0f32;
    let pane_w = 500.0f32;
    let scale = 1.0f32;
    let layout = calculate_scrollbar_layout(
        "pane-1",
        pane_x,
        0.0,
        pane_w,
        400.0,
        50,
        25,
        0,
        scale,
    );

    // With scale=1.0: sb_width=6.0, margin_right=1.0
    // track_x = 0 + 500 - 6 - 1 = 493.0
    assert_eq!(layout.track_rect.x, 493.0);
    assert_eq!(layout.track_rect.width, 6.0);
    // Gap from right edge is exactly 1.0
    let right_gap = (pane_x + pane_w) - (layout.track_rect.x + layout.track_rect.width);
    assert_eq!(right_gap, 1.0);
}

#[test]
fn test_scrollbar_mode_configs() {
    let default_cfg = Config::default();
    assert_eq!(default_cfg.window.scrollbar_mode, celerterm::config::ScrollbarMode::Autohide);
    assert_eq!(default_cfg.window.effective_scrollbar_mode(), celerterm::config::ScrollbarMode::Autohide);

    let toml_always = r#"
        [window]
        scrollbar_mode = "always"
    "#;
    let cfg_always: Config = toml::from_str(toml_always).expect("Parse TOML always");
    assert_eq!(cfg_always.window.scrollbar_mode, celerterm::config::ScrollbarMode::Always);
    assert_eq!(cfg_always.window.effective_scrollbar_mode(), celerterm::config::ScrollbarMode::Always);

    let toml_never = r#"
        [window]
        scrollbar_mode = "never"
    "#;
    let cfg_never: Config = toml::from_str(toml_never).expect("Parse TOML never");
    assert_eq!(cfg_never.window.scrollbar_mode, celerterm::config::ScrollbarMode::Never);
    assert_eq!(cfg_never.window.effective_scrollbar_mode(), celerterm::config::ScrollbarMode::Never);

    // Backwards compatibility with show_scrollbar = false
    let toml_compat = r#"
        [window]
        show_scrollbar = false
        scrollbar_mode = "always"
    "#;
    let cfg_compat: Config = toml::from_str(toml_compat).expect("Parse TOML compat");
    assert_eq!(cfg_compat.window.effective_scrollbar_mode(), celerterm::config::ScrollbarMode::Never);
}

