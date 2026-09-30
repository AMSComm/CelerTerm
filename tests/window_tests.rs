use celerterm::window::{
    calculate_confirm_delete_buttons, calculate_header_layout, calculate_modal_buttons, Rect,
};

#[test]
fn test_header_layout_disabled() {
    let tabs = vec![("tab1".to_string(), "Zsh".to_string())];
    let layout = calculate_header_layout(800.0, &tabs, false, false, 1.0, 8.0);
    assert_eq!(layout.height, 0.0);
    assert!(layout.tab_rects.is_empty());
}

#[test]
fn test_header_layout_traffic_lights_offset() {
    let tabs = vec![
        ("tab1".to_string(), "Tab 1".to_string()),
        ("tab2".to_string(), "Tab 2".to_string()),
    ];

    let layout_visible = calculate_header_layout(800.0, &tabs, false, true, 1.0, 8.0);
    let layout_hidden = calculate_header_layout(800.0, &tabs, true, true, 1.0, 8.0);

    assert_eq!(layout_visible.height, 26.0);
    assert_eq!(layout_hidden.height, 26.0);

    // When traffic lights are hidden, tabs start much closer to the left edge
    assert_eq!(layout_hidden.tab_rects[0].1.x, 8.0);
    if cfg!(target_os = "macos") {
        assert_eq!(layout_visible.tab_rects[0].1.x, 82.0);
    }

    // Retina 2x scaling verification
    let layout_retina = calculate_header_layout(1600.0, &tabs, false, true, 2.0, 8.0);
    assert_eq!(layout_retina.height, 52.0);
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

#[test]
fn test_modal_buttons_fit_within_modal_no_overflow() {
    let modal = Rect {
        x: 100.0,
        y: 80.0,
        width: 580.0,
        height: 320.0,
    };
    let buttons = calculate_modal_buttons(modal, 42.0, 1.0, 8.5);

    assert_eq!(buttons.len(), 6);

    // Verify labels
    assert_eq!(buttons[0].id, "new");
    assert_eq!(buttons[0].label, "[n] New");
    assert_eq!(buttons[1].id, "rename");
    assert_eq!(buttons[1].label, "[r] Rename");
    assert_eq!(buttons[2].id, "color");
    assert_eq!(buttons[2].label, "[c] Color");
    assert_eq!(buttons[3].id, "delete");
    assert_eq!(buttons[3].label, "[d] Delete");
    assert_eq!(buttons[4].id, "window");
    assert_eq!(buttons[4].label, "[w] Window");
    assert_eq!(buttons[5].id, "switch");
    assert_eq!(buttons[5].label, "[Enter] Switch");

    // Verify all buttons strictly fit within modal boundaries with zero overflow
    for btn in &buttons {
        assert!(btn.rect.x >= modal.x, "Button {} starts to the left of modal", btn.id);
        assert!(
            btn.rect.x + btn.rect.width <= modal.x + modal.width,
            "Button {} overflows modal right boundary (btn right: {}, modal right: {})",
            btn.id,
            btn.rect.x + btn.rect.width,
            modal.x + modal.width
        );
        assert!(btn.rect.y >= modal.y, "Button {} above modal", btn.id);
        assert!(
            btn.rect.y + btn.rect.height <= modal.y + modal.height,
            "Button {} below modal footer",
            btn.id
        );
    }

    // Verify buttons are laid out sequentially from left to right without overlap
    for i in 0..5 {
        assert!(
            buttons[i].rect.x + buttons[i].rect.width <= buttons[i + 1].rect.x,
            "Buttons {} and {} overlap",
            buttons[i].id,
            buttons[i + 1].id
        );
    }
}

#[test]
fn test_calculate_confirm_delete_buttons() {
    let modal = Rect {
        x: 100.0,
        y: 80.0,
        width: 580.0,
        height: 320.0,
    };
    let buttons = calculate_confirm_delete_buttons(modal, 42.0, 1.0, 8.5);

    assert_eq!(buttons.len(), 2);
    assert_eq!(buttons[0].id, "confirm_delete");
    assert_eq!(buttons[0].label, "[y] Confirm Delete");
    assert_eq!(buttons[0].color, 0x00F7768E);

    assert_eq!(buttons[1].id, "cancel_delete");
    assert_eq!(buttons[1].label, "[Esc / n] Cancel");
    assert_eq!(buttons[1].color, 0x007AA2F7);

    for btn in &buttons {
        assert!(btn.rect.x >= modal.x);
        assert!(
            btn.rect.x + btn.rect.width <= modal.x + modal.width,
            "Button {} exceeded modal width",
            btn.id
        );
        assert!(btn.rect.y >= modal.y);
        assert!(
            btn.rect.y + btn.rect.height <= modal.y + modal.height,
            "Button {} exceeded modal height",
            btn.id
        );
    }

    assert!(buttons[0].rect.x + buttons[0].rect.width <= buttons[1].rect.x);
}

#[test]
fn test_modal_buttons_responsive_narrow_modal() {
    // Narrow modal (e.g. 420px width)
    let modal = Rect {
        x: 10.0,
        y: 20.0,
        width: 420.0,
        height: 280.0,
    };
    let buttons = calculate_modal_buttons(modal, 38.0, 1.0, 8.5);

    for btn in &buttons {
        assert!(btn.rect.x >= modal.x);
        assert!(
            btn.rect.x + btn.rect.width <= modal.x + modal.width,
            "Button {} overflowed in narrow modal",
            btn.id
        );
    }
}

#[test]
fn test_modal_buttons_retina_2x_scaling() {
    let modal = Rect {
        x: 200.0,
        y: 160.0,
        width: 1160.0, // 580 * 2
        height: 640.0,
    };
    let buttons = calculate_modal_buttons(modal, 84.0, 2.0, 17.0);

    for btn in &buttons {
        assert!(btn.rect.x >= modal.x);
        assert!(
            btn.rect.x + btn.rect.width <= modal.x + modal.width,
            "Retina button {} overflowed",
            btn.id
        );
    }
}

#[test]
fn test_header_layout_menu_button() {
    let tabs = vec![("t1".to_string(), "Shell".to_string())];
    let layout = calculate_header_layout(800.0, &tabs, false, true, 1.0, 8.0);

    assert!(layout.menu_button_rect.width > 0.0);
    assert!(layout.menu_button_rect.height > 0.0);
    assert!(layout.menu_button_rect.x + layout.menu_button_rect.width <= 800.0);
    assert!(layout.menu_button_rect.x > layout.add_button_rect.x);
}

#[test]
fn test_recalculate_grid_lid_close_protection() {
    let mut app = celerterm::app::CelerApp::new();
    let initial_cols = app.cols();
    let initial_rows = app.rows();
    assert!(initial_cols >= 20);
    assert!(initial_rows >= 4);

    // Simulate display sleep / lid close (zero or tiny dimensions)
    app.recalculate_grid(0.0, 0.0);
    assert_eq!(app.cols(), initial_cols, "Cols must not shrink to zero/one on zero width");
    assert_eq!(app.rows(), initial_rows, "Rows must not shrink to zero/one on zero height");

    app.recalculate_grid(1.0, 1.0);
    assert_eq!(app.cols(), initial_cols, "Cols must not change on 1x1 dimension");
    assert_eq!(app.rows(), initial_rows, "Rows must not change on 1x1 dimension");

    app.recalculate_grid(50.0, 40.0);
    assert_eq!(app.cols(), initial_cols, "Cols must not change on sub-threshold dimension");
    assert_eq!(app.rows(), initial_rows, "Rows must not change on sub-threshold dimension");

    // Valid resize
    app.recalculate_grid(800.0, 600.0);
    assert!(app.cols() >= 20);
    assert!(app.rows() >= 4);
}

#[test]
fn test_disable_app_nap_safe_execution() {
    celerterm::window::disable_app_nap();
}

#[test]
fn test_live_modifiers() {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSEvent;
        unsafe {
            let flags = NSEvent::modifierFlags_class();
            let raw = flags.0 as usize;
            let shift = (raw & (1 << 17)) != 0;
            let _ = shift;
        }
    }
}

#[test]
fn test_macos_window_helpers_safe_execution() {
    celerterm::window::set_macos_process_name("CelerTerm (Test)");
    celerterm::window::set_macos_dock_badge(Some("Test"));
    celerterm::window::set_macos_dock_badge(None);
    celerterm::window::set_macos_menu_title("CelerTerm (Test)");
}


