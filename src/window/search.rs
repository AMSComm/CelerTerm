use crate::window::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SearchBarLayout {
    pub container: Rect,
    pub input_box: Rect,
    pub counter_box: Rect,
    pub prev_btn: Rect,
    pub next_btn: Rect,
    pub close_btn: Rect,
}

pub fn calculate_search_bar_layout(
    pane_rect: Rect,
    scale: f32,
) -> SearchBarLayout {
    let margin = (8.0 * scale).round();
    let bar_h = (32.0 * scale).round();
    let bar_w = (340.0 * scale).round().min((pane_rect.width - margin * 2.0).max(120.0 * scale));

    let x = (pane_rect.x + pane_rect.width - bar_w - margin).max(pane_rect.x);
    let y = (pane_rect.y + margin).max(pane_rect.y);

    let container = Rect { x, y, width: bar_w, height: bar_h };

    let btn_w = (24.0 * scale).round();
    let btn_h = bar_h - (4.0 * scale).round();
    let btn_y = y + (2.0 * scale).round();

    let close_btn = Rect {
        x: x + bar_w - btn_w - (4.0 * scale).round(),
        y: btn_y,
        width: btn_w,
        height: btn_h,
    };

    let next_btn = Rect {
        x: close_btn.x - btn_w - (2.0 * scale).round(),
        y: btn_y,
        width: btn_w,
        height: btn_h,
    };

    let prev_btn = Rect {
        x: next_btn.x - btn_w - (2.0 * scale).round(),
        y: btn_y,
        width: btn_w,
        height: btn_h,
    };

    let counter_w = (56.0 * scale).round();
    let counter_box = Rect {
        x: prev_btn.x - counter_w - (4.0 * scale).round(),
        y: btn_y,
        width: counter_w,
        height: btn_h,
    };

    let input_x = x + (6.0 * scale).round();
    let input_w = (counter_box.x - input_x - (4.0 * scale).round()).max(30.0 * scale);
    let input_box = Rect {
        x: input_x,
        y: btn_y,
        width: input_w,
        height: btn_h,
    };

    SearchBarLayout {
        container,
        input_box,
        counter_box,
        prev_btn,
        next_btn,
        close_btn,
    }
}
