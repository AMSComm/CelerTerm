#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= (self.x + self.width) && py >= self.y && py <= (self.y + self.height)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabHeaderLayout {
    pub height: f32,
    pub tab_rects: Vec<(String, Rect)>, // tab_id, bounding rect
    pub add_button_rect: Rect,
    pub menu_button_rect: Rect,
}

pub fn calculate_header_layout(
    window_width: f32,
    tabs: &[(String, String)], // (id, title)
    hide_traffic_lights: bool,
    tabs_in_titlebar: bool,
    scale_factor: f32,
    char_width: f32,
) -> TabHeaderLayout {
    if !tabs_in_titlebar {
        return TabHeaderLayout {
            height: 0.0,
            tab_rects: Vec::new(),
            add_button_rect: Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
            menu_button_rect: Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
        };
    }

    let scale = scale_factor.max(1.0);
    // Compact, space-saving height like WezTerm (26px scaled)
    let height = (26.0 * scale).round();
    // On macOS, traffic lights take ~78-82pt from the left when visible.
    let start_x = if cfg!(target_os = "macos") && !hide_traffic_lights {
        (82.0 * scale).round()
    } else {
        (8.0 * scale).round()
    };

    let btn_width = (24.0 * scale).round();
    let menu_btn_w = (26.0 * scale).round();
    let menu_btn_x = (window_width - menu_btn_w - (8.0 * scale)).round().max(0.0);
    let menu_button_rect = Rect {
        x: menu_btn_x,
        y: (1.0 * scale).round(),
        width: menu_btn_w,
        height: height - (2.0 * scale).round(),
    };

    let available_w = (window_width - start_x - btn_width - menu_btn_w - (60.0 * scale)).max(10.0);
    let max_per_tab = (available_w / (tabs.len().max(1) as f32)).max(20.0);

    let mut current_x = start_x;
    let mut tab_rects = Vec::new();

    for (id, title) in tabs {
        let text_chars = title.chars().count().max(1);
        let text_w = (text_chars as f32 * char_width).round();
        // Dynamic width: text width + close button area & margins (32px scaled)
        let desired_w = (text_w + (32.0 * scale).round()).clamp(50.0 * scale, 240.0 * scale);
        let tab_w = desired_w.min(max_per_tab);

        let rect = Rect {
            x: current_x,
            y: (1.0 * scale).round(),
            width: tab_w,
            height: height - (2.0 * scale).round(),
        };
        tab_rects.push((id.clone(), rect));
        current_x += tab_w;
    }

    let add_button_rect = Rect {
        x: current_x + (4.0 * scale).round(),
        y: (1.0 * scale).round(),
        width: btn_width,
        height: height - (2.0 * scale).round(),
    };

    TabHeaderLayout {
        height,
        tab_rects,
        add_button_rect,
        menu_button_rect,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModalButton {
    pub id: &'static str,
    pub label: &'static str,
    pub rect: Rect,
    pub color: u32,
}

pub fn calculate_modal_buttons(
    modal_rect: Rect,
    footer_h: f32,
    scale: f32,
    cell_w: f32,
) -> [ModalButton; 6] {
    let btn_h = (footer_h - 14.0 * scale).max(20.0);
    let btn_y = modal_rect.y + modal_rect.height - footer_h + ((footer_h - btn_h) * 0.5);

    let items: [(&'static str, &'static str, u32); 6] = [
        ("new", "[n] New", 0x007AA2F7),
        ("rename", "[r] Rename", 0x007AA2F7),
        ("color", "[c] Color", 0x00E0AF68),
        ("delete", "[d] Delete", 0x00F7768E),
        ("window", "[w] Window", 0x00BB9AF7),
        ("switch", "[Enter] Switch", 0x009ECE6A),
    ];

    let pad_inner = 5.0 * scale;
    let mut gap = 4.0 * scale;

    let mut widths = [0.0f32; 6];
    let mut total_w = 0.0f32;
    for (i, (_, text, _)) in items.iter().enumerate() {
        let w = (text.chars().count() as f32 * cell_w) + pad_inner * 2.0;
        widths[i] = w;
        total_w += w;
    }
    total_w += gap * 5.0;

    let avail_w = modal_rect.width - (16.0 * scale);
    if total_w > avail_w && avail_w > 100.0 {
        let factor = avail_w / total_w;
        for w in &mut widths {
            *w *= factor;
        }
        gap *= factor;
        total_w = avail_w;
    }

    let start_x = modal_rect.x + ((modal_rect.width - total_w) * 0.5).max(8.0 * scale);

    let mut cur_x = start_x;
    [
        ModalButton {
            id: items[0].0,
            label: items[0].1,
            rect: Rect { x: cur_x, y: btn_y, width: widths[0], height: btn_h },
            color: items[0].2,
        },
        {
            cur_x += widths[0] + gap;
            ModalButton {
                id: items[1].0,
                label: items[1].1,
                rect: Rect { x: cur_x, y: btn_y, width: widths[1], height: btn_h },
                color: items[1].2,
            }
        },
        {
            cur_x += widths[1] + gap;
            ModalButton {
                id: items[2].0,
                label: items[2].1,
                rect: Rect { x: cur_x, y: btn_y, width: widths[2], height: btn_h },
                color: items[2].2,
            }
        },
        {
            cur_x += widths[2] + gap;
            ModalButton {
                id: items[3].0,
                label: items[3].1,
                rect: Rect { x: cur_x, y: btn_y, width: widths[3], height: btn_h },
                color: items[3].2,
            }
        },
        {
            cur_x += widths[3] + gap;
            ModalButton {
                id: items[4].0,
                label: items[4].1,
                rect: Rect { x: cur_x, y: btn_y, width: widths[4], height: btn_h },
                color: items[4].2,
            }
        },
        {
            cur_x += widths[4] + gap;
            ModalButton {
                id: items[5].0,
                label: items[5].1,
                rect: Rect { x: cur_x, y: btn_y, width: widths[5], height: btn_h },
                color: items[5].2,
            }
        },
    ]
}

pub fn calculate_confirm_delete_buttons(
    modal_rect: Rect,
    footer_h: f32,
    scale: f32,
    cell_w: f32,
) -> [ModalButton; 2] {
    let btn_h = (footer_h - 14.0 * scale).max(20.0);
    let btn_y = modal_rect.y + modal_rect.height - footer_h + ((footer_h - btn_h) * 0.5);

    let items: [(&'static str, &'static str, u32); 2] = [
        ("confirm_delete", "[y] Confirm Delete", 0x00F7768E),
        ("cancel_delete", "[Esc / n] Cancel", 0x007AA2F7),
    ];

    let pad_inner = 8.0 * scale;
    let mut gap = 12.0 * scale;

    let mut widths = [0.0f32; 2];
    let mut total_w = 0.0f32;
    for (i, (_, text, _)) in items.iter().enumerate() {
        let w = (text.chars().count() as f32 * cell_w) + pad_inner * 2.0;
        widths[i] = w;
        total_w += w;
    }
    total_w += gap;

    let avail_w = modal_rect.width - (16.0 * scale);
    if total_w > avail_w && avail_w > 100.0 {
        let factor = avail_w / total_w;
        for w in &mut widths {
            *w *= factor;
        }
        gap *= factor;
        total_w = avail_w;
    }

    let start_x = modal_rect.x + ((modal_rect.width - total_w) * 0.5).max(8.0 * scale);
    let mut cur_x = start_x;

    [
        ModalButton {
            id: items[0].0,
            label: items[0].1,
            rect: Rect { x: cur_x, y: btn_y, width: widths[0], height: btn_h },
            color: items[0].2,
        },
        {
            cur_x += widths[0] + gap;
            ModalButton {
                id: items[1].0,
                label: items[1].1,
                rect: Rect { x: cur_x, y: btn_y, width: widths[1], height: btn_h },
                color: items[1].2,
            }
        },
    ]
}

pub fn calculate_update_modal_buttons(
    modal_rect: Rect,
    footer_h: f32,
    scale: f32,
    cell_w: f32,
    is_available: bool,
) -> [ModalButton; 3] {
    let primary_text = if is_available { "[Enter] Download" } else { "[Enter] Check Again" };
    calculate_update_modal_buttons_with_label(modal_rect, footer_h, scale, cell_w, primary_text, 0x007AA2F7)
}

pub fn calculate_update_modal_buttons_with_label(
    modal_rect: Rect,
    footer_h: f32,
    scale: f32,
    cell_w: f32,
    primary_text: &'static str,
    primary_color: u32,
) -> [ModalButton; 3] {
    let btn_h = (footer_h - 14.0 * scale).max(20.0);
    let btn_y = modal_rect.y + modal_rect.height - footer_h + ((footer_h - btn_h) * 0.5);

    let items: [(&'static str, &'static str, u32); 3] = [
        ("primary", primary_text, primary_color),
        ("github", "[g] GitHub", 0x00BB9AF7),
        ("close", "[Esc] Close", 0x00565F89),
    ];

    let pad_inner = 8.0 * scale;
    let mut gap = 8.0 * scale;
    let mut widths = [0.0f32; 3];
    let mut total_w = 0.0f32;

    for (i, (_, text, _)) in items.iter().enumerate() {
        let w = (text.chars().count() as f32 * cell_w) + pad_inner * 2.0;
        widths[i] = w;
        total_w += w;
    }
    total_w += gap * 2.0;

    let avail_w = modal_rect.width - (16.0 * scale);
    if total_w > avail_w && avail_w > 100.0 {
        let factor = avail_w / total_w;
        for w in &mut widths {
            *w *= factor;
        }
        gap *= factor;
        total_w = avail_w;
    }

    let start_x = modal_rect.x + ((modal_rect.width - total_w) * 0.5).max(8.0 * scale);
    let mut cur_x = start_x;

    [
        ModalButton {
            id: items[0].0,
            label: items[0].1,
            rect: Rect { x: cur_x, y: btn_y, width: widths[0], height: btn_h },
            color: items[0].2,
        },
        {
            cur_x += widths[0] + gap;
            ModalButton {
                id: items[1].0,
                label: items[1].1,
                rect: Rect { x: cur_x, y: btn_y, width: widths[1], height: btn_h },
                color: items[1].2,
            }
        },
        {
            cur_x += widths[1] + gap;
            ModalButton {
                id: items[2].0,
                label: items[2].1,
                rect: Rect { x: cur_x, y: btn_y, width: widths[2], height: btn_h },
                color: items[2].2,
            }
        },
    ]
}
