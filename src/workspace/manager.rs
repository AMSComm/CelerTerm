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
    next_id: usize,
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceManager {
    pub fn new() -> Self {
        let default_cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let default_tab = Tab::new("tab_1", "Shell", default_cwd);
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
        let tab = Tab::new(&tab_id, format!("Tab {}", self.next_id - 1), cwd);

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

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let tab = Tab::new(&tab_id, "Shell", cwd);
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
}
