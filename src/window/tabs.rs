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
) -> TabHeaderLayout {
    if !tabs_in_titlebar {
        return TabHeaderLayout {
            height: 0.0,
            tab_rects: Vec::new(),
            add_button_rect: Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
        };
    }

    let height = 34.0;
    // On macOS, traffic lights take ~78px from the left when visible.
    let start_x = if cfg!(target_os = "macos") && !hide_traffic_lights {
        80.0
    } else {
        8.0
    };

    let mut tab_rects = Vec::new();
    let tab_width = 160.0f32.min((window_width - start_x - 40.0) / (tabs.len().max(1) as f32));
    let mut current_x = start_x;

    for (id, _) in tabs {
        let rect = Rect {
            x: current_x,
            y: 4.0,
            width: tab_width - 4.0,
            height: height - 8.0,
        };
        tab_rects.push((id.clone(), rect));
        current_x += tab_width;
    }

    let add_button_rect = Rect {
        x: current_x + 4.0,
        y: 4.0,
        width: 24.0,
        height: height - 8.0,
    };

    TabHeaderLayout {
        height,
        tab_rects,
        add_button_rect,
    }
}
