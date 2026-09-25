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
}

pub fn calculate_header_layout(
    window_width: f32,
    tabs: &[(String, String)], // (id, title)
    hide_traffic_lights: bool,
    tabs_in_titlebar: bool,
    scale_factor: f32,
) -> TabHeaderLayout {
    if !tabs_in_titlebar {
        return TabHeaderLayout {
            height: 0.0,
            tab_rects: Vec::new(),
            add_button_rect: Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
        };
    }

    let scale = scale_factor.max(1.0);
    let height = (38.0 * scale).round();
    // On macOS, traffic lights take ~78-82pt from the left when visible.
    let start_x = if cfg!(target_os = "macos") && !hide_traffic_lights {
        (82.0 * scale).round()
    } else {
        (8.0 * scale).round()
    };

    let tab_padding = (4.0 * scale).round();
    let btn_width = (28.0 * scale).round();
    let max_tab_width = (180.0 * scale).round();
    let available_w = (window_width - start_x - btn_width - tab_padding * 2.0).max(10.0);
    let tab_width = max_tab_width.min(available_w / (tabs.len().max(1) as f32));
    let mut current_x = start_x;
    let mut tab_rects = Vec::new();

    for (id, _) in tabs {
        let rect = Rect {
            x: current_x,
            y: tab_padding,
            width: (tab_width - tab_padding).max(10.0),
            height: height - tab_padding * 2.0,
        };
        tab_rects.push((id.clone(), rect));
        current_x += tab_width;
    }

    let add_button_rect = Rect {
        x: current_x + tab_padding,
        y: tab_padding,
        width: btn_width,
        height: height - tab_padding * 2.0,
    };

    TabHeaderLayout {
        height,
        tab_rects,
        add_button_rect,
    }
}
