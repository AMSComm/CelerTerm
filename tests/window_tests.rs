use celerterm::window::{calculate_header_layout, Rect};

#[test]
fn test_header_layout_disabled() {
    let tabs = vec![("tab1".to_string(), "Zsh".to_string())];
    let layout = calculate_header_layout(800.0, &tabs, false, false);
    assert_eq!(layout.height, 0.0);
    assert!(layout.tab_rects.is_empty());
}

#[test]
fn test_header_layout_traffic_lights_offset() {
    let tabs = vec![
        ("tab1".to_string(), "Tab 1".to_string()),
        ("tab2".to_string(), "Tab 2".to_string()),
    ];

    let layout_visible = calculate_header_layout(800.0, &tabs, false, true);
    let layout_hidden = calculate_header_layout(800.0, &tabs, true, true);

    assert_eq!(layout_visible.height, 34.0);
    assert_eq!(layout_hidden.height, 34.0);

    // When traffic lights are hidden, tabs start much closer to the left edge
    assert_eq!(layout_hidden.tab_rects[0].1.x, 8.0);
    if cfg!(target_os = "macos") {
        assert_eq!(layout_visible.tab_rects[0].1.x, 80.0);
    }
}

#[test]
fn test_rect_hit_testing() {
    let rect = Rect {
        x: 80.0,
        y: 4.0,
        width: 150.0,
        height: 26.0,
    };

    assert!(rect.contains(85.0, 10.0));
    assert!(rect.contains(80.0, 4.0));
    assert!(rect.contains(230.0, 30.0));
    assert!(!rect.contains(79.0, 10.0));
    assert!(!rect.contains(231.0, 10.0));
    assert!(!rect.contains(100.0, 31.0));
}
