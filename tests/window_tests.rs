use celerterm::window::{calculate_header_layout, Rect};

#[test]
fn test_header_layout_disabled() {
    let tabs = vec![("tab1".to_string(), "Zsh".to_string())];
    let layout = calculate_header_layout(800.0, &tabs, false, false, 1.0);
    assert_eq!(layout.height, 0.0);
    assert!(layout.tab_rects.is_empty());
}

#[test]
fn test_header_layout_traffic_lights_offset() {
    let tabs = vec![
        ("tab1".to_string(), "Tab 1".to_string()),
        ("tab2".to_string(), "Tab 2".to_string()),
    ];

    let layout_visible = calculate_header_layout(800.0, &tabs, false, true, 1.0);
    let layout_hidden = calculate_header_layout(800.0, &tabs, true, true, 1.0);

    assert_eq!(layout_visible.height, 38.0);
    assert_eq!(layout_hidden.height, 38.0);

    // When traffic lights are hidden, tabs start much closer to the left edge
    assert_eq!(layout_hidden.tab_rects[0].1.x, 8.0);
    if cfg!(target_os = "macos") {
        assert_eq!(layout_visible.tab_rects[0].1.x, 82.0);
    }

    // Retina 2x scaling verification
    let layout_retina = calculate_header_layout(1600.0, &tabs, false, true, 2.0);
    assert_eq!(layout_retina.height, 76.0);
    if cfg!(target_os = "macos") {
        assert_eq!(layout_retina.tab_rects[0].1.x, 164.0);
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
