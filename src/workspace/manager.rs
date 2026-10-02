use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use super::pane::{PaneInfo, PaneNode, SplitDirection};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tab {
    pub id: String,
    pub title: String,
    pub cwd: PathBuf,
    pub scrollback_cache: Vec<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub pane_tree: Option<PaneNode>,
    #[serde(default)]
    pub active_pane_id: Option<String>,
    #[serde(default)]
    pub is_zoomed: bool,
}

impl Tab {
    pub fn new(id: impl Into<String>, title: impl Into<String>, cwd: PathBuf) -> Self {
        let id_str = id.into();
        let title_str = title.into();
        let leaf = PaneNode::new_leaf(PaneInfo::new(&id_str, &title_str, cwd.clone()));
        Self {
            id: id_str.clone(),
            title: title_str,
            cwd,
            scrollback_cache: Vec::new(),
            color: None,
            pane_tree: Some(leaf),
            active_pane_id: Some(id_str),
            is_zoomed: false,
        }
    }

    pub fn new_with_color(
        id: impl Into<String>,
        title: impl Into<String>,
        cwd: PathBuf,
        color: Option<String>,
    ) -> Self {
        let mut tab = Self::new(id, title, cwd);
        tab.color = color;
        tab
    }

    pub fn ensure_pane_tree(&mut self) -> &mut PaneNode {
        if self.pane_tree.is_none() {
            let pane = PaneInfo {
                id: self.id.clone(),
                title: self.title.clone(),
                cwd: self.cwd.clone(),
                scrollback_cache: self.scrollback_cache.clone(),
            };
            self.pane_tree = Some(PaneNode::new_leaf(pane));
            self.active_pane_id = Some(self.id.clone());
        }
        self.pane_tree.as_mut().unwrap()
    }

    pub fn pane_tree(&self) -> PaneNode {
        if let Some(ref tree) = self.pane_tree {
            tree.clone()
        } else {
            PaneNode::new_leaf(PaneInfo {
                id: self.id.clone(),
                title: self.title.clone(),
                cwd: self.cwd.clone(),
                scrollback_cache: self.scrollback_cache.clone(),
            })
        }
    }

    pub fn active_pane_id(&self) -> String {
        self.active_pane_id
            .clone()
            .unwrap_or_else(|| self.id.clone())
    }

    pub fn all_pane_ids(&self) -> Vec<String> {
        if let Some(ref tree) = self.pane_tree {
            tree.all_panes().into_iter().map(|p| p.id.clone()).collect()
        } else {
            vec![self.id.clone()]
        }
    }

    pub fn effective_color_u32(&self, fallback: u32) -> u32 {
        if let Some(ref c) = self.color {
            crate::renderer::color::parse_hex_color(c, fallback)
        } else {
            fallback
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    pub tabs: Vec<Tab>,
    pub active_tab_id: String,
}

impl Workspace {
    pub fn new(id: impl Into<String>, name: impl Into<String>, initial_cwd: PathBuf) -> Self {
        let initial_tab_id = format!("{}_tab_1", id.into());
        let tab = Tab::new(&initial_tab_id, "Shell", initial_cwd);
        Self {
            id: initial_tab_id.split('_').next().unwrap_or("ws").to_string(),
            name: name.into(),
            color: None,
            background: None,
            tabs: vec![tab],
            active_tab_id: initial_tab_id,
        }
    }

    pub fn new_with_color(
        id: impl Into<String>,
        name: impl Into<String>,
        initial_cwd: PathBuf,
        color: Option<String>,
        background: Option<String>,
    ) -> Self {
        let initial_tab_id = format!("{}_tab_1", id.into());
        let tab = Tab::new(&initial_tab_id, "Shell", initial_cwd);
        Self {
            id: initial_tab_id.split('_').next().unwrap_or("ws").to_string(),
            name: name.into(),
            color,
            background,
            tabs: vec![tab],
            active_tab_id: initial_tab_id,
        }
    }

    pub fn effective_color_u32(&self, fallback_idx: usize) -> u32 {
        if let Some(ref c) = self.color {
            crate::renderer::color::parse_hex_color(
                c,
                super::palette::WORKSPACE_ACCENT_PALETTE[fallback_idx % super::palette::WORKSPACE_ACCENT_PALETTE.len()].u32_val,
            )
        } else {
            super::palette::WORKSPACE_ACCENT_PALETTE[fallback_idx % super::palette::WORKSPACE_ACCENT_PALETTE.len()].u32_val
        }
    }

    pub fn effective_color_hex(&self, fallback_idx: usize) -> String {
        self.color.clone().unwrap_or_else(|| {
            super::palette::WORKSPACE_ACCENT_PALETTE[fallback_idx % super::palette::WORKSPACE_ACCENT_PALETTE.len()].hex.to_string()
        })
    }

    pub fn effective_background_u32(&self, default_bg: u32) -> u32 {
        if let Some(ref bg) = self.background {
            crate::renderer::color::parse_hex_color(bg, default_bg)
        } else {
            default_bg
        }
    }

    pub fn get_active_tab(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == self.active_tab_id)
    }

    pub fn get_active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == self.active_tab_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceManager {
    pub workspaces: Vec<Workspace>,
    pub active_workspace_id: String,
    pub next_id: usize,
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceManager {
    pub fn new() -> Self {
        let default_cwd = directories::BaseDirs::new()
            .map(|b| b.home_dir().to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
        let initial_title = crate::pty::format_tab_title(None, Some(&default_cwd), None);
        let default_tab = Tab::new("tab_1", initial_title, default_cwd);
        let default_color = super::palette::WORKSPACE_ACCENT_PALETTE[0].hex.to_string();
        let default_ws = Workspace {
            id: "ws_default".to_string(),
            name: "Default".to_string(),
            color: Some(default_color),
            background: None,
            tabs: vec![default_tab],
            active_tab_id: "tab_1".to_string(),
        };

        Self {
            workspaces: vec![default_ws],
            active_workspace_id: "ws_default".to_string(),
            next_id: 2,
        }
    }

    pub fn pick_distinct_color(&self) -> String {
        let used_colors: std::collections::HashSet<String> = self.workspaces
            .iter()
            .filter_map(|w| w.color.as_ref().map(|c| c.trim().to_lowercase()))
            .collect();

        for palette in super::palette::WORKSPACE_ACCENT_PALETTE {
            if !used_colors.contains(&palette.hex.to_lowercase()) {
                return palette.hex.to_string();
            }
        }

        // If all 20 colors are used, cycle by index
        let idx = self.workspaces.len() % super::palette::WORKSPACE_ACCENT_PALETTE.len();
        super::palette::WORKSPACE_ACCENT_PALETTE[idx].hex.to_string()
    }

    pub fn ensure_distinct_colors(&mut self) {
        for i in 0..self.workspaces.len() {
            if self.workspaces[i].color.is_none() {
                let distinct = self.pick_distinct_color();
                self.workspaces[i].color = Some(distinct);
            }
        }
    }

    pub fn get_active_workspace(&self) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == self.active_workspace_id)
    }

    pub fn get_active_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|w| w.id == self.active_workspace_id)
    }

    pub fn new_tab(&mut self, cwd: PathBuf) -> Result<String, String> {
        let tab_id = format!("tab_{}", self.next_id);
        self.next_id += 1;
        let initial_title = crate::pty::format_tab_title(None, Some(&cwd), None);
        let tab = Tab::new(&tab_id, initial_title, cwd);

        let ws = self.get_active_workspace_mut().ok_or("No active workspace found")?;
        ws.tabs.push(tab);
        ws.active_tab_id = tab_id.clone();
        Ok(tab_id)
    }

    pub fn close_tab(&mut self, tab_id: &str) -> Result<(), String> {
        let ws = self.get_active_workspace_mut().ok_or("No active workspace found")?;
        if ws.tabs.len() <= 1 {
            return Err("Cannot close last tab in workspace".to_string());
        }

        if let Some(pos) = ws.tabs.iter().position(|t| t.id == tab_id) {
            ws.tabs.remove(pos);
            if ws.active_tab_id == tab_id {
                let new_active_index = if pos >= ws.tabs.len() { pos - 1 } else { pos };
                ws.active_tab_id = ws.tabs[new_active_index].id.clone();
            }
            Ok(())
        } else {
            Err("Tab not found".to_string())
        }
    }

    pub fn new_workspace(&mut self, name: &str) -> Result<String, String> {
        let ws_id = format!("ws_{}", self.next_id);
        let tab_id = format!("tab_{}", self.next_id + 1);
        self.next_id += 2;

        let cwd = directories::BaseDirs::new()
            .map(|b| b.home_dir().to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
        let initial_title = crate::pty::format_tab_title(None, Some(&cwd), None);
        let tab = Tab::new(&tab_id, initial_title, cwd);
        let color = self.pick_distinct_color();
        let ws = Workspace {
            id: ws_id.clone(),
            name: name.to_string(),
            color: Some(color),
            background: None,
            tabs: vec![tab],
            active_tab_id: tab_id,
        };

        self.workspaces.push(ws);
        self.active_workspace_id = ws_id.clone();
        Ok(ws_id)
    }

    pub fn set_workspace_color(&mut self, workspace_id: &str, color: Option<String>) -> Result<(), String> {
        if let Some(ws) = self.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            ws.color = color;
            Ok(())
        } else {
            Err("Workspace not found".to_string())
        }
    }

    pub fn set_workspace_background(&mut self, workspace_id: &str, background: Option<String>) -> Result<(), String> {
        if let Some(ws) = self.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            ws.background = background;
            Ok(())
        } else {
            Err("Workspace not found".to_string())
        }
    }

    pub fn set_tab_color(&mut self, tab_id: &str, color: Option<String>) -> Result<(), String> {
        for ws in &mut self.workspaces {
            if let Some(tab) = ws.tabs.iter_mut().find(|t| t.id == tab_id) {
                tab.color = color;
                return Ok(());
            }
        }
        Err("Tab not found".to_string())
    }

    pub fn switch_workspace(&mut self, workspace_id: &str) -> Result<(), String> {
        if self.workspaces.iter().any(|w| w.id == workspace_id) {
            self.active_workspace_id = workspace_id.to_string();
            Ok(())
        } else {
            Err("Workspace not found".to_string())
        }
    }

    pub fn select_tab_by_1_index(&mut self, index_1_based: usize) -> Result<String, String> {
        if index_1_based == 0 {
            return Err("Tab index must be >= 1".to_string());
        }
        let ws = self.get_active_workspace_mut().ok_or("No active workspace found")?;
        let zero_based = index_1_based - 1;
        if zero_based < ws.tabs.len() {
            let id = ws.tabs[zero_based].id.clone();
            ws.active_tab_id = id.clone();
            Ok(id)
        } else {
            Err(format!("Tab index {} out of bounds (total tabs: {})", index_1_based, ws.tabs.len()))
        }
    }

    pub fn select_next_tab(&mut self) -> Result<String, String> {
        let ws = self.get_active_workspace_mut().ok_or("No active workspace found")?;
        if ws.tabs.is_empty() {
            return Err("No tabs in workspace".to_string());
        }
        let current_pos = ws.tabs.iter().position(|t| t.id == ws.active_tab_id).unwrap_or(0);
        let next_pos = (current_pos + 1) % ws.tabs.len();
        let id = ws.tabs[next_pos].id.clone();
        ws.active_tab_id = id.clone();
        Ok(id)
    }

    pub fn select_previous_tab(&mut self) -> Result<String, String> {
        let ws = self.get_active_workspace_mut().ok_or("No active workspace found")?;
        if ws.tabs.is_empty() {
            return Err("No tabs in workspace".to_string());
        }
        let current_pos = ws.tabs.iter().position(|t| t.id == ws.active_tab_id).unwrap_or(0);
        let prev_pos = if current_pos == 0 {
            ws.tabs.len() - 1
        } else {
            current_pos - 1
        };
        let id = ws.tabs[prev_pos].id.clone();
        ws.active_tab_id = id.clone();
        Ok(id)
    }

    pub fn move_active_tab_left(&mut self) -> Result<usize, String> {
        let ws = self.get_active_workspace_mut().ok_or("No active workspace found")?;
        if ws.tabs.len() <= 1 {
            return Ok(0);
        }
        let current_pos = ws.tabs.iter().position(|t| t.id == ws.active_tab_id).ok_or("Active tab not found")?;
        if current_pos > 0 {
            ws.tabs.swap(current_pos, current_pos - 1);
            Ok(current_pos - 1)
        } else {
            Ok(0)
        }
    }

    pub fn move_active_tab_right(&mut self) -> Result<usize, String> {
        let ws = self.get_active_workspace_mut().ok_or("No active workspace found")?;
        if ws.tabs.len() <= 1 {
            return Ok(0);
        }
        let current_pos = ws.tabs.iter().position(|t| t.id == ws.active_tab_id).ok_or("Active tab not found")?;
        if current_pos + 1 < ws.tabs.len() {
            ws.tabs.swap(current_pos, current_pos + 1);
            Ok(current_pos + 1)
        } else {
            Ok(current_pos)
        }
    }

    pub fn next_workspace(&mut self) -> Result<String, String> {
        if self.workspaces.is_empty() {
            return Err("No workspaces".to_string());
        }
        let current_pos = self.workspaces.iter().position(|w| w.id == self.active_workspace_id).unwrap_or(0);
        let next_pos = (current_pos + 1) % self.workspaces.len();
        let id = self.workspaces[next_pos].id.clone();
        self.active_workspace_id = id.clone();
        Ok(id)
    }

    pub fn previous_workspace(&mut self) -> Result<String, String> {
        if self.workspaces.is_empty() {
            return Err("No workspaces".to_string());
        }
        let current_pos = self.workspaces.iter().position(|w| w.id == self.active_workspace_id).unwrap_or(0);
        let prev_pos = if current_pos == 0 {
            self.workspaces.len() - 1
        } else {
            current_pos - 1
        };
        let id = self.workspaces[prev_pos].id.clone();
        self.active_workspace_id = id.clone();
        Ok(id)
    }

    pub fn rename_workspace(&mut self, workspace_id: &str, new_name: &str) -> Result<(), String> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err("Workspace name cannot be empty".to_string());
        }
        if let Some(ws) = self.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            ws.name = trimmed.to_string();
            Ok(())
        } else {
            Err("Workspace not found".to_string())
        }
    }

    pub fn delete_workspace(&mut self, workspace_id: &str) -> Result<String, String> {
        if self.workspaces.len() <= 1 {
            return Err("Cannot delete the only remaining workspace".to_string());
        }

        if let Some(pos) = self.workspaces.iter().position(|w| w.id == workspace_id) {
            self.workspaces.remove(pos);
            if self.active_workspace_id == workspace_id {
                let new_idx = if pos >= self.workspaces.len() {
                    self.workspaces.len() - 1
                } else {
                    pos
                };
                self.active_workspace_id = self.workspaces[new_idx].id.clone();
            }
            Ok(self.active_workspace_id.clone())
        } else {
            Err("Workspace not found".to_string())
        }
    }

    pub fn split_active_pane(
        &mut self,
        direction: SplitDirection,
        new_pane_id: &str,
        new_title: &str,
        cwd: PathBuf,
    ) -> Result<String, String> {
        let ws = self.get_active_workspace_mut().ok_or("No active workspace found")?;
        let tab = ws.get_active_tab_mut().ok_or("No active tab found")?;
        let active_pane_id = tab.active_pane_id();
        let new_pane = PaneInfo::new(new_pane_id, new_title, cwd);

        tab.ensure_pane_tree();
        let tree = tab.pane_tree.as_mut().unwrap();
        if tree.split_pane(&active_pane_id, direction, new_pane) {
            tab.active_pane_id = Some(new_pane_id.to_string());
            tab.is_zoomed = false;
            Ok(new_pane_id.to_string())
        } else {
            Err("Failed to split active pane: target pane not found".to_string())
        }
    }

    pub fn close_active_pane(&mut self) -> Result<Option<String>, String> {
        let ws = self.get_active_workspace_mut().ok_or("No active workspace found")?;
        let tab = ws.get_active_tab_mut().ok_or("No active tab found")?;
        tab.ensure_pane_tree();

        let active_id = tab.active_pane_id();
        let tree = tab.pane_tree.as_mut().unwrap();
        if tree.count_panes() <= 1 {
            return Ok(None);
        }

        let next_focus = tree.previous_pane_id(&active_id)
            .or_else(|| tree.next_pane_id(&active_id));

        if let Some(removed) = tree.remove_pane(&active_id) {
            tab.active_pane_id = next_focus;
            tab.is_zoomed = false;
            Ok(Some(removed.id))
        } else {
            Err("Failed to remove pane".to_string())
        }
    }

    pub fn toggle_zoom_active_pane(&mut self) -> bool {
        if let Some(ws) = self.get_active_workspace_mut()
            && let Some(tab) = ws.get_active_tab_mut()
        {
            tab.is_zoomed = !tab.is_zoomed;
            tab.is_zoomed
        } else {
            false
        }
    }

    pub fn next_pane(&mut self) -> bool {
        if let Some(ws) = self.get_active_workspace_mut()
            && let Some(tab) = ws.get_active_tab_mut()
        {
            let current = tab.active_pane_id();
            let tree = tab.ensure_pane_tree();
            if let Some(next) = tree.next_pane_id(&current) {
                tab.active_pane_id = Some(next);
                return true;
            }
        }
        false
    }

    pub fn previous_pane(&mut self) -> bool {
        if let Some(ws) = self.get_active_workspace_mut()
            && let Some(tab) = ws.get_active_tab_mut()
        {
            let current = tab.active_pane_id();
            let tree = tab.ensure_pane_tree();
            if let Some(prev) = tree.previous_pane_id(&current) {
                tab.active_pane_id = Some(prev);
                return true;
            }
        }
        false
    }

    pub fn focus_pane(&mut self, pane_id: &str) -> bool {
        if let Some(ws) = self.get_active_workspace_mut()
            && let Some(tab) = ws.get_active_tab_mut()
        {
            let tree = tab.ensure_pane_tree();
            if tree.find_pane(pane_id).is_some() {
                tab.active_pane_id = Some(pane_id.to_string());
                return true;
            }
        }
        false
    }

    pub fn adjust_active_tab_divider(&mut self, divider_id: usize, new_ratio: f32) -> bool {
        if let Some(ws) = self.get_active_workspace_mut()
            && let Some(tab) = ws.get_active_tab_mut()
            && let Some(ref mut tree) = tab.pane_tree
        {
            tree.adjust_ratio_by_id(divider_id, new_ratio)
        } else {
            false
        }
    }
}


