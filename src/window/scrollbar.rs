use crate::window::tabs::Rect;

#[derive(Debug, Clone, PartialEq)]
pub struct ScrollbarLayout {
    pub pane_id: String,
    pub track_rect: Rect,
    pub thumb_rect: Rect,
    pub hit_rect: Rect,
    pub is_scrollable: bool,
    pub history_lines: usize,
    pub display_offset: usize,
}

/// Calculate vertical scrollbar layout for a terminal pane.
#[allow(clippy::too_many_arguments)]
pub fn calculate_scrollbar_layout(
    pane_id: &str,
    pane_x: f32,
    pane_y: f32,
    pane_width: f32,
    pane_height: f32,
    history_lines: usize,
    visible_lines: usize,
    display_offset: usize,
    scale_factor: f32,
) -> ScrollbarLayout {
    let scale = scale_factor.max(0.5);
    let sb_width = (6.0 * scale).round().max(4.0);
    let margin_right = (1.0 * scale).round().max(1.0);
    let margin_vertical = (2.0 * scale).round().max(1.0);

    let track_x = pane_x + pane_width - sb_width - margin_right;
    let track_y = pane_y + margin_vertical;
    let track_w = sb_width;
    let track_h = (pane_height - 2.0 * margin_vertical).max(8.0);

    let hit_margin_x = (4.0 * scale).round();
    let hit_rect = Rect {
        x: track_x - hit_margin_x,
        y: track_y,
        width: track_w + hit_margin_x + margin_right,
        height: track_h,
    };

    let track_rect = Rect {
        x: track_x,
        y: track_y,
        width: track_w,
        height: track_h,
    };

    let total_lines = history_lines + visible_lines;
    let is_scrollable = history_lines > 0;

    let (thumb_y, thumb_h) = if !is_scrollable || total_lines == 0 {
        (track_y, track_h)
    } else {
        let min_thumb_h = (20.0 * scale).round().min(track_h);
        let raw_thumb_h = ((visible_lines as f32 / total_lines as f32) * track_h).round();
        let thumb_h = raw_thumb_h.max(min_thumb_h).min(track_h);

        let travel = track_h - thumb_h;
        let clamped_offset = display_offset.min(history_lines);
        let scroll_progress = 1.0 - (clamped_offset as f32 / history_lines as f32);
        let thumb_y = track_y + (scroll_progress * travel).round();

        (thumb_y, thumb_h)
    };

    let thumb_rect = Rect {
        x: track_x,
        y: thumb_y,
        width: track_w,
        height: thumb_h,
    };

    ScrollbarLayout {
        pane_id: pane_id.to_string(),
        track_rect,
        thumb_rect,
        hit_rect,
        is_scrollable,
        history_lines,
        display_offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_history_layout() {
        let layout = calculate_scrollbar_layout(
            "pane-1",
            10.0,
            20.0,
            800.0,
            600.0,
            0,
            30,
            0,
            1.0,
        );

        assert!(!layout.is_scrollable);
        assert_eq!(layout.history_lines, 0);
        assert_eq!(layout.display_offset, 0);
        assert_eq!(layout.thumb_rect.y, layout.track_rect.y);
        assert_eq!(layout.thumb_rect.height, layout.track_rect.height);
        assert!(layout.hit_rect.contains(layout.track_rect.x, layout.track_rect.y + 10.0));
    }

    #[test]
    fn test_scroll_positions() {
        // At bottom: display_offset == 0
        let bottom = calculate_scrollbar_layout(
            "pane-1",
            0.0,
            0.0,
            500.0,
            400.0,
            100,
            25,
            0,
            1.0,
        );
        assert!(bottom.is_scrollable);
        let bottom_thumb_end = bottom.thumb_rect.y + bottom.thumb_rect.height;
        let track_end = bottom.track_rect.y + bottom.track_rect.height;
        assert!((bottom_thumb_end - track_end).abs() <= 1.0);

        // At top: display_offset == 100
        let top = calculate_scrollbar_layout(
            "pane-1",
            0.0,
            0.0,
            500.0,
            400.0,
            100,
            25,
            100,
            1.0,
        );
        assert!((top.thumb_rect.y - top.track_rect.y).abs() <= 1.0);

        // At midpoint: display_offset == 50
        let mid = calculate_scrollbar_layout(
            "pane-1",
            0.0,
            0.0,
            500.0,
            400.0,
            100,
            25,
            50,
            1.0,
        );
        let mid_expected = top.track_rect.y + (top.track_rect.height - top.thumb_rect.height) * 0.5;
        assert!((mid.thumb_rect.y - mid_expected).abs() <= 1.5);
    }

    #[test]
    fn test_huge_history_min_thumb_height() {
        let layout = calculate_scrollbar_layout(
            "pane-1",
            0.0,
            0.0,
            500.0,
            400.0,
            100_000,
            25,
            0,
            1.0,
        );
        assert_eq!(layout.thumb_rect.height, 20.0);
    }
}
