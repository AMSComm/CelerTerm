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
    char_width: f32,
) -> TabHeaderLayout {
    if !tabs_in_titlebar {
        return TabHeaderLayout {
            height: 0.0,
            tab_rects: Vec::new(),
            add_button_rect: Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
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
    let available_w = (window_width - start_x - btn_width - (16.0 * scale)).max(10.0);
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
    }
}
