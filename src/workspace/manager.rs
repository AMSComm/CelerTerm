use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tab {
    pub id: String,
    pub title: String,
    pub cwd: PathBuf,
    pub scrollback_cache: Vec<String>,
}

impl Tab {
    pub fn new(id: impl Into<String>, title: impl Into<String>, cwd: PathBuf) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            cwd,
            scrollback_cache: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
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
            tabs: vec![tab],
            active_tab_id: initial_tab_id,
        }
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
        let default_ws = Workspace {
            id: "ws_default".to_string(),
            name: "Default".to_string(),
            tabs: vec![default_tab],
            active_tab_id: "tab_1".to_string(),
        };

        Self {
            workspaces: vec![default_ws],
            active_workspace_id: "ws_default".to_string(),
            next_id: 2,
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
        let ws = Workspace {
            id: ws_id.clone(),
            name: name.to_string(),
            tabs: vec![tab],
            active_tab_id: tab_id,
        };

        self.workspaces.push(ws);
        self.active_workspace_id = ws_id.clone();
        Ok(ws_id)
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
}

