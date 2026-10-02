use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal, // Top / Bottom
    Vertical,   // Left / Right
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaneInfo {
    pub id: String,
    pub title: String,
    pub cwd: PathBuf,
    #[serde(default)]
    pub scrollback_cache: Vec<String>,
}

impl PaneInfo {
    pub fn new(id: impl Into<String>, title: impl Into<String>, cwd: PathBuf) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            cwd,
            scrollback_cache: Vec::new(),
        }
    }
}

fn default_ratio() -> f32 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum PaneNode {
    Leaf {
        pane: PaneInfo,
    },
    Split {
        direction: SplitDirection,
        #[serde(default = "default_ratio")]
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

impl Eq for PaneNode {}

#[derive(Debug, Clone, PartialEq)]
pub struct PaneRect {
    pub pane_id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub cols: usize,
    pub rows: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DividerRect {
    pub direction: SplitDirection,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub divider_id: usize,
    pub bounds_x: f32,
    pub bounds_y: f32,
    pub bounds_w: f32,
    pub bounds_h: f32,
}

impl PaneNode {
    pub fn new_leaf(pane: PaneInfo) -> Self {
        PaneNode::Leaf { pane }
    }

    pub fn all_panes(&self) -> Vec<&PaneInfo> {
        let mut result = Vec::new();
        self.collect_panes(&mut result);
        result
    }

    fn collect_panes<'a>(&'a self, result: &mut Vec<&'a PaneInfo>) {
        match self {
            PaneNode::Leaf { pane } => result.push(pane),
            PaneNode::Split { first, second, .. } => {
                first.collect_panes(result);
                second.collect_panes(result);
            }
        }
    }

    pub fn all_panes_mut(&mut self) -> Vec<&mut PaneInfo> {
        let mut result = Vec::new();
        self.collect_panes_mut(&mut result);
        result
    }

    fn collect_panes_mut<'a>(&'a mut self, result: &mut Vec<&'a mut PaneInfo>) {
        match self {
            PaneNode::Leaf { pane } => result.push(pane),
            PaneNode::Split { first, second, .. } => {
                first.collect_panes_mut(result);
                second.collect_panes_mut(result);
            }
        }
    }

    pub fn count_panes(&self) -> usize {
        match self {
            PaneNode::Leaf { .. } => 1,
            PaneNode::Split { first, second, .. } => first.count_panes() + second.count_panes(),
        }
    }

    pub fn find_pane(&self, id: &str) -> Option<&PaneInfo> {
        match self {
            PaneNode::Leaf { pane } => {
                if pane.id == id {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Split { first, second, .. } => {
                first.find_pane(id).or_else(|| second.find_pane(id))
            }
        }
    }

    pub fn find_pane_mut(&mut self, id: &str) -> Option<&mut PaneInfo> {
        match self {
            PaneNode::Leaf { pane } => {
                if pane.id == id {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Split { first, second, .. } => {
                if let Some(p) = first.find_pane_mut(id) {
                    Some(p)
                } else {
                    second.find_pane_mut(id)
                }
            }
        }
    }

    pub fn split_pane(&mut self, target_id: &str, direction: SplitDirection, new_pane: PaneInfo) -> bool {
        match self {
            PaneNode::Leaf { pane } => {
                if pane.id == target_id {
                    let old_leaf = PaneNode::Leaf { pane: pane.clone() };
                    let new_leaf = PaneNode::Leaf { pane: new_pane };
                    *self = PaneNode::Split {
                        direction,
                        ratio: 0.5,
                        first: Box::new(old_leaf),
                        second: Box::new(new_leaf),
                    };
                    true
                } else {
                    false
                }
            }
            PaneNode::Split { first, second, .. } => {
                if first.split_pane(target_id, direction, new_pane.clone()) {
                    true
                } else {
                    second.split_pane(target_id, direction, new_pane)
                }
            }
        }
    }

    pub fn remove_pane(&mut self, target_id: &str) -> Option<PaneInfo> {
        // Can't remove if root is a leaf that matches because a tree must have at least one pane.
        if let PaneNode::Leaf { pane } = self {
            if pane.id == target_id {
                return None;
            }
        }

        self.remove_child(target_id)
    }

    fn remove_child(&mut self, target_id: &str) -> Option<PaneInfo> {
        match self {
            PaneNode::Leaf { .. } => None,
            PaneNode::Split { first, second, .. } => {
                // If first child is the matching leaf: replace self with second
                if let PaneNode::Leaf { pane } = &**first {
                    if pane.id == target_id {
                        let removed = pane.clone();
                        let replacement = (**second).clone();
                        *self = replacement;
                        return Some(removed);
                    }
                }

                // If second child is the matching leaf: replace self with first
                if let PaneNode::Leaf { pane } = &**second {
                    if pane.id == target_id {
                        let removed = pane.clone();
                        let replacement = (**first).clone();
                        *self = replacement;
                        return Some(removed);
                    }
                }

                // Recursively check deeper
                if let Some(removed) = first.remove_child(target_id) {
                    Some(removed)
                } else {
                    second.remove_child(target_id)
                }
            }
        }
    }

    pub fn next_pane_id(&self, current_id: &str) -> Option<String> {
        let panes = self.all_panes();
        if panes.is_empty() {
            return None;
        }
        let pos = panes.iter().position(|p| p.id == current_id)?;
        let next_pos = (pos + 1) % panes.len();
        Some(panes[next_pos].id.clone())
    }

    pub fn previous_pane_id(&self, current_id: &str) -> Option<String> {
        let panes = self.all_panes();
        if panes.is_empty() {
            return None;
        }
        let pos = panes.iter().position(|p| p.id == current_id)?;
        let prev_pos = if pos == 0 { panes.len() - 1 } else { pos - 1 };
        Some(panes[prev_pos].id.clone())
    }

    pub fn calculate_layout(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        cell_w: f32,
        cell_h: f32,
    ) -> (Vec<PaneRect>, Vec<DividerRect>) {
        let mut panes = Vec::new();
        let mut dividers = Vec::new();
        let mut divider_id_counter = 0;
        self.compute_layout_recursive(
            x,
            y,
            width,
            height,
            cell_w,
            cell_h,
            &mut panes,
            &mut dividers,
            &mut divider_id_counter,
        );
        (panes, dividers)
    }

    fn compute_layout_recursive(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        cell_w: f32,
        cell_h: f32,
        panes: &mut Vec<PaneRect>,
        dividers: &mut Vec<DividerRect>,
        divider_id_counter: &mut usize,
    ) {
        match self {
            PaneNode::Leaf { pane } => {
                let cols = if cell_w > 0.0 {
                    (width / cell_w).floor().max(1.0) as usize
                } else {
                    80
                };
                let rows = if cell_h > 0.0 {
                    (height / cell_h).floor().max(1.0) as usize
                } else {
                    24
                };
                panes.push(PaneRect {
                    pane_id: pane.id.clone(),
                    x,
                    y,
                    width,
                    height,
                    cols,
                    rows,
                });
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let current_div_id = *divider_id_counter;
                *divider_id_counter += 1;
                let clamped_ratio = ratio.clamp(0.1, 0.9);

                const DIVIDER_SIZE: f32 = 1.0;

                match direction {
                    SplitDirection::Vertical => {
                        let available_w = (width - DIVIDER_SIZE).max(0.0);
                        let w1 = (available_w * clamped_ratio).round();
                        let w2 = (available_w - w1).max(0.0);
                        let div_x = x + w1;

                        dividers.push(DividerRect {
                            direction: *direction,
                            x: div_x,
                            y,
                            width: DIVIDER_SIZE,
                            height,
                            divider_id: current_div_id,
                            bounds_x: x,
                            bounds_y: y,
                            bounds_w: width,
                            bounds_h: height,
                        });

                        first.compute_layout_recursive(
                            x,
                            y,
                            w1,
                            height,
                            cell_w,
                            cell_h,
                            panes,
                            dividers,
                            divider_id_counter,
                        );
                        second.compute_layout_recursive(
                            div_x + DIVIDER_SIZE,
                            y,
                            w2,
                            height,
                            cell_w,
                            cell_h,
                            panes,
                            dividers,
                            divider_id_counter,
                        );
                    }
                    SplitDirection::Horizontal => {
                        let available_h = (height - DIVIDER_SIZE).max(0.0);
                        let h1 = (available_h * clamped_ratio).round();
                        let h2 = (available_h - h1).max(0.0);
                        let div_y = y + h1;

                        dividers.push(DividerRect {
                            direction: *direction,
                            x,
                            y: div_y,
                            width,
                            height: DIVIDER_SIZE,
                            divider_id: current_div_id,
                            bounds_x: x,
                            bounds_y: y,
                            bounds_w: width,
                            bounds_h: height,
                        });

                        first.compute_layout_recursive(
                            x,
                            y,
                            width,
                            h1,
                            cell_w,
                            cell_h,
                            panes,
                            dividers,
                            divider_id_counter,
                        );
                        second.compute_layout_recursive(
                            x,
                            div_y + DIVIDER_SIZE,
                            width,
                            h2,
                            cell_w,
                            cell_h,
                            panes,
                            dividers,
                            divider_id_counter,
                        );
                    }
                }
            }
        }
    }

    pub fn can_split(
        &self,
        target_id: &str,
        direction: SplitDirection,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        cell_w: f32,
        cell_h: f32,
        min_cols: usize,
        min_rows: usize,
    ) -> bool {
        let (panes, _) = self.calculate_layout(x, y, width, height, cell_w, cell_h);
        if let Some(target_rect) = panes.iter().find(|p| p.pane_id == target_id) {
            match direction {
                SplitDirection::Vertical => {
                    let half_w = (target_rect.width - 1.0) * 0.5;
                    let half_cols = (half_w / cell_w).floor() as usize;
                    half_cols >= min_cols && target_rect.rows >= min_rows
                }
                SplitDirection::Horizontal => {
                    let half_h = (target_rect.height - 1.0) * 0.5;
                    let half_rows = (half_h / cell_h).floor() as usize;
                    target_rect.cols >= min_cols && half_rows >= min_rows
                }
            }
        } else {
            false
        }
    }

    pub fn adjust_ratio_by_id(&mut self, target_div_id: usize, new_ratio: f32) -> bool {
        let mut current_id = 0;
        self.adjust_ratio_recursive(target_div_id, new_ratio, &mut current_id)
    }

    fn adjust_ratio_recursive(&mut self, target_div_id: usize, new_ratio: f32, current_id: &mut usize) -> bool {
        match self {
            PaneNode::Leaf { .. } => false,
            PaneNode::Split { ratio, first, second, .. } => {
                if *current_id == target_div_id {
                    *ratio = new_ratio.clamp(0.1, 0.9);
                    return true;
                }
                *current_id += 1;
                if first.adjust_ratio_recursive(target_div_id, new_ratio, current_id) {
                    true
                } else {
                    second.adjust_ratio_recursive(target_div_id, new_ratio, current_id)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leaf_creation_and_counting() {
        let pane = PaneInfo::new("p1", "Shell", PathBuf::from("/tmp"));
        let node = PaneNode::new_leaf(pane.clone());
        assert_eq!(node.count_panes(), 1);
        assert_eq!(node.find_pane("p1"), Some(&pane));
        assert_eq!(node.all_panes().len(), 1);
    }

    #[test]
    fn test_split_vertical_and_horizontal() {
        let mut tree = PaneNode::new_leaf(PaneInfo::new("p1", "Shell 1", PathBuf::from("/tmp")));
        assert!(tree.split_pane(
            "p1",
            SplitDirection::Vertical,
            PaneInfo::new("p2", "Shell 2", PathBuf::from("/tmp"))
        ));
        assert_eq!(tree.count_panes(), 2);

        // Split p2 horizontally
        assert!(tree.split_pane(
            "p2",
            SplitDirection::Horizontal,
            PaneInfo::new("p3", "Shell 3", PathBuf::from("/tmp"))
        ));
        assert_eq!(tree.count_panes(), 3);

        let ids: Vec<String> = tree.all_panes().into_iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids, vec!["p1", "p2", "p3"]);
    }

    #[test]
    fn test_remove_pane() {
        let mut tree = PaneNode::new_leaf(PaneInfo::new("p1", "Shell 1", PathBuf::from("/tmp")));
        tree.split_pane(
            "p1",
            SplitDirection::Vertical,
            PaneInfo::new("p2", "Shell 2", PathBuf::from("/tmp")),
        );
        tree.split_pane(
            "p2",
            SplitDirection::Horizontal,
            PaneInfo::new("p3", "Shell 3", PathBuf::from("/tmp")),
        );
        assert_eq!(tree.count_panes(), 3);

        // Remove p2
        let removed = tree.remove_pane("p2");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "p2");
        assert_eq!(tree.count_panes(), 2);

        let ids: Vec<String> = tree.all_panes().into_iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids, vec!["p1", "p3"]);

        // Cannot remove the only remaining pane from a 1-leaf tree
        let mut single = PaneNode::new_leaf(PaneInfo::new("p1", "Shell 1", PathBuf::from("/tmp")));
        assert!(single.remove_pane("p1").is_none());
    }

    #[test]
    fn test_navigation_next_and_previous() {
        let mut tree = PaneNode::new_leaf(PaneInfo::new("p1", "Shell 1", PathBuf::from("/tmp")));
        tree.split_pane(
            "p1",
            SplitDirection::Vertical,
            PaneInfo::new("p2", "Shell 2", PathBuf::from("/tmp")),
        );
        tree.split_pane(
            "p2",
            SplitDirection::Horizontal,
            PaneInfo::new("p3", "Shell 3", PathBuf::from("/tmp")),
        );

        assert_eq!(tree.next_pane_id("p1"), Some("p2".to_string()));
        assert_eq!(tree.next_pane_id("p2"), Some("p3".to_string()));
        assert_eq!(tree.next_pane_id("p3"), Some("p1".to_string())); // wraps around

        assert_eq!(tree.previous_pane_id("p1"), Some("p3".to_string())); // wraps around
        assert_eq!(tree.previous_pane_id("p2"), Some("p1".to_string()));
        assert_eq!(tree.previous_pane_id("p3"), Some("p2".to_string()));
    }

    #[test]
    fn test_calculate_layout_dimensions() {
        let mut tree = PaneNode::new_leaf(PaneInfo::new("p1", "Shell 1", PathBuf::from("/tmp")));
        tree.split_pane(
            "p1",
            SplitDirection::Vertical,
            PaneInfo::new("p2", "Shell 2", PathBuf::from("/tmp")),
        );

        let cell_w = 10.0;
        let cell_h = 20.0;
        let (panes, dividers) = tree.calculate_layout(0.0, 0.0, 1001.0, 600.0, cell_w, cell_h);

        assert_eq!(panes.len(), 2);
        assert_eq!(dividers.len(), 1);

        // 1001 - 1 = 1000 width available. 500 each.
        assert_eq!(panes[0].pane_id, "p1");
        assert_eq!(panes[0].x, 0.0);
        assert_eq!(panes[0].width, 500.0);
        assert_eq!(panes[0].cols, 50);

        assert_eq!(dividers[0].x, 500.0);
        assert_eq!(dividers[0].width, 1.0);
        assert_eq!(dividers[0].bounds_x, 0.0);
        assert_eq!(dividers[0].bounds_w, 1001.0);

        assert_eq!(panes[1].pane_id, "p2");
        assert_eq!(panes[1].x, 501.0);
        assert_eq!(panes[1].width, 500.0);
        assert_eq!(panes[1].cols, 50);
    }

    #[test]
    fn test_adjust_ratio_and_layout() {
        let mut tree = PaneNode::new_leaf(PaneInfo::new("p1", "Shell 1", PathBuf::from("/tmp")));
        tree.split_pane(
            "p1",
            SplitDirection::Vertical,
            PaneInfo::new("p2", "Shell 2", PathBuf::from("/tmp")),
        );

        assert!(tree.adjust_ratio_by_id(0, 0.3));

        let cell_w = 10.0;
        let cell_h = 20.0;
        let (panes, dividers) = tree.calculate_layout(0.0, 0.0, 1001.0, 600.0, cell_w, cell_h);

        assert_eq!(panes.len(), 2);
        assert_eq!(dividers.len(), 1);
        // available = 1000. w1 = round(1000 * 0.3) = 300.
        assert_eq!(panes[0].width, 300.0);
        assert_eq!(panes[1].width, 700.0);
    }
}
