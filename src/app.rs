use std::collections::HashMap;
use std::io::{Read, Write};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{ModifiersState, PhysicalKey};
use winit::window::{CursorIcon, Window, WindowAttributes, WindowId};
use crate::config::load_config;
use crate::window::macos::configure_macos_window;
#[cfg(target_os = "macos")]
use crate::window::macos::apply_traffic_lights_visibility;
use crate::window::tabs::calculate_header_layout;
use crate::workspace::WorkspaceManager;
use crate::pty::PtySession;
use crate::term::{TermScreen, translate_key_event, Modifiers, KeyAction};
use crate::renderer::{TextRenderer, parse_hex_color, resolve_color};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::SelectionType;
use parking_lot::Mutex;
use log::info;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeCommitAction {
    Append(Vec<u8>),
    Backspace,
    Enter,
    ShiftEnter,
    Confirm,
    None,
}


#[derive(Debug)]

pub enum UserEvent {
    PtyOutput {
        tab_id: String,
        bytes: Vec<u8>,
    },
    PtyExited {
        tab_id: String,
    },
    UpdateCheckResult(Result<Option<crate::update::ReleaseInfo>, String>),
    UpdateDownloadResult(Result<PathBuf, String>),
}

pub struct TabSession {
    pub screen: TermScreen,
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    pub child_pid: Option<u32>,
}

impl TabSession {
    pub fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        let mut w = self.writer.lock();
        w.write_all(buf)
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        let mut w = self.writer.lock();
        w.flush()
    }

    pub fn foreground_process_id(&self) -> Option<u32> {
        #[cfg(unix)]
        {
            let master = self.master.lock();
            master.process_group_leader().map(|pid| pid as u32)
        }
        #[cfg(not(unix))]
        {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceModalMode {
    List,
    Renaming { input: String },
    Creating { input: String },
    Coloring {
        target_ws_id: String,
        is_background: bool,
        selected_swatch_idx: usize,
        hex_input: String,
    },
}

#[derive(Debug, Clone)]
pub struct WorkspaceModalState {
    pub is_open: bool,
    pub selected_index: usize,
    pub mode: WorkspaceModalMode,
}

impl Default for WorkspaceModalState {
    fn default() -> Self {
        Self {
            is_open: false,
            selected_index: 0,
            mode: WorkspaceModalMode::List,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateModalState {
    pub is_open: bool,
    pub state: crate::update::UpdateState,
    pub scroll_offset: usize,
}

impl Default for UpdateModalState {
    fn default() -> Self {
        Self {
            is_open: false,
            state: crate::update::UpdateState::Idle,
            scroll_offset: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TabColorModalState {
    pub is_open: bool,
    pub target_tab_id: String,
    pub target_tab_title: String,
    pub selected_swatch_idx: usize,
    pub hex_input: String,
    pub anchor_x: f32,
    pub anchor_y: f32,
}

impl Default for TabColorModalState {
    fn default() -> Self {
        Self {
            is_open: false,
            target_tab_id: String::new(),
            target_tab_title: String::new(),
            selected_swatch_idx: 0,
            hex_input: String::new(),
            anchor_x: 0.0,
            anchor_y: 0.0,
        }
    }
}

pub struct CelerApp {
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    config: crate::config::Config,
    workspace_mgr: WorkspaceManager,
    workspace_modal: WorkspaceModalState,
    update_modal: UpdateModalState,
    tab_color_modal: TabColorModalState,
    app_menu_open: bool,
    tab_sessions: HashMap<String, TabSession>,
    renderer: TextRenderer,
    modifiers: ModifiersState,
    proxy: Option<EventLoopProxy<UserEvent>>,
    mouse_pos: (f64, f64),
    mouse_pressed_button: Option<MouseButton>,
    last_reported_mouse_grid: Option<(usize, usize)>,
    is_selecting: bool,
    last_click: Option<(std::time::Instant, Point)>,
    click_count: usize,
    ime_preedit: Option<(String, Option<(usize, usize)>)>,
    last_preedit: Option<(String, std::time::Instant)>,
    last_ime_confirm: Option<std::time::Instant>,
    scale_factor: f32,
    cols: usize,
    rows: usize,
    is_secondary_window: bool,
    deleted_workspace_ids: Vec<String>,
    last_snapshot_mtime: Option<std::time::SystemTime>,
}

impl Default for CelerApp {
    fn default() -> Self {
        Self::new()
    }
}

impl CelerApp {
    pub fn new() -> Self {
        crate::pty::bootstrap_env_path();
        crate::window::disable_app_nap();
        let config = load_config();
        let mut workspace_mgr = WorkspaceManager::new();

        if config.workspace.restore_on_startup
            && let Some(path) = crate::workspace::get_default_snapshot_path()
            && path.exists()
        {
            match crate::workspace::load_snapshot_from_file(&path) {
                Ok(loaded) => {
                    if !loaded.workspaces.is_empty() && loaded.workspaces.iter().any(|w| !w.tabs.is_empty()) {
                        info!("Restored workspace snapshot from {}", path.display());
                        workspace_mgr = loaded;
                    }
                }
                Err(e) => {
                    log::warn!("Failed to load workspace snapshot: {e}");
                }
            }
        }

        // Support --workspace <name> CLI flag to open directly into a named workspace
        let mut target_ws = None;
        let mut is_secondary_window = false;
        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            if (args[i] == "--workspace" || args[i] == "-w") && i + 1 < args.len() {
                target_ws = Some(args[i + 1].clone());
                is_secondary_window = true;
                i += 2;
            } else {
                i += 1;
            }
        }

        let my_pid = std::process::id();
        let other_instances = crate::workspace::get_all_active_instances();

        if let Some(ref ws_name) = target_ws {
            if let Some(other) = crate::workspace::find_other_instance_in(&other_instances, my_pid, "", ws_name) {
                log::info!("Workspace '{}' is already open in PID {}. Focusing that window.", ws_name, other.pid);
                crate::workspace::focus_instance(other.pid);
                std::process::exit(0);
            }
            if let Some(existing) = workspace_mgr.workspaces.iter().find(|w| &w.name == ws_name) {
                workspace_mgr.active_workspace_id = existing.id.clone();
            } else if let Ok(new_id) = workspace_mgr.new_workspace(ws_name) {
                workspace_mgr.active_workspace_id = new_id;
            }
        } else {
            let active_name = workspace_mgr.get_active_workspace().map(|w| w.name.clone()).unwrap_or_default();
            if crate::workspace::find_other_instance_in(&other_instances, my_pid, &workspace_mgr.active_workspace_id, &active_name).is_some() {
                let available = workspace_mgr.workspaces.iter().find(|w| {
                    crate::workspace::find_other_instance_in(&other_instances, my_pid, &w.id, &w.name).is_none()
                }).map(|w| w.id.clone());

                if let Some(available_id) = available {
                    workspace_mgr.active_workspace_id = available_id;
                } else {
                    let mut n = workspace_mgr.workspaces.len() + 1;
                    let mut new_name = format!("Workspace {}", n);
                    while workspace_mgr.workspaces.iter().any(|w| w.name == new_name) {
                        n += 1;
                        new_name = format!("Workspace {}", n);
                    }
                    if let Ok(new_id) = workspace_mgr.new_workspace(&new_name) {
                        workspace_mgr.active_workspace_id = new_id;
                    }
                }
            }
        }

        let is_secondary_window = is_secondary_window || !other_instances.is_empty();


        let cols = 100;
        let rows = 30;
        let renderer = TextRenderer::with_options(
            &config.font.family,
            &config.font.fallback_families,
            config.font.size,
            config.font.line_height,
            config.font.ligatures,
        );

        let app = Self {
            window: None,
            surface: None,
            config,
            workspace_mgr,
            workspace_modal: WorkspaceModalState::default(),
            update_modal: UpdateModalState::default(),
            tab_color_modal: TabColorModalState::default(),
            app_menu_open: false,
            tab_sessions: HashMap::new(),
            renderer,
            modifiers: ModifiersState::default(),
            proxy: None,
            mouse_pos: (0.0, 0.0),
            mouse_pressed_button: None,
            last_reported_mouse_grid: None,
            is_selecting: false,
            last_click: None,
            click_count: 0,
            ime_preedit: None,
            last_preedit: None,
            last_ime_confirm: None,
            scale_factor: 1.0,
            cols,
            rows,
            is_secondary_window,
            deleted_workspace_ids: Vec::new(),
            last_snapshot_mtime: None,
        };

        if let Some(active_ws) = app.workspace_mgr.get_active_workspace() {
            crate::workspace::register_active_instance(std::process::id(), &active_ws.name, &active_ws.id);
        }

        app.update_window_and_process_title();

        app
    }

    /// Synchronizes process name, dock badge, window title, and environment variables
    /// with the currently active workspace.
    pub fn update_window_and_process_title(&self) {
        if let Some(active_ws) = self.workspace_mgr.get_active_workspace() {
            let ws_name = &active_ws.name;
            crate::workspace::register_active_instance(std::process::id(), ws_name, &active_ws.id);
            unsafe {
                std::env::set_var("CELERTERM_WORKSPACE", ws_name);
                std::env::set_var("CELER_WORKSPACE", ws_name);
                std::env::set_var("CELERTERM_WORKSPACE_ID", &active_ws.id);
            }

            #[cfg(target_os = "macos")]
            {
                crate::window::macos::set_macos_process_name(&format!("CelerTerm ({})", ws_name));
                crate::window::macos::set_macos_dock_badge(Some(ws_name));
                crate::window::macos::set_macos_menu_title(&format!("CelerTerm ({})", ws_name));
            }

            if let Some(ref window) = self.window {
                window.set_title(&format!("CelerTerm - {}", ws_name));
            }
        }
    }


    pub fn update_renderer(&mut self) {
        let effective_size = (self.config.font.size * self.scale_factor).max(8.0);
        self.renderer = TextRenderer::with_options(
            &self.config.font.family,
            &self.config.font.fallback_families,
            effective_size,
            self.config.font.line_height,
            self.config.font.ligatures,
        );
    }

    pub fn reload_config(&mut self) {
        self.config = load_config();
        self.update_renderer();
        let window = self.window.clone();
        if let Some(ref win) = window {
            let size = win.inner_size();
            self.recalculate_grid(size.width as f32, size.height as f32);
            #[cfg(target_os = "macos")]
            apply_traffic_lights_visibility(win, self.config.window.hide_traffic_lights);
            win.request_redraw();
        }
        info!("Configuration reloaded from disk");
    }

    pub fn recalculate_grid(&mut self, width: f32, height: f32) {
        // Prevent recalculating grid when window is minimized or display closed (zero or tiny dimensions)
        if width < 120.0 || height < 80.0 {
            return;
        }

        if self.renderer.cell_width <= 0.0 || self.renderer.cell_height <= 0.0 {
            return;
        }

        let scale = self.scale_factor.max(0.5);
        let header_h = if self.config.window.tabs_in_titlebar { (26.0 * scale).round() } else { 0.0 };
        let pad_x = (self.config.window.padding_x * scale).round();
        let pad_y = (self.config.window.padding_y * scale).round();
        let term_h = (height - header_h - pad_y * 2.0).max(10.0);
        let term_w = (width - pad_x * 2.0).max(10.0);

        let cols = (term_w / self.renderer.cell_width).floor() as usize;
        let rows = (term_h / self.renderer.cell_height).floor() as usize;

        // Ensure minimum terminal size so TUI apps (agy, claude, fzf, vim) never receive panic-inducing 1x1 or 0x0 SIGWINCH
        const MIN_COLS: usize = 20;
        const MIN_ROWS: usize = 4;

        if cols >= MIN_COLS && rows >= MIN_ROWS {
            if self.cols != cols || self.rows != rows {
                self.cols = cols;
                self.rows = rows;
                for session in self.tab_sessions.values_mut() {
                    session.screen.resize(cols, rows);
                    let master = session.master.lock();
                    let _ = master.resize(portable_pty::PtySize {
                        rows: rows as u16,
                        cols: cols as u16,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                }
            }
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn grid_size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    pub fn spawn_tab_session(&mut self, tab_id: &str, cwd: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
        let ws_info = self.workspace_mgr.workspaces.iter()
            .find(|ws| ws.tabs.iter().any(|t| t.id == tab_id))
            .or_else(|| self.workspace_mgr.get_active_workspace());
        let ws_name = ws_info.map(|w| w.name.as_str());
        let ws_id = ws_info.map(|w| w.id.as_str());

        let mut pty = PtySession::spawn_with_workspace(self.cols as u16, self.rows as u16, cwd, ws_name, ws_id)?;
        let mut screen = TermScreen::new(self.cols, self.rows);
        let writer = Arc::new(Mutex::new(pty.writer));
        screen.set_pty_writer(writer.clone());

        // Pre-populate screen with scrollback cache if restoring tab
        let scrollback = self.workspace_mgr.workspaces.iter()
            .find_map(|ws| ws.tabs.iter().find(|t| t.id == tab_id))
            .map(|t| t.scrollback_cache.clone())
            .unwrap_or_default();

        for line in &scrollback {
            screen.process_bytes(line.as_bytes());
            screen.process_bytes(b"\r\n");
        }

        let child_pid = pty.child_pid;
        if let Some(proxy) = &self.proxy {
            let proxy_clone = proxy.clone();
            let thread_tab_id = tab_id.to_string();
            let mut reader = std::mem::replace(
                &mut pty.reader,
                Box::new(std::io::empty()) as Box<dyn Read + Send>,
            );

            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if proxy_clone.send_event(UserEvent::PtyOutput {
                                tab_id: thread_tab_id.clone(),
                                bytes: buf[..n].to_vec(),
                            }).is_err() {
                                break;
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                            continue;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            continue;
                        }
                        Err(_) => break,
                    }
                }
                let _ = proxy_clone.send_event(UserEvent::PtyExited {
                    tab_id: thread_tab_id,
                });
            });
        }

        let session = TabSession {
            screen,
            writer,
            master: pty.master,
            child_pid,
        };

        self.tab_sessions.insert(tab_id.to_string(), session);
        Ok(())
    }

    fn active_tab_id(&self) -> Option<String> {
        self.workspace_mgr.get_active_workspace().map(|ws| ws.active_tab_id.clone())
    }

    pub fn ensure_tab_session(&mut self, tab_id: &str) {
        if !self.tab_sessions.contains_key(tab_id) {
            let cwd = self.workspace_mgr.workspaces.iter()
                .find_map(|ws| ws.tabs.iter().find(|t| t.id == tab_id))
                .map(|t| t.cwd.clone());
            let _ = self.spawn_tab_session(tab_id, cwd.as_deref());
        }
    }

    pub fn activate_current_workspace_sessions(&mut self) {
        if let Some(ws) = self.workspace_mgr.get_active_workspace() {
            let tab_ids: Vec<String> = ws.tabs.iter().map(|t| t.id.clone()).collect();
            for tab_id in tab_ids {
                self.ensure_tab_session(&tab_id);
            }
        }
    }

    pub fn sync_workspace_from_disk_if_unowned(&mut self, workspace_id: &str) {
        let has_active_sessions = self.workspace_mgr.workspaces.iter()
            .find(|w| w.id == workspace_id)
            .map(|w| w.tabs.iter().any(|t| self.tab_sessions.contains_key(&t.id)))
            .unwrap_or(false);

        if !has_active_sessions {
            if let Some(path) = crate::workspace::get_default_snapshot_path()
                && path.exists()
                && let Ok(disk) = crate::workspace::load_snapshot_from_file(&path)
                && let Some(disk_ws) = disk.workspaces.into_iter().find(|w| w.id == workspace_id)
            {
                if let Some(existing) = self.workspace_mgr.workspaces.iter_mut().find(|w| w.id == workspace_id) {
                    *existing = disk_ws;
                }
                self.workspace_mgr.next_id = self.workspace_mgr.next_id.max(disk.next_id);
            }
        }
    }

    pub fn reload_workspaces_from_disk(&mut self) -> bool {
        let Some(path) = crate::workspace::get_default_snapshot_path() else {
            return false;
        };
        if !path.exists() {
            return false;
        }

        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if mtime.is_some() && mtime == self.last_snapshot_mtime {
            return false;
        }

        if let Ok(disk_manager) = crate::workspace::load_snapshot_from_file(&path) {
            let owned_tabs: Vec<String> = self.tab_sessions.keys().cloned().collect();
            let merged = crate::workspace::merge_workspace_managers(
                &self.workspace_mgr,
                &owned_tabs,
                &self.deleted_workspace_ids,
                Some(&disk_manager),
                self.is_secondary_window,
            );
            let changed = self.workspace_mgr.workspaces.len() != merged.workspaces.len()
                || self.workspace_mgr.workspaces.iter().zip(&merged.workspaces).any(|(a, b)| a.id != b.id || a.name != b.name);
            self.workspace_mgr.workspaces = merged.workspaces;
            self.workspace_mgr.next_id = self.workspace_mgr.next_id.max(merged.next_id);
            self.last_snapshot_mtime = mtime;

            if self.workspace_modal.selected_index >= self.workspace_mgr.workspaces.len() {
                self.workspace_modal.selected_index = self.workspace_mgr.workspaces.len().saturating_sub(1);
            }
            return changed;
        }
        false
    }

    pub fn get_active_tab_cwd(&self) -> PathBuf {
        if let Some(active_id) = self.active_tab_id() {
            if let Some(session) = self.tab_sessions.get(&active_id)
                && let Some(child_pid) = session.child_pid
                && let Some(cwd) = crate::pty::get_process_cwd(child_pid)
            {
                return cwd;
            }
            if let Some(ws) = self.workspace_mgr.get_active_workspace()
                && let Some(tab) = ws.tabs.iter().find(|t| t.id == active_id)
            {
                return tab.cwd.clone();
            }
        }
        directories::BaseDirs::new()
            .map(|b| b.home_dir().to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")))
    }

    pub fn update_tab_titles(&mut self) {
        if let Some(ws) = self.workspace_mgr.get_active_workspace_mut() {
            for tab in &mut ws.tabs {
                if let Some(session) = self.tab_sessions.get(&tab.id) {
                    let fg_pid = session.foreground_process_id();
                    let child_pid = session.child_pid;

                    // Update live cwd of the shell process
                    if let Some(cpid) = child_pid
                        && let Some(cwd) = crate::pty::get_process_cwd(cpid)
                    {
                        tab.cwd = cwd;
                    }

                    let dyn_title = session.screen.dynamic_title();

                    let mut is_ssh = false;
                    if let Some(fpid) = fg_pid
                        && child_pid != Some(fpid) && fpid > 0
                    {
                        let proc_name = crate::pty::get_process_name(fpid);
                        if let Some(ref p) = proc_name {
                            let p_trim = p.trim();
                            is_ssh = p_trim == "ssh" || p_trim.starts_with("ssh ") || p_trim == "mosh-client";
                        }
                        tab.title = crate::pty::format_tab_title(proc_name.as_deref(), Some(&tab.cwd), dyn_title.as_deref());
                    } else {
                        tab.title = crate::pty::format_tab_title(None, Some(&tab.cwd), dyn_title.as_deref());
                    }

                    if !is_ssh && dyn_title.is_some() {
                        session.screen.reset_dynamic_title();
                    }
                }
            }
        }
    }

    pub fn save_workspace_state(&mut self) {
        let max_lines = self.config.workspace.max_scrollback_lines;
        let save_scrollback = self.config.workspace.save_scrollback;

        for ws in &mut self.workspace_mgr.workspaces {
            for tab in &mut ws.tabs {
                if let Some(session) = self.tab_sessions.get(&tab.id) {
                    if let Some(child_pid) = session.child_pid
                        && let Some(cwd) = crate::pty::get_process_cwd(child_pid)
                    {
                        tab.cwd = cwd;
                        let fg_pid = session.foreground_process_id();
                        let proc_name = if let Some(fpid) = fg_pid && child_pid != fpid && fpid > 0 {
                            crate::pty::get_process_name(fpid)
                        } else {
                            None
                        };
                        let dyn_title = session.screen.dynamic_title();
                        tab.title = crate::pty::format_tab_title(proc_name.as_deref(), Some(&tab.cwd), dyn_title.as_deref());
                    }
                    if save_scrollback {
                        tab.scrollback_cache = session.screen.get_scrollback_lines(max_lines);
                    }
                }
            }
        }

        if let Some(path) = crate::workspace::get_default_snapshot_path() {
            let disk_manager = if path.exists() {
                crate::workspace::load_snapshot_from_file(&path).ok()
            } else {
                None
            };

            let owned_tabs: Vec<String> = self.tab_sessions.keys().cloned().collect();
            let merged = crate::workspace::merge_workspace_managers(
                &self.workspace_mgr,
                &owned_tabs,
                &self.deleted_workspace_ids,
                disk_manager.as_ref(),
                self.is_secondary_window,
            );
            self.workspace_mgr.workspaces = merged.workspaces.clone();
            self.workspace_mgr.next_id = merged.next_id;

            if let Err(e) = crate::workspace::save_snapshot_to_file(&merged, &path) {
                log::warn!("Failed to save workspace snapshot: {e}");
            } else {
                info!("Saved workspace snapshot to {}", path.display());
                self.last_snapshot_mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            }
        }

        if let Some(active_ws) = self.workspace_mgr.get_active_workspace() {
            crate::workspace::register_active_instance(std::process::id(), &active_ws.name, &active_ws.id);
        }
        self.update_window_and_process_title();
    }

    /// Attempts to switch to target workspace. If it is already open in another window,
    /// brings that window to front instead of switching locally.
    /// Returns true if switched locally, false if focused another window or failed.
    pub fn switch_or_focus_workspace(&mut self, target_id: &str) -> bool {
        let target = match self.workspace_mgr.workspaces.iter().find(|w| w.id == target_id) {
            Some(w) => w,
            None => return false,
        };
        let target_name = target.name.clone();
        let target_id = target.id.clone();
        let my_pid = std::process::id();

        if let Some(other) = crate::workspace::find_other_instance_for_workspace(my_pid, &target_id, &target_name) {
            log::info!("Workspace '{}' is already open in PID {}. Focusing that instance.", target_name, other.pid);
            crate::workspace::focus_instance(other.pid);
            return false;
        }

        self.sync_workspace_from_disk_if_unowned(&target_id);
        if self.workspace_mgr.switch_workspace(&target_id).is_ok() {
            self.activate_current_workspace_sessions();
            self.save_workspace_state();
            true
        } else {
            false
        }
    }

    /// Spawns a new window for the workspace if it's not already open elsewhere.
    /// If already open in another window, focuses it instead.
    /// If it's already the current active workspace in this window, ignores duplicate creation.
    pub fn open_workspace_in_new_window(&mut self, target_idx: usize) {
        if let Some(ws) = self.workspace_mgr.workspaces.get(target_idx) {
            let target_id = ws.id.clone();
            let target_name = ws.name.clone();
            let my_pid = std::process::id();

            if let Some(other) = crate::workspace::find_other_instance_for_workspace(my_pid, &target_id, &target_name) {
                log::info!("Workspace '{}' is already open in PID {}. Focusing that instance.", target_name, other.pid);
                crate::workspace::focus_instance(other.pid);
                return;
            }

            if target_id == self.workspace_mgr.active_workspace_id {
                log::info!("Workspace '{}' is already active in current window.", target_name);
                return;
            }

            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe)
                    .arg("--workspace")
                    .arg(&target_name)
                    .spawn();
            }
        }
    }

    fn handle_modal_key(&mut self, key: &winit::keyboard::Key, _code: Option<winit::keyboard::KeyCode>) {
        match &mut self.workspace_modal.mode {
            WorkspaceModalMode::Renaming { input } => {
                match key {
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                        self.workspace_modal.mode = WorkspaceModalMode::List;
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) => {
                        let trimmed = input.trim().to_string();
                        if !trimmed.is_empty() {
                            let idx = self.workspace_modal.selected_index;
                            if let Some(ws) = self.workspace_mgr.workspaces.get(idx) {
                                let id = ws.id.clone();
                                let _ = self.workspace_mgr.rename_workspace(&id, &trimmed);
                                self.save_workspace_state();
                            }
                        }
                        self.workspace_modal.mode = WorkspaceModalMode::List;
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => {
                        input.pop();
                    }
                    winit::keyboard::Key::Character(s) => {
                        input.push_str(s);
                    }
                    _ => {}
                }
            }
            WorkspaceModalMode::Creating { input } => {
                match key {
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                        self.workspace_modal.mode = WorkspaceModalMode::List;
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) => {
                        let name = if input.trim().is_empty() {
                            format!("Workspace {}", self.workspace_mgr.workspaces.len() + 1)
                        } else {
                            input.trim().to_string()
                        };
                        if let Ok(_new_ws_id) = self.workspace_mgr.new_workspace(&name) {
                            if let Some(active_id) = self.active_tab_id() {
                                let cwd = self.get_active_tab_cwd();
                                let _ = self.spawn_tab_session(&active_id, Some(&cwd));
                            }
                            self.save_workspace_state();
                            self.workspace_modal.selected_index = self.workspace_mgr.workspaces.len().saturating_sub(1);
                        }
                        self.workspace_modal.mode = WorkspaceModalMode::List;
                        self.workspace_modal.is_open = false;
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => {
                        input.pop();
                    }
                    winit::keyboard::Key::Character(s) => {
                        input.push_str(s);
                    }
                    _ => {}
                }
            }
            WorkspaceModalMode::Coloring {
                target_ws_id,
                is_background,
                selected_swatch_idx,
                hex_input,
            } => {
                match key {
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                        self.workspace_modal.mode = WorkspaceModalMode::List;
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Tab) => {
                        *is_background = !*is_background;
                        *selected_swatch_idx = 0;
                        if let Some(ws) = self.workspace_mgr.workspaces.iter().find(|w| &w.id == target_ws_id) {
                            if *is_background {
                                *hex_input = ws.background.as_deref().unwrap_or("1a1b26").trim_start_matches('#').to_string();
                            } else {
                                *hex_input = ws.color.as_deref().unwrap_or("7aa2f7").trim_start_matches('#').to_string();
                            }
                        }
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) => {
                        let ws_id = target_ws_id.clone();
                        let chosen_hex = if hex_input.len() == 6 && hex_input.chars().all(|c| c.is_ascii_hexdigit()) {
                            format!("#{}", hex_input.to_lowercase())
                        } else if let Some(norm) = crate::workspace::normalize_hex(hex_input) {
                            norm
                        } else if *is_background {
                            crate::workspace::WORKSPACE_BACKGROUND_PALETTE[*selected_swatch_idx % crate::workspace::WORKSPACE_BACKGROUND_PALETTE.len()].hex.to_string()
                        } else {
                            crate::workspace::WORKSPACE_ACCENT_PALETTE[*selected_swatch_idx % crate::workspace::WORKSPACE_ACCENT_PALETTE.len()].hex.to_string()
                        };

                        if *is_background {
                            let _ = self.workspace_mgr.set_workspace_background(&ws_id, Some(chosen_hex));
                        } else {
                            let _ = self.workspace_mgr.set_workspace_color(&ws_id, Some(chosen_hex));
                        }
                        self.save_workspace_state();
                        self.workspace_modal.mode = WorkspaceModalMode::List;
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => {
                        hex_input.pop();
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowLeft) => {
                        *selected_swatch_idx = selected_swatch_idx.saturating_sub(1);
                        if *is_background {
                            *hex_input = crate::workspace::WORKSPACE_BACKGROUND_PALETTE[*selected_swatch_idx].hex.trim_start_matches('#').to_string();
                        } else {
                            *hex_input = crate::workspace::WORKSPACE_ACCENT_PALETTE[*selected_swatch_idx].hex.trim_start_matches('#').to_string();
                        }
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowRight) => {
                        let max_idx = if *is_background { crate::workspace::WORKSPACE_BACKGROUND_PALETTE.len() } else { crate::workspace::WORKSPACE_ACCENT_PALETTE.len() };
                        if *selected_swatch_idx + 1 < max_idx {
                            *selected_swatch_idx += 1;
                        }
                        if *is_background {
                            *hex_input = crate::workspace::WORKSPACE_BACKGROUND_PALETTE[*selected_swatch_idx].hex.trim_start_matches('#').to_string();
                        } else {
                            *hex_input = crate::workspace::WORKSPACE_ACCENT_PALETTE[*selected_swatch_idx].hex.trim_start_matches('#').to_string();
                        }
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowUp) => {
                        if !*is_background && *selected_swatch_idx >= 10 {
                            *selected_swatch_idx -= 10;
                            *hex_input = crate::workspace::WORKSPACE_ACCENT_PALETTE[*selected_swatch_idx].hex.trim_start_matches('#').to_string();
                        }
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowDown) => {
                        if !*is_background && *selected_swatch_idx + 10 < crate::workspace::WORKSPACE_ACCENT_PALETTE.len() {
                            *selected_swatch_idx += 10;
                            *hex_input = crate::workspace::WORKSPACE_ACCENT_PALETTE[*selected_swatch_idx].hex.trim_start_matches('#').to_string();
                        }
                    }
                    winit::keyboard::Key::Character(s) => {
                        match s.as_str() {
                            "r" | "R" if hex_input.is_empty() => {
                                let ws_id = target_ws_id.clone();
                                if *is_background {
                                    let _ = self.workspace_mgr.set_workspace_background(&ws_id, None);
                                } else {
                                    let _ = self.workspace_mgr.set_workspace_color(&ws_id, None);
                                }
                                self.save_workspace_state();
                                self.workspace_modal.mode = WorkspaceModalMode::List;
                            }
                            ch => {
                                for c in ch.chars() {
                                    if (c.is_ascii_hexdigit() || c == '#') && hex_input.len() < 6 {
                                        if c != '#' {
                                            hex_input.push(c);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            WorkspaceModalMode::List => {
                match key {
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                        self.workspace_modal.is_open = false;
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowUp) => {
                        self.workspace_modal.selected_index = self.workspace_modal.selected_index.saturating_sub(1);
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowDown) => {
                        if self.workspace_modal.selected_index + 1 < self.workspace_mgr.workspaces.len() {
                            self.workspace_modal.selected_index += 1;
                        }
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) => {
                        let idx = self.workspace_modal.selected_index;
                        if let Some(ws) = self.workspace_mgr.workspaces.get(idx) {
                            let target_id = ws.id.clone();
                            self.switch_or_focus_workspace(&target_id);
                        }
                        self.workspace_modal.is_open = false;
                    }
                    winit::keyboard::Key::Character(s) => {
                        match s.as_str() {
                            "n" | "N" => {
                                self.workspace_modal.mode = WorkspaceModalMode::Creating { input: String::new() };
                            }
                            "r" | "R" => {
                                let idx = self.workspace_modal.selected_index;
                                if let Some(ws) = self.workspace_mgr.workspaces.get(idx) {
                                    self.workspace_modal.mode = WorkspaceModalMode::Renaming { input: ws.name.clone() };
                                }
                            }
                            "c" | "C" => {
                                let idx = self.workspace_modal.selected_index;
                                if let Some(ws) = self.workspace_mgr.workspaces.get(idx) {
                                    let initial_hex = ws.color.clone().unwrap_or_else(|| ws.effective_color_hex(idx));
                                    self.workspace_modal.mode = WorkspaceModalMode::Coloring {
                                        target_ws_id: ws.id.clone(),
                                        is_background: false,
                                        selected_swatch_idx: 0,
                                        hex_input: initial_hex.trim_start_matches('#').to_string(),
                                    };
                                }
                            }
                            "d" | "D" => {
                                if self.workspace_mgr.workspaces.len() > 1 {
                                    let idx = self.workspace_modal.selected_index;
                                    if let Some(ws) = self.workspace_mgr.workspaces.get(idx) {
                                        let target_id = ws.id.clone();
                                        let my_pid = std::process::id();
                                        if crate::workspace::find_other_instance_for_workspace(my_pid, &target_id, &ws.name).is_some() {
                                            log::warn!("Cannot delete workspace '{}' because it is active in another window.", ws.name);
                                        } else {
                                            for tab in &ws.tabs {
                                                self.tab_sessions.remove(&tab.id);
                                            }
                                            self.deleted_workspace_ids.push(target_id.clone());
                                            let _ = self.workspace_mgr.delete_workspace(&target_id);
                                            self.workspace_modal.selected_index = self.workspace_modal.selected_index.min(self.workspace_mgr.workspaces.len().saturating_sub(1));
                                            self.activate_current_workspace_sessions();
                                            self.save_workspace_state();
                                        }
                                    }
                                }
                            }
                            "w" | "W" => {
                                let idx = self.workspace_modal.selected_index;
                                self.open_workspace_in_new_window(idx);
                                self.workspace_modal.is_open = false;
                            }
                            "k" => {
                                self.workspace_modal.selected_index = self.workspace_modal.selected_index.saturating_sub(1);
                            }
                            "j" => {
                                if self.workspace_modal.selected_index + 1 < self.workspace_mgr.workspaces.len() {
                                    self.workspace_modal.selected_index += 1;
                                }
                            }
                            _ => {
                                if let Ok(num) = s.parse::<usize>()
                                    && num >= 1 && num <= self.workspace_mgr.workspaces.len()
                                {
                                    let target_id = self.workspace_mgr.workspaces[num - 1].id.clone();
                                    self.switch_or_focus_workspace(&target_id);
                                    self.workspace_modal.is_open = false;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn open_tab_color_modal(&mut self, tab_id: &str, anchor_x: f32, anchor_y: f32) {
        let ws = self.workspace_mgr.get_active_workspace();
        let target_tab = ws.and_then(|w| w.tabs.iter().find(|t| t.id == tab_id));
        let tab_title = target_tab.map(|t| t.title.clone()).unwrap_or_else(|| "Tab".to_string());
        let current_color = target_tab.and_then(|t| t.color.clone());
        let hex_val = current_color.as_deref().unwrap_or("7aa2f7").trim_start_matches('#').to_string();
        let selected_swatch = crate::workspace::WORKSPACE_ACCENT_PALETTE
            .iter()
            .position(|p| p.hex.eq_ignore_ascii_case(&format!("#{}", hex_val)))
            .unwrap_or(0);

        self.tab_color_modal = TabColorModalState {
            is_open: true,
            target_tab_id: tab_id.to_string(),
            target_tab_title: tab_title,
            selected_swatch_idx: selected_swatch,
            hex_input: hex_val,
            anchor_x,
            anchor_y,
        };
    }

    fn handle_tab_color_modal_key(&mut self, key: &winit::keyboard::Key) {
        match key {
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                self.tab_color_modal.is_open = false;
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) => {
                let chosen_hex = if self.tab_color_modal.hex_input.len() == 6 && self.tab_color_modal.hex_input.chars().all(|c| c.is_ascii_hexdigit()) {
                    format!("#{}", self.tab_color_modal.hex_input.to_lowercase())
                } else if let Some(norm) = crate::workspace::normalize_hex(&self.tab_color_modal.hex_input) {
                    norm
                } else {
                    crate::workspace::WORKSPACE_ACCENT_PALETTE[self.tab_color_modal.selected_swatch_idx % crate::workspace::WORKSPACE_ACCENT_PALETTE.len()].hex.to_string()
                };

                let _ = self.workspace_mgr.set_tab_color(&self.tab_color_modal.target_tab_id, Some(chosen_hex));
                self.save_workspace_state();
                self.tab_color_modal.is_open = false;
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => {
                self.tab_color_modal.hex_input.pop();
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowLeft) => {
                self.tab_color_modal.selected_swatch_idx = self.tab_color_modal.selected_swatch_idx.saturating_sub(1);
                self.tab_color_modal.hex_input = crate::workspace::WORKSPACE_ACCENT_PALETTE[self.tab_color_modal.selected_swatch_idx].hex.trim_start_matches('#').to_string();
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowRight) => {
                if self.tab_color_modal.selected_swatch_idx + 1 < crate::workspace::WORKSPACE_ACCENT_PALETTE.len() {
                    self.tab_color_modal.selected_swatch_idx += 1;
                    self.tab_color_modal.hex_input = crate::workspace::WORKSPACE_ACCENT_PALETTE[self.tab_color_modal.selected_swatch_idx].hex.trim_start_matches('#').to_string();
                }
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowUp) => {
                if self.tab_color_modal.selected_swatch_idx >= 10 {
                    self.tab_color_modal.selected_swatch_idx -= 10;
                    self.tab_color_modal.hex_input = crate::workspace::WORKSPACE_ACCENT_PALETTE[self.tab_color_modal.selected_swatch_idx].hex.trim_start_matches('#').to_string();
                }
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowDown) => {
                if self.tab_color_modal.selected_swatch_idx + 10 < crate::workspace::WORKSPACE_ACCENT_PALETTE.len() {
                    self.tab_color_modal.selected_swatch_idx += 10;
                    self.tab_color_modal.hex_input = crate::workspace::WORKSPACE_ACCENT_PALETTE[self.tab_color_modal.selected_swatch_idx].hex.trim_start_matches('#').to_string();
                }
            }
            winit::keyboard::Key::Character(s) => {
                match s.as_str() {
                    "r" | "R" => {
                        let _ = self.workspace_mgr.set_tab_color(&self.tab_color_modal.target_tab_id, None);
                        self.save_workspace_state();
                        self.tab_color_modal.is_open = false;
                    }
                    ch => {
                        for c in ch.chars() {
                            if (c.is_ascii_hexdigit() || c == '#') && self.tab_color_modal.hex_input.len() < 6 {
                                if c != '#' {
                                    self.tab_color_modal.hex_input.push(c.to_ascii_lowercase());
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn start_check_for_updates(&mut self) {
        self.update_modal.is_open = true;
        self.update_modal.state = crate::update::UpdateState::Checking;
        self.update_modal.scroll_offset = 0;
        if let Some(ref win) = self.window {
            win.request_redraw();
        }
        if let Some(ref proxy) = self.proxy {
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                let res = crate::update::fetch_latest_release(crate::update::GITHUB_REPO);
                let _ = proxy.send_event(UserEvent::UpdateCheckResult(res));
            });
        }
    }

    fn trigger_primary_update_action(&mut self, event_loop: &ActiveEventLoop) {
        match self.update_modal.state.clone() {
            crate::update::UpdateState::Available {
                download_url,
                asset_name,
                latest_version,
                html_url,
                ..
            } => {
                if let (Some(url), Some(name)) = (download_url, asset_name) {
                    self.start_download_and_install(url, name, latest_version);
                } else {
                    crate::update::open_browser(&html_url);
                }
            }
            crate::update::UpdateState::ReadyToRestart {
                staged_path,
                target_path,
                ..
            } => {
                self.save_workspace_state();

                // 1. Gather all active instances and running CelerTerm processes
                let my_pid = std::process::id();
                let instances = crate::workspace::get_all_active_instances();
                let mut other_pids = crate::update::find_other_celerterm_pids();
                for inst in &instances {
                    if inst.pid != my_pid && !other_pids.contains(&inst.pid) {
                        other_pids.push(inst.pid);
                    }
                }

                // 2. Collect workspaces to reopen
                let mut workspaces_to_reopen = Vec::new();
                if let Some(active_ws) = self.workspace_mgr.get_active_workspace() {
                    workspaces_to_reopen.push(active_ws.name.clone());
                }
                for inst in &instances {
                    if inst.pid != my_pid && !workspaces_to_reopen.contains(&inst.workspace_name) {
                        workspaces_to_reopen.push(inst.workspace_name.clone());
                    }
                }
                // Fallback: If other CelerTerm processes were running but active_instances didn't have their workspace names,
                // check if the workspace snapshot has additional workspaces not yet included
                if workspaces_to_reopen.len() <= 1 && !other_pids.is_empty() {
                    for ws in &self.workspace_mgr.workspaces {
                        if !workspaces_to_reopen.contains(&ws.name) && workspaces_to_reopen.len() <= other_pids.len() {
                            workspaces_to_reopen.push(ws.name.clone());
                        }
                    }
                }

                crate::workspace::unregister_active_instance(my_pid);

                if let Err(e) = crate::update::apply_update_and_restart(
                    &staged_path,
                    &target_path,
                    &other_pids,
                    &workspaces_to_reopen,
                ) {
                    log::error!("Failed to launch update script: {}", e);
                    self.update_modal.state = crate::update::UpdateState::Error(e);
                    if let Some(ref win) = self.window {
                        win.request_redraw();
                    }
                } else {
                    event_loop.exit();
                }
            }
            crate::update::UpdateState::Downloading { .. } => {
                // In progress; do nothing
            }
            crate::update::UpdateState::UpToDate { .. }
            | crate::update::UpdateState::Error(_) => {
                self.start_check_for_updates();
            }
            crate::update::UpdateState::Idle | crate::update::UpdateState::Checking => {}
        }
    }

    fn start_download_and_install(&mut self, url: String, asset_name: String, version: String) {
        self.update_modal.state = crate::update::UpdateState::Downloading {
            latest_version: version,
            status_text: "Downloading update package from GitHub...".to_string(),
        };
        if let Some(ref win) = self.window {
            win.request_redraw();
        }

        if let Some(ref proxy) = self.proxy {
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                let res = crate::update::download_and_stage_update(&url, &asset_name);
                let _ = proxy.send_event(UserEvent::UpdateDownloadResult(res));
            });
        }
    }

    fn handle_update_modal_key(&mut self, event_loop: &ActiveEventLoop, key: &winit::keyboard::Key, _code: Option<winit::keyboard::KeyCode>) {
        match key {
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                self.update_modal.is_open = false;
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) => {
                self.trigger_primary_update_action(event_loop);
            }
            winit::keyboard::Key::Character(s) if s.eq_ignore_ascii_case("g") => {
                crate::update::open_browser(&format!("https://github.com/{}", crate::update::GITHUB_REPO));
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowUp)
            | winit::keyboard::Key::Named(winit::keyboard::NamedKey::PageUp) => {
                self.update_modal.scroll_offset = self.update_modal.scroll_offset.saturating_sub(1);
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowDown)
            | winit::keyboard::Key::Named(winit::keyboard::NamedKey::PageDown) => {
                self.update_modal.scroll_offset += 1;
            }
            _ => {}
        }
    }
}

impl ApplicationHandler<UserEvent> for CelerApp {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::UpdateCheckResult(res) => {
                match res {
                    Ok(Some(info)) => {
                        let ordering = crate::update::compare_versions(crate::update::CURRENT_VERSION, &info.tag_name);
                        if ordering == std::cmp::Ordering::Less {
                            self.update_modal.state = crate::update::UpdateState::Available {
                                current_version: crate::update::CURRENT_VERSION.to_string(),
                                latest_version: info.tag_name,
                                html_url: info.html_url,
                                download_url: info.download_url,
                                asset_name: info.asset_name,
                                notes: info.body,
                                published_at: info.published_at,
                            };
                        } else {
                            self.update_modal.state = crate::update::UpdateState::UpToDate {
                                current_version: crate::update::CURRENT_VERSION.to_string(),
                            };
                        }
                    }
                    Ok(None) => {
                        self.update_modal.state = crate::update::UpdateState::UpToDate {
                            current_version: crate::update::CURRENT_VERSION.to_string(),
                        };
                    }
                    Err(err) => {
                        self.update_modal.state = crate::update::UpdateState::Error(err);
                    }
                }
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
            UserEvent::UpdateDownloadResult(res) => {
                match res {
                    Ok(staged_path) => {
                        let latest_version = match &self.update_modal.state {
                            crate::update::UpdateState::Downloading { latest_version, .. } => latest_version.clone(),
                            _ => "latest".to_string(),
                        };
                        let target_path = crate::update::get_target_app_path();
                        self.update_modal.state = crate::update::UpdateState::ReadyToRestart {
                            latest_version,
                            staged_path,
                            target_path,
                        };
                    }
                    Err(err) => {
                        self.update_modal.state = crate::update::UpdateState::Error(err);
                    }
                }
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
            UserEvent::PtyOutput { tab_id, bytes } => {
                if let Some(session) = self.tab_sessions.get_mut(&tab_id) {
                    session.screen.process_bytes(&bytes);
                    if let Some(active_id) = self.active_tab_id()
                        && active_id == tab_id
                        && let Some(ref window) = self.window
                    {
                        window.request_redraw();
                    }
                }
            }
            UserEvent::PtyExited { tab_id } => {
                if self.tab_sessions.len() <= 1 {
                    self.save_workspace_state();
                    event_loop.exit();
                    return;
                }
                if let Some(session) = self.tab_sessions.get(&tab_id) {
                    for ws in &mut self.workspace_mgr.workspaces {
                        if let Some(tab) = ws.tabs.iter_mut().find(|t| t.id == tab_id)
                            && self.config.workspace.save_scrollback
                        {
                            tab.scrollback_cache = session.screen.get_scrollback_lines(self.config.workspace.max_scrollback_lines);
                        }
                    }
                }
                self.tab_sessions.remove(&tab_id);
                let _ = self.workspace_mgr.close_tab(&tab_id);
                self.save_workspace_state();
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let active_ws_name = self.workspace_mgr.get_active_workspace()
            .map(|w| w.name.clone())
            .unwrap_or_else(|| "CelerTerm".to_string());
        let win_title = format!("CelerTerm - {}", active_ws_name);

        let mut attrs = WindowAttributes::default()
            .with_title(win_title)
            .with_inner_size(winit::dpi::LogicalSize::new(980.0, 620.0));

        attrs = configure_macos_window(attrs, &self.config.window, self.config.macos.option_as_alt);

        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window = Arc::new(window);
                let scale = window.scale_factor() as f32;
                self.scale_factor = scale;
                self.update_renderer();
                let size = window.inner_size();
                self.recalculate_grid(size.width as f32, size.height as f32);

                #[cfg(target_os = "macos")]
                apply_traffic_lights_visibility(&window, self.config.window.hide_traffic_lights);

                // Enable macOS IME composition (Telex, VNI, Japanese Hiragana)
                window.set_ime_allowed(true);

                let context = match softbuffer::Context::new(window.clone()) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        eprintln!("Failed to create softbuffer context: {e}");
                        return;
                    }
                };

                let surface = match softbuffer::Surface::new(&context, window.clone()) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to create softbuffer surface: {e}");
                        return;
                    }
                };

                // Spawn sessions for all tabs in the active workspace
                self.activate_current_workspace_sessions();

                self.surface = Some(surface);
                self.window = Some(window);
                self.update_window_and_process_title();
                #[cfg(target_os = "macos")]
                crate::window::macos::set_macos_app_icon(include_bytes!("../assets/icon.png"));
                info!("CelerTerm initialized successfully.");
            }
            Err(e) => eprintln!("Error creating window: {e}"),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.save_workspace_state();
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(new_mods) => {
                self.modifiers = new_mods.state();
                if self.config.macos.option_as_alt
                    && let Some(ref window) = self.window
                {
                    // Disable macOS IME dead-key interception whenever Option is held down
                    window.set_ime_allowed(!self.modifiers.alt_key());
                }
            }
            WindowEvent::Focused(is_focused) => {
                if is_focused {
                    if self.reload_workspaces_from_disk() {
                        if let Some(ref win) = self.window {
                            win.request_redraw();
                        }
                    }
                } else {
                    self.is_selecting = false;
                    self.mouse_pressed_button = None;
                    self.last_reported_mouse_grid = None;
                    self.modifiers = ModifiersState::default();
                }
            }
            WindowEvent::Occluded(occluded) => {
                if !occluded {
                    if let Some(ref win) = self.window {
                        win.request_redraw();
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x, position.y);

                let header_h = if self.config.window.tabs_in_titlebar {
                    (26.0 * self.scale_factor.max(1.0)).round()
                } else {
                    0.0
                };

                if let Some(ref win) = self.window {
                    if self.workspace_modal.is_open || self.update_modal.is_open || self.tab_color_modal.is_open || self.app_menu_open {
                        win.set_cursor(CursorIcon::Default);
                    } else if (position.y as f32) > header_h {
                        win.set_cursor(CursorIcon::Text);
                    } else {
                        win.set_cursor(CursorIcon::Default);
                    }
                }

                if self.is_selecting
                    && let Some(active_id) = self.active_tab_id()
                    && let Some(session) = self.tab_sessions.get_mut(&active_id)
                {
                    let cell_w = self.renderer.cell_width;
                    let cell_h = self.renderer.cell_height;
                    let cols = session.screen.size.columns;
                    let lines = session.screen.size.lines;

                    if cell_w > 0.0 && cell_h > 0.0 && cols > 0 && lines > 0 {
                        let pad_x = (self.config.window.padding_x * self.scale_factor).round();
                        let pad_y = (self.config.window.padding_y * self.scale_factor).round();
                        let start_y = header_h + pad_y;

                        let mx = self.mouse_pos.0 as f32;
                        let my = self.mouse_pos.1 as f32;

                        if my < start_y {
                            session.screen.scroll_display(1);
                        } else if my > start_y + (lines as f32 * cell_h) {
                            session.screen.scroll_display(-1);
                        }

                        let c = (((mx - pad_x) / cell_w).floor() as i32).clamp(0, cols as i32 - 1) as usize;
                        let l = (((my - start_y) / cell_h).floor() as i32).clamp(0, lines as i32 - 1) as usize;
                        let side = if (mx - pad_x) - (c as f32 * cell_w) < cell_w * 0.5 {
                            Side::Left
                        } else {
                            Side::Right
                        };

                        let display_offset = session.screen.display_offset();
                        let grid_line = Line(l as i32 - display_offset as i32);
                        let point = Point::new(grid_line, Column(c));

                        session.screen.update_selection(point, side);
                        if let Some(ref win) = self.window {
                            win.request_redraw();
                        }
                    }
                } else if let Some(active_id) = self.active_tab_id()
                    && let Some(session) = self.tab_sessions.get_mut(&active_id)
                    && session.screen.is_mouse_mode()
                    && !self.modifiers.shift_key()
                {
                    let cell_w = self.renderer.cell_width;
                    let cell_h = self.renderer.cell_height;
                    let cols = session.screen.size.columns;
                    let lines = session.screen.size.lines;

                    if cell_w > 0.0 && cell_h > 0.0 && cols > 0 && lines > 0 {
                        let pad_x = (self.config.window.padding_x * self.scale_factor).round();
                        let pad_y = (self.config.window.padding_y * self.scale_factor).round();
                        let start_y = header_h + pad_y;

                        let mx = self.mouse_pos.0 as f32;
                        let my = self.mouse_pos.1 as f32;

                        let col = (((mx - pad_x) / cell_w).floor() as i32 + 1).clamp(1, cols as i32) as usize;
                        let row = (((my - start_y) / cell_h).floor() as i32 + 1).clamp(1, lines as i32) as usize;

                        if let Some(pressed_btn) = self.mouse_pressed_button {
                            if session.screen.is_mouse_drag() && self.last_reported_mouse_grid != Some((col, row)) {
                                self.last_reported_mouse_grid = Some((col, row));
                                let btn_num = match pressed_btn {
                                    MouseButton::Left => 0,
                                    MouseButton::Middle => 1,
                                    MouseButton::Right => 2,
                                    _ => 0,
                                };
                                let payload = crate::term::format_sgr_mouse(
                                    btn_num,
                                    col,
                                    row,
                                    crate::term::MouseEventKind::Drag,
                                    self.modifiers.shift_key(),
                                    self.modifiers.alt_key(),
                                    self.modifiers.control_key(),
                                );
                                let _ = session.write_all(payload.as_bytes());
                                let _ = session.flush();
                            }
                        } else if session.screen.is_mouse_motion() && self.last_reported_mouse_grid != Some((col, row)) {
                            self.last_reported_mouse_grid = Some((col, row));
                            let payload = crate::term::format_sgr_mouse(
                                0,
                                col,
                                row,
                                crate::term::MouseEventKind::Move,
                                self.modifiers.shift_key(),
                                self.modifiers.alt_key(),
                                self.modifiers.control_key(),
                            );
                            let _ = session.write_all(payload.as_bytes());
                            let _ = session.flush();
                        }
                    }
                } else if (self.workspace_modal.is_open || self.update_modal.is_open || self.tab_color_modal.is_open || self.app_menu_open)
                    && let Some(ref win) = self.window
                {
                    win.request_redraw();
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                // If Option/Alt is pressed, ignore dead-key characters produced by macOS IME
                if self.modifiers.alt_key() && self.config.macos.option_as_alt {
                    self.ime_preedit = None;
                    self.last_preedit = None;
                    return;
                }
                let had_preedit = self.ime_preedit.is_some()
                    || self.last_preedit.as_ref().map(|(_, t)| t.elapsed() < std::time::Duration::from_millis(1000)).unwrap_or(false);
                let preedit_text = self.ime_preedit.as_ref().map(|(s, _)| s.clone())
                    .or_else(|| self.last_preedit.as_ref().map(|(s, _)| s.clone()));
                self.ime_preedit = None;
                self.last_preedit = None;

                // Send committed IME text (Vietnamese / Japanese) to the active tab's PTY
                if let Some(active_id) = self.active_tab_id()
                    && let Some(session) = self.tab_sessions.get_mut(&active_id)
                {
                    let _ = session.write_all(text.as_bytes());
                    // If text was committed from preedit, also send the commit key (Space, Return, Tab, punctuation, digits)
                    // unless text already contains or ends with that suffix
                    if had_preedit {
                        match get_ime_commit_action() {
                            ImeCommitAction::Append(extra) => {
                                let extra_str = String::from_utf8_lossy(&extra);
                                if !text.ends_with(extra_str.as_ref()) {
                                    let _ = session.write_all(&extra);
                                }
                            }
                            ImeCommitAction::Backspace => {
                                // Handled by WindowEvent::KeyboardInput (which sends 0x7f).
                                // Do NOT append 0x7f here (would delete 2 characters),
                                // and do NOT append fallback space (would require 2 backspace presses).
                            }
                            ImeCommitAction::Enter => {
                                let is_jp = is_japanese_input_source()
                                    || text.chars().any(is_japanese_char)
                                    || preedit_text.as_deref().map(|p| p.chars().any(is_japanese_char)).unwrap_or(false);
                                if is_jp {
                                    // Japanese IME: Enter only confirms the preedit text into the terminal buffer.
                                    // Do NOT send \r (does not execute command).
                                    self.last_ime_confirm = Some(std::time::Instant::now());
                                } else {
                                    // Vietnamese / Western: Enter immediately executes the command!
                                    // No need to press Enter twice.
                                    let _ = session.write_all(b"\r");
                                    // Guard against trailing KeyboardInput Enter within 150ms from winit
                                    self.last_ime_confirm = Some(std::time::Instant::now());
                                }
                            }
                            ImeCommitAction::ShiftEnter => {
                                // Shift+Enter in multiline prompt (Claude CLI, AGY, etc.):
                                // Send newline (\n) without submitting/executing command.
                                let _ = session.write_all(b"\n");
                                self.last_ime_confirm = Some(std::time::Instant::now());
                            }
                            ImeCommitAction::Confirm => {
                                self.last_ime_confirm = Some(std::time::Instant::now());
                            }
                            ImeCommitAction::None => {
                                if !text.ends_with(' ') && !text.ends_with('\n') && !text.ends_with('\r') {
                                    // Default fallback for Vietnamese IME commit: space key
                                    let _ = session.write_all(b" ");
                                }
                            }
                        }
                    }
                    let _ = session.flush();
                }
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::Ime(Ime::Preedit(text, cursor_range)) => {
                // Ignore dead-key composition preedits (e.g. Option+E acute '´') when Option as Alt is active
                if self.modifiers.alt_key() && self.config.macos.option_as_alt {
                    return;
                }
                if text.is_empty() {
                    self.ime_preedit = None;
                } else {
                    self.last_preedit = Some((text.clone(), std::time::Instant::now()));
                    self.ime_preedit = Some((text, cursor_range));
                }
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_x, y) => y.round() as i32,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0).round() as i32,
                };
                if self.update_modal.is_open {
                    if lines > 0 {
                        self.update_modal.scroll_offset = self.update_modal.scroll_offset.saturating_sub(lines.unsigned_abs() as usize);
                    } else if lines < 0 {
                        self.update_modal.scroll_offset += lines.unsigned_abs() as usize;
                    }
                    if let Some(ref win) = self.window {
                        win.request_redraw();
                    }
                    return;
                }
                if lines != 0
                    && let Some(active_id) = self.active_tab_id()
                    && let Some(session) = self.tab_sessions.get_mut(&active_id)
                {
                    if session.screen.is_mouse_mode() {
                        let pad_x = (self.config.window.padding_x * self.scale_factor).round();
                        let header_h = if self.config.window.tabs_in_titlebar { (26.0 * self.scale_factor).round() } else { 0.0 };
                        let pad_y = (self.config.window.padding_y * self.scale_factor).round();
                        let start_y = header_h + pad_y;

                        let col = (((self.mouse_pos.0 as f32 - pad_x) / self.renderer.cell_width).floor() as i32 + 1)
                            .clamp(1, session.screen.size.columns as i32) as usize;
                        let row = (((self.mouse_pos.1 as f32 - start_y) / self.renderer.cell_height).floor() as i32 + 1)
                            .clamp(1, session.screen.size.lines as i32) as usize;

                        // SGR mouse mode: button 64 = wheel up (lines > 0), button 65 = wheel down (lines < 0)
                        let btn = if lines > 0 { 64 } else { 65 };
                        let payload = format!("\x1b[<{};{};{}M", btn, col, row);
                        for _ in 0..lines.abs().min(5) {
                            let _ = session.write_all(payload.as_bytes());
                        }
                        let _ = session.flush();
                    } else if session.screen.is_alt_screen() {
                        // Alternate screen without mouse mode: lines > 0 is scroll up (Up Arrow), lines < 0 is scroll down (Down Arrow)
                        let arrow = if lines > 0 { b"\x1b[A" } else { b"\x1b[B" };
                        for _ in 0..lines.abs().min(5) {
                            let _ = session.write_all(arrow);
                        }
                        let _ = session.flush();
                    } else {
                        // Normal shell: lines > 0 scrolls up into history (+lines), lines < 0 scrolls down to prompt (-lines)
                        session.screen.scroll_display(lines);
                        if let Some(ref win) = self.window {
                            win.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button,
                ..
            } => {
                let window = self.window.clone();
                if let Some(ref window) = window {
                    let size = window.inner_size();
                    let (mx, my) = (self.mouse_pos.0 as f32, self.mouse_pos.1 as f32);

                    if state == ElementState::Pressed {
                        if self.update_modal.is_open {
                        if button == MouseButton::Left {
                            let (width, height) = (size.width as f32, size.height as f32);
                            let scale = self.scale_factor;
                            let modal_w = (640.0 * scale).min(width - 32.0);
                            let header_h = (36.0 * scale).round();
                            let footer_h = (42.0 * scale).round();
                            let content_h = (200.0 * scale).round();
                            let modal_h = header_h + content_h + footer_h;

                            let modal_x = ((width - modal_w) * 0.5).max(10.0);
                            let modal_y = ((height - modal_h) * 0.5).max(10.0);

                            // Outside modal -> close
                            if mx < modal_x || mx > modal_x + modal_w || my < modal_y || my > modal_y + modal_h {
                                self.update_modal.is_open = false;
                                window.request_redraw();
                                return;
                            }

                            // Header close button [Esc] Close
                            let esc_label = "[Esc] Close";
                            let esc_w = esc_label.len() as f32 * self.renderer.cell_width;
                            let esc_x = modal_x + modal_w - esc_w - (20.0 * scale);
                            if my >= modal_y && my <= modal_y + header_h && mx >= esc_x {
                                self.update_modal.is_open = false;
                                window.request_redraw();
                                return;
                            }

                            // Footer buttons
                            let modal_rect = crate::window::Rect {
                                x: modal_x,
                                y: modal_y,
                                width: modal_w,
                                height: modal_h,
                            };
                            let (primary_label, primary_color) = match &self.update_modal.state {
                                crate::update::UpdateState::Available { download_url, .. } => {
                                    if download_url.is_some() {
                                        ("[Enter] Update Now", 0x007AA2F7)
                                    } else {
                                        ("[Enter] Download", 0x007AA2F7)
                                    }
                                }
                                crate::update::UpdateState::Downloading { .. } => {
                                    ("[...] Downloading", 0x00565F89)
                                }
                                crate::update::UpdateState::ReadyToRestart { .. } => {
                                    ("[Enter] Restart & Update", 0x009ECE6A)
                                }
                                crate::update::UpdateState::Error(_) => {
                                    ("[Enter] Try Again", 0x00F7768E)
                                }
                                _ => {
                                    ("[Enter] Check Again", 0x007AA2F7)
                                }
                            };
                            let buttons = crate::window::calculate_update_modal_buttons_with_label(
                                modal_rect,
                                footer_h,
                                scale,
                                self.renderer.cell_width,
                                primary_label,
                                primary_color,
                            );

                            for btn in &buttons {
                                if btn.rect.contains(mx, my) {
                                    match btn.id {
                                        "primary" => {
                                            self.trigger_primary_update_action(event_loop);
                                        }
                                        "github" => {
                                            crate::update::open_browser(&format!("https://github.com/{}", crate::update::GITHUB_REPO));
                                        }
                                        "close" => {
                                            self.update_modal.is_open = false;
                                        }
                                        _ => {}
                                    }
                                    window.request_redraw();
                                    return;
                                }
                            }
                        }
                        return;
                    }

                    if self.tab_color_modal.is_open {
                        let (width, height) = (size.width as f32, size.height as f32);
                        let scale = self.scale_factor;
                        let modal_w = (460.0 * scale).min(width - 32.0);
                        let header_h = (34.0 * scale).round();
                        let swatches_h = (68.0 * scale).round();
                        let input_preview_h = (40.0 * scale).round();
                        let footer_h = (38.0 * scale).round();
                        let modal_h = header_h + swatches_h + input_preview_h + footer_h + (16.0 * scale);

                        let modal_x = if self.tab_color_modal.anchor_x > 0.0 || self.tab_color_modal.anchor_y > 0.0 {
                            self.tab_color_modal.anchor_x.min(width - modal_w - 16.0).max(16.0)
                        } else {
                            ((width - modal_w) * 0.5).max(10.0)
                        };
                        let modal_y = if self.tab_color_modal.anchor_x > 0.0 || self.tab_color_modal.anchor_y > 0.0 {
                            self.tab_color_modal.anchor_y.min(height - modal_h - 16.0).max(16.0)
                        } else {
                            let h_height = if self.config.window.tabs_in_titlebar { (26.0 * scale).round() } else { 0.0 };
                            (h_height + (8.0 * scale)).min(height - modal_h - 16.0).max(10.0)
                        };

                        let modal_rect = crate::window::tabs::Rect { x: modal_x, y: modal_y, width: modal_w, height: modal_h };
                        if !modal_rect.contains(mx, my) {
                            self.tab_color_modal.is_open = false;
                            window.request_redraw();
                        } else if button == MouseButton::Left {
                            // 1. Close button in header [Esc]
                            let esc_label = "[Esc] Close";
                            let esc_w = esc_label.len() as f32 * self.renderer.cell_width;
                            let esc_x = modal_x + modal_w - esc_w - (16.0 * scale);
                            if my >= modal_y && my <= modal_y + header_h && mx >= esc_x {
                                self.tab_color_modal.is_open = false;
                                window.request_redraw();
                                return;
                            }

                            // 2. Swatches clicks
                            let swatches_y = modal_y + header_h + (10.0 * scale);
                            let box_w = (32.0 * scale).round();
                            let box_h = (24.0 * scale).round();
                            let gap = (8.0 * scale).round();
                            let swatches_total_w = 10.0 * box_w + 9.0 * gap;
                            let swatches_x = modal_x + ((modal_w - swatches_total_w) * 0.5).max(0.0);

                            for i in 0..20 {
                                let col = i % 10;
                                let row = i / 10;
                                let sx = swatches_x + col as f32 * (box_w + gap);
                                let sy = swatches_y + row as f32 * (box_h + gap);
                                let s_rect = crate::window::tabs::Rect { x: sx, y: sy, width: box_w, height: box_h };
                                if s_rect.contains(mx, my) {
                                    self.tab_color_modal.selected_swatch_idx = i;
                                    self.tab_color_modal.hex_input = crate::workspace::WORKSPACE_ACCENT_PALETTE[i].hex.trim_start_matches('#').to_string();
                                    window.request_redraw();
                                    return;
                                }
                            }

                            // 3. Reset Button [r]
                            let input_y = swatches_y + (2.0 * box_h) + gap + (12.0 * scale);
                            let reset_btn_w = (140.0 * scale).round();
                            let reset_btn_h = (28.0 * scale).round();
                            let reset_btn_x = modal_x + modal_w - reset_btn_w - (16.0 * scale);
                            let reset_rect = crate::window::tabs::Rect { x: reset_btn_x, y: input_y, width: reset_btn_w, height: reset_btn_h };
                            if reset_rect.contains(mx, my) {
                                let _ = self.workspace_mgr.set_tab_color(&self.tab_color_modal.target_tab_id, None);
                                self.save_workspace_state();
                                self.tab_color_modal.is_open = false;
                                window.request_redraw();
                                return;
                            }

                            // 4. Footer buttons: Apply [Enter] vs Cancel [Esc]
                            let footer_y = modal_y + modal_h - footer_h;
                            let apply_btn_w = (120.0 * scale).round();
                            let apply_btn_h = (28.0 * scale).round();
                            let apply_rect = crate::window::tabs::Rect { x: modal_x + (16.0 * scale), y: footer_y + (4.0 * scale), width: apply_btn_w, height: apply_btn_h };
                            if apply_rect.contains(mx, my) {
                                let chosen_hex = if self.tab_color_modal.hex_input.len() == 6 && self.tab_color_modal.hex_input.chars().all(|c| c.is_ascii_hexdigit()) {
                                    format!("#{}", self.tab_color_modal.hex_input.to_lowercase())
                                } else if let Some(norm) = crate::workspace::normalize_hex(&self.tab_color_modal.hex_input) {
                                    norm
                                } else {
                                    crate::workspace::WORKSPACE_ACCENT_PALETTE[self.tab_color_modal.selected_swatch_idx % crate::workspace::WORKSPACE_ACCENT_PALETTE.len()].hex.to_string()
                                };
                                let _ = self.workspace_mgr.set_tab_color(&self.tab_color_modal.target_tab_id, Some(chosen_hex));
                                self.save_workspace_state();
                                self.tab_color_modal.is_open = false;
                                window.request_redraw();
                                return;
                            }

                            let cancel_rect = crate::window::tabs::Rect { x: modal_x + (28.0 * scale) + apply_btn_w, y: footer_y + (4.0 * scale), width: (80.0 * scale).round(), height: apply_btn_h };
                            if cancel_rect.contains(mx, my) {
                                self.tab_color_modal.is_open = false;
                                window.request_redraw();
                                return;
                            }

                            return;
                        }
                    }

                    if self.workspace_modal.is_open {
                        if button == MouseButton::Left {
                            let (width, height) = (size.width as f32, size.height as f32);
                            let scale = self.scale_factor;

                            if let WorkspaceModalMode::Coloring {
                                ref target_ws_id,
                                is_background,
                                selected_swatch_idx,
                                ref hex_input,
                            } = self.workspace_modal.mode
                            {
                                let modal_w = (620.0 * scale).min(width - 32.0);
                                let header_h = (36.0 * scale).round();
                                let tab_bar_h = (36.0 * scale).round();
                                let swatches_h = if is_background { (36.0 * scale).round() } else { (68.0 * scale).round() };
                                let input_preview_h = (44.0 * scale).round();
                                let footer_h = (42.0 * scale).round();
                                let modal_h = header_h + tab_bar_h + swatches_h + input_preview_h + footer_h + (24.0 * scale);

                                let modal_x = ((width - modal_w) * 0.5).max(10.0);
                                let modal_y = ((height - modal_h) * 0.5).max(10.0);

                                if mx < modal_x || mx > modal_x + modal_w || my < modal_y || my > modal_y + modal_h {
                                    self.workspace_modal.mode = WorkspaceModalMode::List;
                                    window.request_redraw();
                                    return;
                                }

                                let esc_label = "[Esc] Back";
                                let esc_w = esc_label.len() as f32 * self.renderer.cell_width;
                                let esc_x = modal_x + modal_w - esc_w - (20.0 * scale);
                                if my >= modal_y && my <= modal_y + header_h && mx >= esc_x {
                                    self.workspace_modal.mode = WorkspaceModalMode::List;
                                    window.request_redraw();
                                    return;
                                }

                                // Tabs (Accent vs Background)
                                let tabs_y = modal_y + header_h + (8.0 * scale);
                                let tab_w = ((modal_w - (40.0 * scale)) * 0.5).round();
                                let tab0_rect = crate::window::Rect { x: modal_x + (16.0 * scale), y: tabs_y, width: tab_w, height: (28.0 * scale).round() };
                                let tab1_rect = crate::window::Rect { x: modal_x + (24.0 * scale) + tab_w, y: tabs_y, width: tab_w, height: (28.0 * scale).round() };
                                let ws_id = target_ws_id.clone();

                                if tab0_rect.contains(mx, my) {
                                    let cur_hex = self.workspace_mgr.workspaces.iter().find(|w| w.id == ws_id)
                                        .and_then(|w| w.color.clone())
                                        .unwrap_or_else(|| "7aa2f7".to_string());
                                    self.workspace_modal.mode = WorkspaceModalMode::Coloring {
                                        target_ws_id: ws_id,
                                        is_background: false,
                                        selected_swatch_idx: 0,
                                        hex_input: cur_hex.trim_start_matches('#').to_string(),
                                    };
                                    window.request_redraw();
                                    return;
                                } else if tab1_rect.contains(mx, my) {
                                    let cur_hex = self.workspace_mgr.workspaces.iter().find(|w| w.id == ws_id)
                                        .and_then(|w| w.background.clone())
                                        .unwrap_or_else(|| "1a1b26".to_string());
                                    self.workspace_modal.mode = WorkspaceModalMode::Coloring {
                                        target_ws_id: ws_id,
                                        is_background: true,
                                        selected_swatch_idx: 0,
                                        hex_input: cur_hex.trim_start_matches('#').to_string(),
                                    };
                                    window.request_redraw();
                                    return;
                                }

                                // Swatches
                                let swatches_y = tabs_y + tab_bar_h + (4.0 * scale);
                                let pad_x = modal_x + (16.0 * scale);
                                let avail_w = modal_w - (32.0 * scale);
                                let (cols, count) = if is_background {
                                    (10, crate::workspace::WORKSPACE_BACKGROUND_PALETTE.len())
                                } else {
                                    (10, crate::workspace::WORKSPACE_ACCENT_PALETTE.len())
                                };
                                let gap = 6.0 * scale;
                                let swatch_w = (avail_w - ((cols - 1) as f32 * gap)) / cols as f32;
                                let swatch_h = (26.0 * scale).round();

                                for i in 0..count {
                                    let col = i % cols;
                                    let row = i / cols;
                                    let sx = pad_x + col as f32 * (swatch_w + gap);
                                    let sy = swatches_y + row as f32 * (swatch_h + gap);
                                    let s_rect = crate::window::Rect { x: sx, y: sy, width: swatch_w, height: swatch_h };
                                    if s_rect.contains(mx, my) {
                                        let h = if is_background {
                                            crate::workspace::WORKSPACE_BACKGROUND_PALETTE[i].hex.trim_start_matches('#').to_string()
                                        } else {
                                            crate::workspace::WORKSPACE_ACCENT_PALETTE[i].hex.trim_start_matches('#').to_string()
                                        };
                                        self.workspace_modal.mode = WorkspaceModalMode::Coloring {
                                            target_ws_id: ws_id,
                                            is_background,
                                            selected_swatch_idx: i,
                                            hex_input: h,
                                        };
                                        window.request_redraw();
                                        return;
                                    }
                                }

                                // Footer buttons in coloring mode:
                                let footer_y = modal_y + modal_h - footer_h;
                                let btn_h = (footer_h - 14.0 * scale).max(20.0);
                                let btn_y = footer_y + ((footer_h - btn_h) * 0.5);
                                let btn_w = (110.0 * scale).round();
                                let apply_btn = crate::window::Rect { x: modal_x + (16.0 * scale), y: btn_y, width: btn_w, height: btn_h };
                                let reset_btn = crate::window::Rect { x: modal_x + (24.0 * scale) + btn_w, y: btn_y, width: btn_w, height: btn_h };
                                let back_btn = crate::window::Rect { x: modal_x + (32.0 * scale) + (btn_w * 2.0), y: btn_y, width: btn_w, height: btn_h };

                                if apply_btn.contains(mx, my) {
                                    let chosen_hex = if hex_input.len() == 6 && hex_input.chars().all(|c| c.is_ascii_hexdigit()) {
                                        format!("#{}", hex_input.to_lowercase())
                                    } else if let Some(norm) = crate::workspace::normalize_hex(hex_input) {
                                        norm
                                    } else if is_background {
                                        crate::workspace::WORKSPACE_BACKGROUND_PALETTE[selected_swatch_idx % crate::workspace::WORKSPACE_BACKGROUND_PALETTE.len()].hex.to_string()
                                    } else {
                                        crate::workspace::WORKSPACE_ACCENT_PALETTE[selected_swatch_idx % crate::workspace::WORKSPACE_ACCENT_PALETTE.len()].hex.to_string()
                                    };

                                    if is_background {
                                        let _ = self.workspace_mgr.set_workspace_background(&ws_id, Some(chosen_hex));
                                    } else {
                                        let _ = self.workspace_mgr.set_workspace_color(&ws_id, Some(chosen_hex));
                                    }
                                    self.save_workspace_state();
                                    self.workspace_modal.mode = WorkspaceModalMode::List;
                                    window.request_redraw();
                                    return;
                                } else if reset_btn.contains(mx, my) {
                                    if is_background {
                                        let _ = self.workspace_mgr.set_workspace_background(&ws_id, None);
                                    } else {
                                        let _ = self.workspace_mgr.set_workspace_color(&ws_id, None);
                                    }
                                    self.save_workspace_state();
                                    self.workspace_modal.mode = WorkspaceModalMode::List;
                                    window.request_redraw();
                                    return;
                                } else if back_btn.contains(mx, my) {
                                    self.workspace_modal.mode = WorkspaceModalMode::List;
                                    window.request_redraw();
                                    return;
                                }
                                return;
                            }

                            let modal_w = (580.0 * scale).min(width - 32.0);
                            let header_h = (36.0 * scale).round();
                            let row_h = (32.0 * scale).round();
                            let ws_count = self.workspace_mgr.workspaces.len();
                            let list_h = (ws_count as f32 * row_h).max(32.0 * scale);
                            let is_editing = matches!(self.workspace_modal.mode, WorkspaceModalMode::Renaming { .. } | WorkspaceModalMode::Creating { .. });
                            let edit_h = if is_editing { (36.0 * scale).round() } else { 0.0 };
                            let footer_h = (42.0 * scale).round();
                            let modal_h = header_h + list_h + edit_h + footer_h + (16.0 * scale);

                            let modal_x = ((width - modal_w) * 0.5).max(10.0);
                            let modal_y = ((height - modal_h) * 0.5).max(10.0);

                            // If clicked outside modal, close it
                            if mx < modal_x || mx > modal_x + modal_w || my < modal_y || my > modal_y + modal_h {
                                self.workspace_modal.is_open = false;
                                self.workspace_modal.mode = WorkspaceModalMode::List;
                                window.request_redraw();
                                return;
                            }

                            // Close button on top right of header
                            let esc_label = "[Esc] Close";
                            let esc_w = esc_label.len() as f32 * self.renderer.cell_width;
                            let esc_x = modal_x + modal_w - esc_w - (20.0 * scale);
                            if my >= modal_y && my <= modal_y + header_h && mx >= esc_x {
                                self.workspace_modal.is_open = false;
                                self.workspace_modal.mode = WorkspaceModalMode::List;
                                window.request_redraw();
                                return;
                            }

                            // Check list rows
                            let list_top = modal_y + header_h + (8.0 * scale);
                            if my >= list_top && my < list_top + list_h {
                                let clicked_idx = ((my - list_top) / row_h).floor() as usize;
                                if clicked_idx < ws_count {
                                    let dot_x = modal_x + (34.0 * scale);
                                    let dot_w = 20.0 * scale;
                                    if mx >= dot_x && mx <= dot_x + dot_w {
                                        // Click directly on color dot opens color picker for this workspace!
                                        self.workspace_modal.selected_index = clicked_idx;
                                        let ws = &self.workspace_mgr.workspaces[clicked_idx];
                                        let initial_hex = ws.color.clone().unwrap_or_else(|| ws.effective_color_hex(clicked_idx));
                                        self.workspace_modal.mode = WorkspaceModalMode::Coloring {
                                            target_ws_id: ws.id.clone(),
                                            is_background: false,
                                            selected_swatch_idx: 0,
                                            hex_input: initial_hex.trim_start_matches('#').to_string(),
                                        };
                                        window.request_redraw();
                                        return;
                                    }

                                    if self.workspace_modal.selected_index == clicked_idx {
                                        // Clicking selected row switches to it
                                        let target_id = self.workspace_mgr.workspaces[clicked_idx].id.clone();
                                        self.switch_or_focus_workspace(&target_id);
                                        self.workspace_modal.is_open = false;
                                    } else {
                                        self.workspace_modal.selected_index = clicked_idx;
                                    }
                                    window.request_redraw();
                                    return;
                                }
                            }

                            // Check footer button chips
                            let modal_rect = crate::window::Rect {
                                x: modal_x,
                                y: modal_y,
                                width: modal_w,
                                height: modal_h,
                            };
                            let footer_buttons = crate::window::calculate_modal_buttons(
                                modal_rect,
                                footer_h,
                                scale,
                                self.renderer.cell_width,
                            );

                            for btn in &footer_buttons {
                                if btn.rect.contains(mx, my) {
                                    match btn.id {
                                        "new" => {
                                            self.workspace_modal.mode = WorkspaceModalMode::Creating { input: String::new() };
                                            window.request_redraw();
                                            return;
                                        }
                                        "rename" => {
                                            let idx = self.workspace_modal.selected_index;
                                            if let Some(ws) = self.workspace_mgr.workspaces.get(idx) {
                                                self.workspace_modal.mode = WorkspaceModalMode::Renaming { input: ws.name.clone() };
                                            }
                                            window.request_redraw();
                                            return;
                                        }
                                        "color" => {
                                            let idx = self.workspace_modal.selected_index;
                                            if let Some(ws) = self.workspace_mgr.workspaces.get(idx) {
                                                let initial_hex = ws.color.clone().unwrap_or_else(|| ws.effective_color_hex(idx));
                                                self.workspace_modal.mode = WorkspaceModalMode::Coloring {
                                                    target_ws_id: ws.id.clone(),
                                                    is_background: false,
                                                    selected_swatch_idx: 0,
                                                    hex_input: initial_hex.trim_start_matches('#').to_string(),
                                                };
                                            }
                                            window.request_redraw();
                                            return;
                                        }
                                        "delete" => {
                                            if self.workspace_mgr.workspaces.len() > 1 {
                                                let idx = self.workspace_modal.selected_index;
                                                if let Some(ws) = self.workspace_mgr.workspaces.get(idx) {
                                                    let target_id = ws.id.clone();
                                                    let my_pid = std::process::id();
                                                    if crate::workspace::find_other_instance_for_workspace(my_pid, &target_id, &ws.name).is_some() {
                                                        log::warn!("Cannot delete workspace '{}' because it is active in another window.", ws.name);
                                                    } else {
                                                        for tab in &ws.tabs {
                                                            self.tab_sessions.remove(&tab.id);
                                                        }
                                                        self.deleted_workspace_ids.push(target_id.clone());
                                                        let _ = self.workspace_mgr.delete_workspace(&target_id);
                                                        self.workspace_modal.selected_index = self.workspace_modal.selected_index.min(self.workspace_mgr.workspaces.len().saturating_sub(1));
                                                        self.activate_current_workspace_sessions();
                                                        self.save_workspace_state();
                                                    }
                                                }
                                            }
                                            window.request_redraw();
                                            return;
                                        }
                                        "window" => {
                                            let idx = self.workspace_modal.selected_index;
                                            self.open_workspace_in_new_window(idx);
                                            self.workspace_modal.is_open = false;
                                            window.request_redraw();
                                            return;
                                        }
                                        "switch" => {
                                            let idx = self.workspace_modal.selected_index;
                                            if let Some(ws) = self.workspace_mgr.workspaces.get(idx) {
                                                let target_id = ws.id.clone();
                                                self.switch_or_focus_workspace(&target_id);
                                            }
                                            self.workspace_modal.is_open = false;
                                            window.request_redraw();
                                            return;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        return;
                    }

                    let raw_tabs = self.workspace_mgr.get_active_workspace()
                        .map(|ws| ws.tabs.clone())
                        .unwrap_or_default();
                    let tabs: Vec<(String, String)> = raw_tabs.iter().enumerate().map(|(idx, t)| {
                        (t.id.clone(), format!("{}. {}", idx + 1, t.title))
                    }).collect();

                    let header = calculate_header_layout(
                        size.width as f32,
                        &tabs,
                        self.config.window.hide_traffic_lights,
                        self.config.window.tabs_in_titlebar,
                        self.scale_factor,
                        self.renderer.cell_width,
                    );

                    if self.app_menu_open && button == MouseButton::Left {
                        let scale = self.scale_factor;
                        let menu_w = (220.0 * scale).round();
                        let item_h = (28.0 * scale).round();
                        let menu_pad = (6.0 * scale).round();
                        let menu_h = (5.0 * item_h) + (menu_pad * 2.0);
                        let menu_btn = header.menu_button_rect;
                        let menu_x = (menu_btn.x + menu_btn.width - menu_w).max(8.0 * scale);
                        let menu_y = header.height + (2.0 * scale);

                        if mx >= menu_x && mx <= menu_x + menu_w && my >= menu_y && my <= menu_y + menu_h {
                            let item_idx = ((my - (menu_y + menu_pad)) / item_h).floor() as usize;
                            self.app_menu_open = false;
                            match item_idx {
                                0 => {
                                    self.start_check_for_updates();
                                }
                                1 => {
                                    self.reload_workspaces_from_disk();
                                    self.workspace_modal.is_open = true;
                                    self.workspace_modal.mode = WorkspaceModalMode::List;
                                    if let Some(pos) = self.workspace_mgr.workspaces.iter().position(|w| w.id == self.workspace_mgr.active_workspace_id) {
                                        self.workspace_modal.selected_index = pos;
                                    }
                                }
                                2 => {
                                    self.reload_config();
                                }
                                3 => {
                                    crate::update::open_browser(&format!("https://github.com/{}", crate::update::GITHUB_REPO));
                                }
                                4 => {
                                    self.update_modal.is_open = true;
                                    self.update_modal.state = crate::update::UpdateState::UpToDate {
                                        current_version: crate::update::CURRENT_VERSION.to_string(),
                                    };
                                }
                                _ => {}
                            }
                            window.request_redraw();
                            return;
                        } else {
                            self.app_menu_open = false;
                            if !header.menu_button_rect.contains(mx, my) {
                                window.request_redraw();
                            }
                        }
                    }

                    // Check click in header
                    if my <= header.height {
                        let mut clicked_tab = false;
                        for (tab_id, rect) in &header.tab_rects {
                            if rect.contains(mx, my) {
                                // Middle-click or clicking on 'x' at tab's right edge closes tab
                                let close_area_w = (16.0 * self.scale_factor).max(12.0);
                                if button == MouseButton::Middle || mx >= rect.x + rect.width - close_area_w {
                                    if self.tab_sessions.len() <= 1 {
                                        self.save_workspace_state();
                                        event_loop.exit();
                                        return;
                                    }
                                    if let Some(session) = self.tab_sessions.get(tab_id) {
                                        for ws in &mut self.workspace_mgr.workspaces {
                                            if let Some(tab) = ws.tabs.iter_mut().find(|t| t.id == *tab_id)
                                                && self.config.workspace.save_scrollback
                                            {
                                                tab.scrollback_cache = session.screen.get_scrollback_lines(self.config.workspace.max_scrollback_lines);
                                            }
                                        }
                                    }
                                    self.tab_sessions.remove(tab_id);
                                    let _ = self.workspace_mgr.close_tab(tab_id);
                                    self.save_workspace_state();
                                    window.request_redraw();
                                } else if button == MouseButton::Left
                                    && let Some(ws) = self.workspace_mgr.get_active_workspace_mut()
                                {
                                    ws.active_tab_id = tab_id.clone();
                                    self.ensure_tab_session(tab_id);
                                    window.request_redraw();
                                } else if button == MouseButton::Right {
                                    self.open_tab_color_modal(tab_id, rect.x, rect.y + rect.height);
                                    window.request_redraw();
                                }
                                clicked_tab = true;
                                break;
                            }
                        }

                        // Check click on ☰ Menu Button
                        if !clicked_tab && button == MouseButton::Left && header.menu_button_rect.contains(mx, my) {
                            self.app_menu_open = !self.app_menu_open;
                            window.request_redraw();
                            return;
                        }

                        // Check click on Workspace name badge
                        let ws_name = self.workspace_mgr.get_active_workspace().map(|w| w.name.as_str()).unwrap_or("Default");
                        let ws_label = format!("● {}", ws_name);
                        let ws_w = ws_label.len() as f32 * self.renderer.cell_width + (16.0 * self.scale_factor);
                        let ws_h = (header.height - 6.0 * self.scale_factor).max(16.0);
                        let ws_x = header.menu_button_rect.x - ws_w - (8.0 * self.scale_factor);
                        let ws_y = ((header.height - ws_h) * 0.5).max(0.0);
                        let ws_rect = crate::window::tabs::Rect { x: ws_x, y: ws_y, width: ws_w, height: ws_h };

                        if !clicked_tab && button == MouseButton::Left && ws_rect.contains(mx, my) {
                            self.reload_workspaces_from_disk();
                            self.workspace_modal.is_open = true;
                            self.workspace_modal.mode = WorkspaceModalMode::List;
                            if let Some(pos) = self.workspace_mgr.workspaces.iter().position(|w| w.id == self.workspace_mgr.active_workspace_id) {
                                self.workspace_modal.selected_index = pos;
                            }
                            window.request_redraw();
                            return;
                        }

                        if !clicked_tab && button == MouseButton::Left && header.add_button_rect.contains(mx, my) {
                            let cwd = self.get_active_tab_cwd();
                            if let Ok(new_tab_id) = self.workspace_mgr.new_tab(cwd.clone()) {
                                let _ = self.spawn_tab_session(&new_tab_id, Some(&cwd));
                                self.save_workspace_state();
                                window.request_redraw();
                            }
                        } else if !clicked_tab && button == MouseButton::Left {
                            let _ = window.drag_window();
                        }
                    }

                    // Check click in terminal area
                    if my > header.height {
                        let btn_code = match button {
                            MouseButton::Left => Some(0),
                            MouseButton::Middle => Some(1),
                            MouseButton::Right => Some(2),
                            _ => None,
                        };

                        let pad_x = (self.config.window.padding_x * self.scale_factor).round();
                        let pad_y = (self.config.window.padding_y * self.scale_factor).round();
                        let start_y = header.height + pad_y;
                        let cell_w = self.renderer.cell_width;
                        let cell_h = self.renderer.cell_height;

                        if cell_w > 0.0 && cell_h > 0.0
                            && let Some(active_id) = self.active_tab_id()
                            && let Some(session) = self.tab_sessions.get_mut(&active_id)
                        {
                            let cols = session.screen.size.columns;
                            let lines = session.screen.size.lines;

                            if cols > 0 && lines > 0 {
                                if session.screen.is_mouse_mode() && !self.modifiers.shift_key() {
                                    if let Some(btn_num) = btn_code {
                                        self.mouse_pressed_button = Some(button);
                                        let col = (((mx - pad_x) / cell_w).floor() as i32 + 1).clamp(1, cols as i32) as usize;
                                        let row = (((my - start_y) / cell_h).floor() as i32 + 1).clamp(1, lines as i32) as usize;
                                        self.last_reported_mouse_grid = Some((col, row));

                                        let payload = crate::term::format_sgr_mouse(
                                            btn_num,
                                            col,
                                            row,
                                            crate::term::MouseEventKind::Press,
                                            self.modifiers.shift_key(),
                                            self.modifiers.alt_key(),
                                            self.modifiers.control_key(),
                                        );
                                        let _ = session.write_all(payload.as_bytes());
                                        let _ = session.flush();
                                    }
                                } else if button == MouseButton::Left {
                                    let c = (((mx - pad_x) / cell_w).floor() as i32).clamp(0, cols as i32 - 1) as usize;
                                    let l = (((my - start_y) / cell_h).floor() as i32).clamp(0, lines as i32 - 1) as usize;
                                    let side = if (mx - pad_x) - (c as f32 * cell_w) < cell_w * 0.5 {
                                        Side::Left
                                    } else {
                                        Side::Right
                                    };

                                    let display_offset = session.screen.display_offset();
                                    let grid_line = Line(l as i32 - display_offset as i32);
                                    let point = Point::new(grid_line, Column(c));

                                    let now = std::time::Instant::now();
                                    let is_multi_click = self.last_click
                                        .map(|(t, last_pt)| {
                                            now.duration_since(t).as_millis() < 400
                                                && (last_pt.line.0 - point.line.0).abs() <= 1
                                                && (last_pt.column.0 as i32 - point.column.0 as i32).abs() <= 2
                                        })
                                        .unwrap_or(false);

                                    if is_multi_click {
                                        self.click_count = (self.click_count % 3) + 1;
                                    } else {
                                        self.click_count = 1;
                                    }
                                    self.last_click = Some((now, point));

                                    let sel_type = if self.modifiers.alt_key() {
                                        SelectionType::Block
                                    } else {
                                        match self.click_count {
                                            2 => SelectionType::Semantic,
                                            3 => SelectionType::Lines,
                                            _ => SelectionType::Simple,
                                        }
                                    };

                                    session.screen.start_selection(sel_type, point, side);
                                    self.is_selecting = true;
                                    window.request_redraw();
                                }
                            }
                        }
                    }
                } else if state == ElementState::Released {
                    self.mouse_pressed_button = None;
                    self.last_reported_mouse_grid = None;

                    if self.is_selecting && button == MouseButton::Left {
                        self.is_selecting = false;
                        if let Some(active_id) = self.active_tab_id()
                            && let Some(session) = self.tab_sessions.get_mut(&active_id)
                        {
                            if let Some(ref sel) = session.screen.term.selection {
                                if sel.is_empty() {
                                    session.screen.clear_selection();
                                }
                            }
                        }
                        window.request_redraw();
                    } else if let Some(active_id) = self.active_tab_id()
                        && let Some(session) = self.tab_sessions.get_mut(&active_id)
                        && session.screen.is_mouse_mode()
                        && !self.modifiers.shift_key()
                    {
                        let btn_code = match button {
                            MouseButton::Left => Some(0),
                            MouseButton::Middle => Some(1),
                            MouseButton::Right => Some(2),
                            _ => None,
                        };

                        if let Some(btn_num) = btn_code {
                            let header_h = if self.config.window.tabs_in_titlebar {
                                (26.0 * self.scale_factor.max(1.0)).round()
                            } else {
                                0.0
                            };
                            let pad_x = (self.config.window.padding_x * self.scale_factor).round();
                            let pad_y = (self.config.window.padding_y * self.scale_factor).round();
                            let start_y = header_h + pad_y;
                            let cell_w = self.renderer.cell_width;
                            let cell_h = self.renderer.cell_height;
                            let cols = session.screen.size.columns;
                            let lines = session.screen.size.lines;
                            if cell_w > 0.0 && cell_h > 0.0 && cols > 0 && lines > 0 {
                                let col = (((mx - pad_x) / cell_w).floor() as i32 + 1).clamp(1, cols as i32) as usize;
                                let row = (((my - start_y) / cell_h).floor() as i32 + 1).clamp(1, lines as i32) as usize;
                                let payload = crate::term::format_sgr_mouse(
                                    btn_num,
                                    col,
                                    row,
                                    crate::term::MouseEventKind::Release,
                                    self.modifiers.shift_key(),
                                    self.modifiers.alt_key(),
                                    self.modifiers.control_key(),
                                );
                                let _ = session.write_all(payload.as_bytes());
                                let _ = session.flush();
                            }
                        }
                    }
                }
        }
    }
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    logical_key,
                    physical_key,
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } => {
                let window = self.window.clone();

                if self.update_modal.is_open {
                    self.handle_update_modal_key(event_loop, &logical_key, match physical_key {
                        PhysicalKey::Code(c) => Some(c),
                        _ => None,
                    });
                    if let Some(ref win) = window {
                        win.request_redraw();
                    }
                    return;
                }

                if self.tab_color_modal.is_open {
                    self.handle_tab_color_modal_key(&logical_key);
                    if let Some(ref win) = window {
                        win.request_redraw();
                    }
                    return;
                }

                if self.workspace_modal.is_open {
                    self.handle_modal_key(&logical_key, match physical_key {
                        PhysicalKey::Code(c) => Some(c),
                        _ => None,
                    });
                    if let Some(ref win) = window {
                        win.request_redraw();
                    }
                    return;
                }

                if self.app_menu_open
                    && matches!(logical_key, winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape))
                {
                    self.app_menu_open = false;
                    if let Some(ref win) = window {
                        win.request_redraw();
                    }
                    return;
                }

                let is_enter_key = matches!(logical_key, winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter))
                    || matches!(physical_key, PhysicalKey::Code(winit::keyboard::KeyCode::Enter | winit::keyboard::KeyCode::NumpadEnter));
                if is_enter_key {
                    if let Some(t) = self.last_ime_confirm.take() {
                        if t.elapsed() < std::time::Duration::from_millis(150) {
                            // This Enter key was consumed by IME to confirm preedit text.
                            // Do NOT forward newline/carriage return to the shell.
                            if let Some(ref win) = window {
                                win.request_redraw();
                            }
                            return;
                        }
                    }
                }

                #[cfg(target_os = "macos")]
                let mods = unsafe {
                    let flags = objc2_app_kit::NSEvent::modifierFlags_class();
                    Modifiers {
                        shift: flags.contains(objc2_app_kit::NSEventModifierFlags::NSEventModifierFlagShift),
                        ctrl: flags.contains(objc2_app_kit::NSEventModifierFlags::NSEventModifierFlagControl),
                        alt: flags.contains(objc2_app_kit::NSEventModifierFlags::NSEventModifierFlagOption),
                        logo: flags.contains(objc2_app_kit::NSEventModifierFlags::NSEventModifierFlagCommand),
                    }
                };

                #[cfg(not(target_os = "macos"))]
                let mods = Modifiers {
                    alt: self.modifiers.alt_key(),
                    ctrl: self.modifiers.control_key(),
                    shift: self.modifiers.shift_key(),
                    logo: self.modifiers.super_key(),
                };

                let phys_code = match physical_key {
                    PhysicalKey::Code(code) => Some(code),
                    _ => None,
                };

                if let Some(action) = translate_key_event(&logical_key, phys_code, mods, self.config.macos.option_as_alt) {
                    match action {
                        KeyAction::NewTab => {
                            let cwd = self.get_active_tab_cwd();
                            if let Ok(new_tab_id) = self.workspace_mgr.new_tab(cwd.clone()) {
                                let _ = self.spawn_tab_session(&new_tab_id, Some(&cwd));
                                self.save_workspace_state();
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::CloseTab => {
                            if let Some(active_id) = self.active_tab_id() {
                                if self.tab_sessions.len() <= 1 {
                                    self.save_workspace_state();
                                    crate::workspace::unregister_active_instance(std::process::id());
                                    event_loop.exit();
                                    return;
                                }
                                if let Some(session) = self.tab_sessions.get(&active_id) {
                                    for ws in &mut self.workspace_mgr.workspaces {
                                        if let Some(tab) = ws.tabs.iter_mut().find(|t| t.id == active_id)
                                            && self.config.workspace.save_scrollback
                                        {
                                            tab.scrollback_cache = session.screen.get_scrollback_lines(self.config.workspace.max_scrollback_lines);
                                        }
                                    }
                                }
                                self.tab_sessions.remove(&active_id);
                                let _ = self.workspace_mgr.close_tab(&active_id);
                                self.save_workspace_state();
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::Quit => {
                            self.save_workspace_state();
                            crate::workspace::unregister_active_instance(std::process::id());
                            event_loop.exit();
                        }
                        KeyAction::NewWorkspace => {
                            let ws_count = self.workspace_mgr.workspaces.len() + 1;
                            let ws_name = format!("Workspace {}", ws_count);
                            if let Ok(_new_ws_id) = self.workspace_mgr.new_workspace(&ws_name)
                                && let Some(active_id) = self.active_tab_id()
                            {
                                let cwd = self.get_active_tab_cwd();
                                let _ = self.spawn_tab_session(&active_id, Some(&cwd));
                                self.save_workspace_state();
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::PreviousWorkspace => {
                            self.reload_workspaces_from_disk();
                            if !self.workspace_mgr.workspaces.is_empty() {
                                let current_pos = self.workspace_mgr.workspaces.iter().position(|w| w.id == self.workspace_mgr.active_workspace_id).unwrap_or(0);
                                let prev_pos = if current_pos == 0 {
                                    self.workspace_mgr.workspaces.len() - 1
                                } else {
                                    current_pos - 1
                                };
                                let target_id = self.workspace_mgr.workspaces[prev_pos].id.clone();
                                if self.switch_or_focus_workspace(&target_id)
                                    && let Some(ref win) = window
                                {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::NextWorkspace => {
                            self.reload_workspaces_from_disk();
                            if !self.workspace_mgr.workspaces.is_empty() {
                                let current_pos = self.workspace_mgr.workspaces.iter().position(|w| w.id == self.workspace_mgr.active_workspace_id).unwrap_or(0);
                                let next_pos = (current_pos + 1) % self.workspace_mgr.workspaces.len();
                                let target_id = self.workspace_mgr.workspaces[next_pos].id.clone();
                                if self.switch_or_focus_workspace(&target_id)
                                    && let Some(ref win) = window
                                {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::CycleNextWindow => {
                            crate::workspace::cycle_next_instance(std::process::id());
                        }
                        KeyAction::ToggleWorkspaceModal => {
                            self.reload_workspaces_from_disk();
                            self.workspace_modal.is_open = !self.workspace_modal.is_open;
                            self.workspace_modal.mode = WorkspaceModalMode::List;
                            if let Some(pos) = self.workspace_mgr.workspaces.iter().position(|w| w.id == self.workspace_mgr.active_workspace_id) {
                                self.workspace_modal.selected_index = pos;
                            }
                            if let Some(ref win) = window {
                                win.request_redraw();
                            }
                        }
                        KeyAction::CustomizeTabColor => {
                            if let Some(active_id) = self.active_tab_id() {
                                self.open_tab_color_modal(&active_id, 0.0, 0.0);
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::ReloadConfig => {
                            self.reload_config();
                        }
                        KeyAction::CheckForUpdates => {
                            self.start_check_for_updates();
                        }
                        KeyAction::SelectTab(idx) => {
                            if let Ok(tab_id) = self.workspace_mgr.select_tab_by_1_index(idx) {
                                self.ensure_tab_session(&tab_id);
                                self.save_workspace_state();
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::PreviousTab => {
                            if let Ok(tab_id) = self.workspace_mgr.select_previous_tab() {
                                self.ensure_tab_session(&tab_id);
                                self.save_workspace_state();
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::NextTab => {
                            if let Ok(tab_id) = self.workspace_mgr.select_next_tab() {
                                self.ensure_tab_session(&tab_id);
                                self.save_workspace_state();
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::MoveTabLeft => {
                            if self.workspace_mgr.move_active_tab_left().is_ok() {
                                self.save_workspace_state();
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::MoveTabRight => {
                            if self.workspace_mgr.move_active_tab_right().is_ok() {
                                self.save_workspace_state();
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::Paste => {
                            if let Ok(mut clipboard) = arboard::Clipboard::new()
                                && let Ok(text) = clipboard.get_text()
                                && let Some(active_id) = self.active_tab_id()
                                && let Some(session) = self.tab_sessions.get_mut(&active_id)
                            {
                                session.screen.scroll_to_bottom();
                                if session.screen.is_bracketed_paste() {
                                    let mut payload = Vec::with_capacity(text.len() + 12);
                                    payload.extend_from_slice(b"\x1b[200~");
                                    payload.extend_from_slice(text.as_bytes());
                                    payload.extend_from_slice(b"\x1b[201~");
                                    let _ = session.write_all(&payload);
                                } else {
                                    let _ = session.write_all(text.as_bytes());
                                }
                                let _ = session.flush();
                            }
                        }
                        KeyAction::Copy => {
                            if let Some(active_id) = self.active_tab_id()
                                && let Some(session) = self.tab_sessions.get(&active_id)
                                && let Some(text) = session.screen.copy_selection_text()
                                && !text.is_empty()
                            {
                                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                    let _ = clipboard.set_text(text);
                                }
                            }
                        }
                        KeyAction::ClearScreen => {
                            if let Some(active_id) = self.active_tab_id()
                                && let Some(session) = self.tab_sessions.get_mut(&active_id)
                            {
                                let _ = session.write_all(b"\x0c");
                                let _ = session.flush();
                            }
                        }
                        KeyAction::IncreaseFontSize => {
                            let new_size = (self.config.font.size + 1.0).min(32.0);
                            self.config.font.size = new_size;
                            self.update_renderer();
                            if let Some(ref win) = window {
                                let size = win.inner_size();
                                self.recalculate_grid(size.width as f32, size.height as f32);
                                win.request_redraw();
                            }
                        }
                        KeyAction::DecreaseFontSize => {
                            let new_size = (self.config.font.size - 1.0).max(9.0);
                            self.config.font.size = new_size;
                            self.update_renderer();
                            if let Some(ref win) = window {
                                let size = win.inner_size();
                                self.recalculate_grid(size.width as f32, size.height as f32);
                                win.request_redraw();
                            }
                        }
                        KeyAction::ResetFontSize => {
                            let default_size = 13.0;
                            self.config.font.size = default_size;
                            self.update_renderer();
                            if let Some(ref win) = window {
                                let size = win.inner_size();
                                self.recalculate_grid(size.width as f32, size.height as f32);
                                win.request_redraw();
                            }
                        }
                        KeyAction::Bytes(bytes) => {
                            if let Some(active_id) = self.active_tab_id()
                                && let Some(session) = self.tab_sessions.get_mut(&active_id)
                            {
                                session.screen.scroll_to_bottom();
                                let _ = session.write_all(&bytes);
                                let _ = session.flush();
                            }
                        }
                        KeyAction::Text(text) => {
                            if let Some(active_id) = self.active_tab_id()
                                && let Some(session) = self.tab_sessions.get_mut(&active_id)
                            {
                                session.screen.scroll_to_bottom();
                                let _ = session.write_all(text.as_bytes());
                                let _ = session.flush();
                            }
                        }
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale_factor = scale_factor as f32;
                self.update_renderer();
                let window = self.window.clone();
                if let Some(ref win) = window {
                    let size = win.inner_size();
                    self.recalculate_grid(size.width as f32, size.height as f32);
                    win.request_redraw();
                }
            }
            WindowEvent::Resized(new_size) => {
                if let (Some(w), Some(h)) = (NonZeroU32::new(new_size.width), NonZeroU32::new(new_size.height)) {
                    if let Some(ref mut surface) = self.surface {
                        let _ = surface.resize(w, h);
                    }
                    self.recalculate_grid(new_size.width as f32, new_size.height as f32);
                }
            }
            WindowEvent::RedrawRequested => {
                // Update dynamic tab titles & cwds
                self.update_tab_titles();

                if let (Some(window), Some(surface)) = (&self.window, &mut self.surface) {
                    let win_size = window.inner_size();
                    let width = win_size.width as usize;
                    let height = win_size.height as usize;

                    if width == 0 || height == 0 {
                        return;
                    }

                    if let (Some(w), Some(h)) = (NonZeroU32::new(win_size.width), NonZeroU32::new(win_size.height)) {
                        if surface.resize(w, h).is_err() {
                            if let Ok(ctx) = softbuffer::Context::new(window.clone())
                                && let Ok(mut new_surface) = softbuffer::Surface::new(&ctx, window.clone())
                            {
                                let _ = new_surface.resize(w, h);
                                *surface = new_surface;
                            }
                        }
                    }

                    if surface.buffer_mut().is_err()
                        && let (Some(w), Some(h)) = (NonZeroU32::new(win_size.width), NonZeroU32::new(win_size.height))
                        && let Ok(ctx) = softbuffer::Context::new(window.clone())
                        && let Ok(mut new_surface) = softbuffer::Surface::new(&ctx, window.clone())
                    {
                        let _ = new_surface.resize(w, h);
                        *surface = new_surface;
                    }

                    let mut buffer = match surface.buffer_mut() {
                        Ok(buf) => buf,
                        Err(_) => return,
                    };

                    #[cfg(target_os = "macos")]
                    apply_traffic_lights_visibility(window, self.config.window.hide_traffic_lights);

                    // 1. Fill terminal background with workspace background (or Tokyo Night default #1a1b26)
                    let active_ws = self.workspace_mgr.get_active_workspace();
                    let default_config_bg = parse_hex_color(&self.config.colors.background, 0x001A1B26);
                    let effective_bg = active_ws.map(|w| w.effective_background_u32(default_config_bg)).unwrap_or(default_config_bg);
                    let active_accent = active_ws
                        .map(|w| {
                            let idx = self.workspace_mgr.workspaces.iter().position(|ws| ws.id == w.id).unwrap_or(0);
                            w.effective_color_u32(idx)
                        })
                        .unwrap_or(0x007AA2F7);

                    buffer.fill(effective_bg);

                    // 2. Render Tab Header
                    let raw_tabs = self.workspace_mgr.get_active_workspace()
                        .map(|ws| ws.tabs.clone())
                        .unwrap_or_default();

                    let active_tab_id = self.workspace_mgr.get_active_workspace()
                        .map(|ws| ws.active_tab_id.clone())
                        .unwrap_or_default();

                    let tabs: Vec<(String, String)> = raw_tabs.iter().enumerate().map(|(idx, t)| {
                        let is_active = t.id == active_tab_id;
                        let has_custom = t.color.is_some();
                        let prefix = if is_active || has_custom { "● " } else { "" };
                        (t.id.clone(), format!("{}{}. {}", prefix, idx + 1, t.title))
                    }).collect();

                    let active_ws_name = self.workspace_mgr.get_active_workspace()
                        .map(|ws| ws.name.clone())
                        .unwrap_or_else(|| "Default".to_string());

                    let header = calculate_header_layout(
                        width as f32,
                        &tabs,
                        self.config.window.hide_traffic_lights,
                        self.config.window.tabs_in_titlebar,
                        self.scale_factor,
                        self.renderer.cell_width,
                    );

                    if header.height > 0.0 {
                        // Header bar background strip (#16161e)
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            0,
                            0,
                            width,
                            header.height as usize,
                            0x0016161E,
                        );

                        // Accent stripe 2px under header with active workspace color
                        let stripe_h = (2.0 * self.scale_factor).round().max(1.0) as usize;
                        let stripe_y = (header.height - stripe_h as f32).max(0.0) as usize;
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            0,
                            stripe_y,
                            width,
                            stripe_h,
                            active_accent,
                        );

                        // Draw ☰ Menu Button on the far right
                        let menu_rect = header.menu_button_rect;
                        if menu_rect.width > 0.0 {
                            let (mx, my) = (self.mouse_pos.0 as f32, self.mouse_pos.1 as f32);
                            let is_hovered = menu_rect.contains(mx, my) || self.app_menu_open;
                            let menu_bg = if is_hovered { 0x00283457 } else { 0x001F2335 };
                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                menu_rect.x as usize,
                                menu_rect.y as usize,
                                menu_rect.width as usize,
                                menu_rect.height as usize,
                                menu_bg,
                            );
                            let icon = "☰";
                            let icon_x = menu_rect.x + ((menu_rect.width - self.renderer.cell_width) * 0.5).max(0.0);
                            let icon_y = menu_rect.y + ((menu_rect.height - self.renderer.cell_height) * 0.5).max(0.0);
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                icon_x,
                                icon_y,
                                icon,
                                if is_hovered { 0x007AA2F7 } else { 0x00A9B1D6 },
                            );
                        }

                        // Draw Workspace Indicator badge to the left of the menu button
                        let ws_label = format!("● {}", active_ws_name);
                        let ws_w = ws_label.len() as f32 * self.renderer.cell_width + (16.0 * self.scale_factor);
                        let ws_h = (header.height - 6.0 * self.scale_factor).max(16.0);
                        let ws_x = menu_rect.x - ws_w - (8.0 * self.scale_factor);
                        let ws_y = ((header.height - ws_h) * 0.5).max(0.0);
                        if ws_x > header.add_button_rect.x + (30.0 * self.scale_factor) {
                            let (mx, my) = (self.mouse_pos.0 as f32, self.mouse_pos.1 as f32);
                            let is_hovered = mx >= ws_x && mx <= ws_x + ws_w && my >= ws_y && my <= ws_y + ws_h;
                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                ws_x as usize,
                                ws_y as usize,
                                ws_w as usize,
                                ws_h as usize,
                                if is_hovered { 0x00283457 } else { 0x001F2335 },
                            );
                            draw_outline_rect(
                                &mut buffer,
                                (width, height),
                                (ws_x as usize, ws_y as usize, ws_w as usize, ws_h as usize),
                                1,
                                active_accent,
                            );
                            let text_x = ws_x + (8.0 * self.scale_factor);
                            let text_y = ws_y + ((ws_h - self.renderer.cell_height) * 0.5).max(0.0);
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                text_x,
                                text_y,
                                &ws_label,
                                active_accent,
                            );
                        }

                        // Draw tabs
                        for (idx, (tab_id, rect)) in header.tab_rects.iter().enumerate() {
                            let is_active = tab_id == &active_tab_id;
                            let tab_bg = if is_active { 0x0024283B } else { 0x001A1B26 };
                            let tab_fg = if is_active { 0x00C0CAF5 } else { 0x00787C99 };

                            let raw_tab = raw_tabs.get(idx);
                            let has_custom = raw_tab.and_then(|t| t.color.as_ref()).is_some();
                            let tab_color_u32 = raw_tab
                                .map(|t| t.effective_color_u32(active_accent))
                                .unwrap_or(active_accent);

                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                rect.x as usize,
                                rect.y as usize,
                                rect.width as usize,
                                rect.height as usize,
                                tab_bg,
                            );

                            // Active tab top accent stripe (2px)
                            if is_active {
                                let stripe_h = (2.0 * self.scale_factor).round().max(1.0) as usize;
                                TextRenderer::draw_rect(
                                    &mut buffer,
                                    width,
                                    height,
                                    rect.x as usize,
                                    rect.y as usize,
                                    rect.width as usize,
                                    stripe_h,
                                    tab_color_u32,
                                );
                            }

                            let tab_text_y = rect.y + ((rect.height - self.renderer.cell_height) * 0.5).max(0.0);
                            let mut tab_text_x = rect.x + (6.0 * self.scale_factor);

                            if is_active || has_custom {
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    tab_text_x,
                                    tab_text_y,
                                    "● ",
                                    tab_color_u32,
                                );
                                tab_text_x += 2.0 * self.renderer.cell_width;
                            }

                            let base_title = format!("{}. {}", idx + 1, raw_tab.map(|t| t.title.as_str()).unwrap_or("Tab"));
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                tab_text_x,
                                tab_text_y,
                                &base_title,
                                tab_fg,
                            );

                            if rect.width > (36.0 * self.scale_factor) {
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    rect.x + rect.width - (14.0 * self.scale_factor),
                                    tab_text_y,
                                    "x",
                                    if is_active { 0x00787C99 } else { 0x00414868 },
                                );
                            }

                            // Draw thin separator '|' between adjacent tabs
                            if idx + 1 < header.tab_rects.len() {
                                let sep_x = (rect.x + rect.width).round() as usize;
                                if sep_x < width {
                                    TextRenderer::draw_rect(
                                        &mut buffer,
                                        width,
                                        height,
                                        sep_x,
                                        rect.y as usize + (3.0 * self.scale_factor) as usize,
                                        1,
                                        (rect.height - (6.0 * self.scale_factor)).max(4.0) as usize,
                                        0x003B4261,
                                    );
                                }
                            }
                        }

                        // Draw '+' button
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            header.add_button_rect.x as usize,
                            header.add_button_rect.y as usize,
                            header.add_button_rect.width as usize,
                            header.add_button_rect.height as usize,
                            0x001F2335,
                        );
                        let plus_x = header.add_button_rect.x + ((header.add_button_rect.width - self.renderer.cell_width) * 0.5).max(0.0);
                        let plus_y = header.add_button_rect.y + ((header.add_button_rect.height - self.renderer.cell_height) * 0.5).max(0.0);
                        self.renderer.draw_text(
                            &mut buffer,
                            width,
                            height,
                            plus_x,
                            plus_y,
                            "+",
                            0x007AA2F7,
                        );
                    }

                    // 3. Render Terminal Cells (Two-Pass: Pass 1 Backgrounds, Pass 2 Glyphs & Box Chars)
                    if let Some(active_session) = self.tab_sessions.get_mut(&active_tab_id) {
                        active_session.screen.dirty = false;
                        let pad_x = (self.config.window.padding_x * self.scale_factor).round();
                        let pad_y = (self.config.window.padding_y * self.scale_factor).round();
                        let start_y = header.height + pad_y;
                        let cell_w = self.renderer.cell_width;
                        let cell_h = self.renderer.cell_height;
                        let cols = active_session.screen.size.columns;
                        let lines = active_session.screen.size.lines;
                        let default_fg = 0x00C0CAF5;
                        let default_bg = effective_bg;
                        let selection_range = active_session.screen.selection_range();
                        let selection_bg_u32 = parse_hex_color(&self.config.colors.selection_background, 0x0033467c);
                        let display_offset = active_session.screen.display_offset();

                        // Pass 1: Draw all cell backgrounds across lines
                        for line_idx in 0..lines {
                            let y = start_y + (line_idx as f32) * cell_h;
                            if y + cell_h > height as f32 {
                                break;
                            }
                            let grid_line = Line(line_idx as i32 - display_offset as i32);
                            for col in 0..cols {
                                let point = Point::new(grid_line, Column(col));
                                let is_selected = selection_range.as_ref().map(|sr| sr.contains(point)).unwrap_or(false);

                                if is_selected {
                                    TextRenderer::draw_rect(
                                        &mut buffer,
                                        width,
                                        height,
                                        (pad_x + (col as f32) * cell_w) as usize,
                                        y as usize,
                                        cell_w.ceil() as usize,
                                        cell_h.ceil() as usize,
                                        selection_bg_u32,
                                    );
                                } else {
                                    let (_c, _fg_col, bg_col) = active_session.screen.get_render_cell(col, line_idx);
                                    let bg_u32 = resolve_color(bg_col, default_fg, default_bg);
                                    if bg_u32 != default_bg {
                                        TextRenderer::draw_rect(
                                            &mut buffer,
                                            width,
                                            height,
                                            (pad_x + (col as f32) * cell_w) as usize,
                                            y as usize,
                                            cell_w.ceil() as usize,
                                            cell_h.ceil() as usize,
                                            bg_u32,
                                        );
                                    }
                                }
                            }
                        }

                        // Pass 2: Draw glyphs, geometric box characters, icons, and ligatures
                        for line_idx in 0..lines {
                            let y = start_y + (line_idx as f32) * cell_h;
                            if y + cell_h > height as f32 {
                                break;
                            }

                            let mut col = 0;
                            while col < cols {
                                let (c, fg_col, bg_col) = active_session.screen.get_render_cell(col, line_idx);
                                let fg_u32 = resolve_color(fg_col, default_fg, default_bg);

                                if c == '\0' || c == ' ' {
                                    col += 1;
                                    continue;
                                }

                                let cell_x = pad_x + (col as f32) * cell_w;

                                // A) Geometrically rendered Box-drawing & Block elements
                                if TextRenderer::is_box_or_block(c) {
                                    self.renderer.draw_box_or_block_char(
                                        &mut buffer,
                                        width,
                                        height,
                                        cell_x,
                                        y,
                                        c,
                                        fg_u32,
                                    );
                                    col += 1;
                                    continue;
                                }

                                // B) Nerd Font / PUA icons - individual cell placement
                                if TextRenderer::is_nerd_font_or_pua(c) {
                                    let mut icon_str = String::new();
                                    icon_str.push(c);
                                    self.renderer.draw_text(
                                        &mut buffer,
                                        width,
                                        height,
                                        cell_x,
                                        y,
                                        &icon_str,
                                        fg_u32,
                                    );
                                    col += 1;
                                    continue;
                                }

                                // C) Text & ligatures: group consecutive non-box, non-PUA characters with identical fg/bg
                                let start_col = col;
                                let mut span = String::new();
                                span.push(c);
                                col += 1;

                                while col < cols {
                                    let (nc, nfg, nbg) = active_session.screen.get_render_cell(col, line_idx);
                                    if nc == '\0' || nc == ' ' || TextRenderer::is_box_or_block(nc) || TextRenderer::is_nerd_font_or_pua(nc) {
                                        break;
                                    }
                                    let nfg_u32 = resolve_color(nfg, default_fg, default_bg);
                                    let nbg_u32 = resolve_color(nbg, default_fg, default_bg);
                                    let bg_u32 = resolve_color(bg_col, default_fg, default_bg);
                                    if nfg_u32 != fg_u32 || nbg_u32 != bg_u32 {
                                        let prev_char = span.chars().last().unwrap_or(c);
                                        if is_ligature_punctuation(prev_char) && is_ligature_punctuation(nc) && nbg_u32 == bg_u32 {
                                            span.push(nc);
                                            col += 1;
                                            continue;
                                        }
                                        break;
                                    }
                                    span.push(nc);
                                    col += 1;
                                }

                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    pad_x + (start_col as f32) * cell_w,
                                    y,
                                    &span,
                                    fg_u32,
                                );
                            }
                        }

                        // 4. Render Block Cursor & IME Preedit
                        if let Some((cursor_col, cursor_row)) = active_session.screen.cursor_position() {
                            let cursor_x = pad_x + (cursor_col as f32) * cell_w;
                            let cursor_y = start_y + (cursor_row as f32) * cell_h;

                            if cursor_y + cell_h <= height as f32 && cursor_x + cell_w <= width as f32 {
                                // Anchor native macOS IME candidate window right below cursor.
                                // Note: cursor_x and cursor_y are physical pixels, so pass Position::Physical
                                // to prevent Retina 2x over-scaling.
                                if let Some(ref window) = self.window {
                                    let anchor_x = if let Some((ref text, Some((start, _)))) = self.ime_preedit {
                                        let safe_idx = text.floor_char_boundary(start.min(text.len()));
                                        let prefix = &text[..safe_idx];
                                        cursor_x + UnicodeWidthStr::width(prefix) as f32 * cell_w
                                    } else {
                                        cursor_x
                                    };

                                    window.set_ime_cursor_area(
                                        winit::dpi::Position::Physical(winit::dpi::PhysicalPosition::new(anchor_x.round() as i32, (cursor_y + cell_h).round() as i32)),
                                        winit::dpi::Size::Physical(winit::dpi::PhysicalSize::new(cell_w.round() as u32, cell_h.round() as u32)),
                                    );
                                }

                                if let Some((ref preedit_text, _)) = self.ime_preedit {
                                    // Use unicode display width (takes 2 cells for full-width Japanese / CJK)
                                    let preedit_cols = UnicodeWidthStr::width(preedit_text.as_str());
                                    let preedit_w = (preedit_cols as f32 * cell_w).max(cell_w);

                                    // IME composition background (matches terminal background)
                                    TextRenderer::draw_rect(
                                        &mut buffer,
                                        width,
                                        height,
                                        cursor_x as usize,
                                        cursor_y as usize,
                                        preedit_w.ceil() as usize,
                                        cell_h.ceil() as usize,
                                        default_bg,
                                    );

                                    // IME composition text (matches terminal foreground color)
                                    self.renderer.draw_text(
                                        &mut buffer,
                                        width,
                                        height,
                                        cursor_x,
                                        cursor_y,
                                        preedit_text,
                                        default_fg,
                                    );

                                    // Subtle 1px underline for active preedit
                                    TextRenderer::draw_rect(
                                        &mut buffer,
                                        width,
                                        height,
                                        cursor_x as usize,
                                        (cursor_y + cell_h - 1.0).round() as usize,
                                        preedit_w as usize,
                                        1,
                                        0x007AA2F7,
                                    );

                                    // NOTE: As in WezTerm, DO NOT draw a cursor box while composing (unconfirmed).
                                    // The cursor will appear once confirmed.
                                } else {
                                    let under_char = active_session.screen.get_cell_char(cursor_col, cursor_row);

                                    // Draw cursor block
                                    TextRenderer::draw_rect(
                                        &mut buffer,
                                        width,
                                        height,
                                        cursor_x as usize,
                                        cursor_y as usize,
                                        cell_w as usize,
                                        cell_h as usize,
                                        0x007AA2F7,
                                    );

                                    // Invert character inside cursor box so it's readable
                                    if under_char != ' ' && under_char != '\0' {
                                        let mut char_str = String::new();
                                        char_str.push(under_char);
                                        self.renderer.draw_text(
                                            &mut buffer,
                                            width,
                                            height,
                                            cursor_x,
                                            cursor_y,
                                            &char_str,
                                            0x001A1B26,
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // 4. Dim backdrop if any modal is open
                    if self.workspace_modal.is_open || self.update_modal.is_open || self.tab_color_modal.is_open {
                        // Dim backdrop (50% opacity blend)
                        for pixel in buffer.iter_mut() {
                            let p = *pixel;
                            let r = ((p >> 16) & 0xFF) / 3;
                            let g = ((p >> 8) & 0xFF) / 3;
                            let b = (p & 0xFF) / 3;
                            *pixel = (r << 16) | (g << 8) | b;
                        }
                    }

                    // Render Workspace Management Modal if open
                    if self.workspace_modal.is_open {
                        let scale = self.scale_factor;

                        if let WorkspaceModalMode::Coloring {
                            target_ws_id,
                            is_background,
                            selected_swatch_idx,
                            hex_input,
                        } = &self.workspace_modal.mode
                        {
                            let is_bg = *is_background;
                            let sel_idx = *selected_swatch_idx;
                            let modal_w = (620.0 * scale).min(width as f32 - 32.0);
                            let header_h = (36.0 * scale).round();
                            let tab_bar_h = (36.0 * scale).round();
                            let swatches_h = if is_bg { (36.0 * scale).round() } else { (68.0 * scale).round() };
                            let input_preview_h = (44.0 * scale).round();
                            let footer_h = (42.0 * scale).round();
                            let modal_h = header_h + tab_bar_h + swatches_h + input_preview_h + footer_h + (24.0 * scale);

                            let modal_x = ((width as f32 - modal_w) * 0.5).max(10.0);
                            let modal_y = ((height as f32 - modal_h) * 0.5).max(10.0);

                            // Background (#1A1B26)
                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                modal_x as usize,
                                modal_y as usize,
                                modal_w as usize,
                                modal_h as usize,
                                0x001A1B26,
                            );

                            // Border (#3B4261, 1px)
                            draw_outline_rect(
                                &mut buffer,
                                (width, height),
                                (modal_x as usize, modal_y as usize, modal_w as usize, modal_h as usize),
                                1,
                                0x003B4261,
                            );

                            // Header strip (#24283B)
                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                modal_x as usize,
                                modal_y as usize,
                                modal_w as usize,
                                header_h as usize,
                                0x0024283B,
                            );
                            let head_text_y = modal_y + ((header_h - self.renderer.cell_height) * 0.5).max(0.0);
                            let target_ws = self.workspace_mgr.workspaces.iter().find(|w| &w.id == target_ws_id);
                            let ws_title = target_ws.map(|w| w.name.as_str()).unwrap_or("Workspace");
                            let head_title = format!("Theme & Color: {}", ws_title);
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                modal_x + (16.0 * scale),
                                head_text_y,
                                &head_title,
                                0x00C0CAF5,
                            );
                            let esc_label = "[Esc] Back";
                            let esc_w = esc_label.len() as f32 * self.renderer.cell_width;
                            let esc_x = modal_x + modal_w - esc_w - (20.0 * scale);
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                esc_x,
                                head_text_y,
                                esc_label,
                                0x00787C99,
                            );

                            // Tabs (Accent vs Background)
                            let tabs_y = modal_y + header_h + (8.0 * scale);
                            let tab_w = ((modal_w - (40.0 * scale)) * 0.5).round();
                            let tab_h = (28.0 * scale).round();

                            let cur_accent_u32 = target_ws
                                .map(|w| w.effective_color_u32(self.workspace_modal.selected_index))
                                .unwrap_or(0x007AA2F7);
                            let cur_bg_u32 = target_ws
                                .map(|w| w.effective_background_u32(0x001A1B26))
                                .unwrap_or(0x001A1B26);

                            // Tab 0: Accent Color
                            let tab0_bg = if !is_bg { 0x00283457 } else { 0x001F2335 };
                            let tab0_border = if !is_bg { 0x007AA2F7 } else { 0x003B4261 };
                            TextRenderer::draw_rect(&mut buffer, width, height, (modal_x + (16.0 * scale)) as usize, tabs_y as usize, tab_w as usize, tab_h as usize, tab0_bg);
                            draw_outline_rect(&mut buffer, (width, height), ((modal_x + (16.0 * scale)) as usize, tabs_y as usize, tab_w as usize, tab_h as usize), 1, tab0_border);
                            let tab0_text_y = tabs_y + ((tab_h - self.renderer.cell_height) * 0.5).max(0.0);
                            self.renderer.draw_text(&mut buffer, width, height, modal_x + (24.0 * scale), tab0_text_y, "● [1] Accent Color (20 Tokyo Themes)", if !is_bg { 0x007AA2F7 } else { 0x00787C99 });

                            // Tab 1: Background Color
                            let tab1_x = modal_x + (24.0 * scale) + tab_w;
                            let tab1_bg = if is_bg { 0x00283457 } else { 0x001F2335 };
                            let tab1_border = if is_bg { 0x007AA2F7 } else { 0x003B4261 };
                            TextRenderer::draw_rect(&mut buffer, width, height, tab1_x as usize, tabs_y as usize, tab_w as usize, tab_h as usize, tab1_bg);
                            draw_outline_rect(&mut buffer, (width, height), (tab1_x as usize, tabs_y as usize, tab_w as usize, tab_h as usize), 1, tab1_border);
                            self.renderer.draw_text(&mut buffer, width, height, tab1_x + (8.0 * scale), tab0_text_y, "■ [2] Terminal Background (10 Dark)", if is_bg { 0x007AA2F7 } else { 0x00787C99 });

                            // Swatches
                            let swatches_y = tabs_y + tab_bar_h + (4.0 * scale);
                            let pad_x = modal_x + (16.0 * scale);
                            let avail_w = modal_w - (32.0 * scale);
                            let (cols, count) = if is_bg {
                                (10, crate::workspace::WORKSPACE_BACKGROUND_PALETTE.len())
                            } else {
                                (10, crate::workspace::WORKSPACE_ACCENT_PALETTE.len())
                            };
                            let gap = 6.0 * scale;
                            let swatch_w = (avail_w - ((cols - 1) as f32 * gap)) / cols as f32;
                            let swatch_h = (26.0 * scale).round();

                            for i in 0..count {
                                let col = i % cols;
                                let row = i / cols;
                                let sx = pad_x + col as f32 * (swatch_w + gap);
                                let sy = swatches_y + row as f32 * (swatch_h + gap);
                                let c_val = if is_bg {
                                    crate::workspace::WORKSPACE_BACKGROUND_PALETTE[i].u32_val
                                } else {
                                    crate::workspace::WORKSPACE_ACCENT_PALETTE[i].u32_val
                                };

                                TextRenderer::draw_rect(&mut buffer, width, height, sx as usize, sy as usize, swatch_w as usize, swatch_h as usize, c_val);
                                let is_active_swatch = i == sel_idx;
                                if is_active_swatch {
                                    draw_outline_rect(&mut buffer, (width, height), (sx as usize, sy as usize, swatch_w as usize, swatch_h as usize), 2, 0x00FFFFFF);
                                } else {
                                    draw_outline_rect(&mut buffer, (width, height), (sx as usize, sy as usize, swatch_w as usize, swatch_h as usize), 1, 0x003B4261);
                                }
                            }

                            // Custom HEX Input & Live Preview
                            let input_y = swatches_y + swatches_h + (8.0 * scale);
                            let hex_prompt = "Custom HEX: #";
                            let hex_p_w = hex_prompt.len() as f32 * self.renderer.cell_width;
                            let hex_text_y = input_y + ((28.0 * scale - self.renderer.cell_height) * 0.5).max(0.0);
                            self.renderer.draw_text(&mut buffer, width, height, modal_x + (16.0 * scale), hex_text_y, hex_prompt, 0x00BB9AF7);

                            let input_box_x = modal_x + (16.0 * scale) + hex_p_w;
                            let input_box_w = (110.0 * scale).round();
                            let input_box_h = (28.0 * scale).round();
                            TextRenderer::draw_rect(&mut buffer, width, height, input_box_x as usize, input_y as usize, input_box_w as usize, input_box_h as usize, 0x0016161E);
                            draw_outline_rect(&mut buffer, (width, height), (input_box_x as usize, input_y as usize, input_box_w as usize, input_box_h as usize), 1, 0x007AA2F7);

                            let display_hex = format!("{}_", hex_input);
                            self.renderer.draw_text(&mut buffer, width, height, input_box_x + (6.0 * scale), hex_text_y, &display_hex, 0x00C0CAF5);

                            // Live parsed swatch
                            let parsed_color = if hex_input.len() == 6 && hex_input.chars().all(|c| c.is_ascii_hexdigit()) {
                                crate::renderer::color::parse_hex_color(hex_input, if is_bg { 0x001A1B26 } else { 0x007AA2F7 })
                            } else if is_bg {
                                crate::workspace::WORKSPACE_BACKGROUND_PALETTE[sel_idx % crate::workspace::WORKSPACE_BACKGROUND_PALETTE.len()].u32_val
                            } else {
                                crate::workspace::WORKSPACE_ACCENT_PALETTE[sel_idx % crate::workspace::WORKSPACE_ACCENT_PALETTE.len()].u32_val
                            };
                            let swatch_prev_x = input_box_x + input_box_w + (8.0 * scale);
                            let swatch_prev_w = (28.0 * scale).round();
                            TextRenderer::draw_rect(&mut buffer, width, height, swatch_prev_x as usize, input_y as usize, swatch_prev_w as usize, input_box_h as usize, parsed_color);
                            draw_outline_rect(&mut buffer, (width, height), (swatch_prev_x as usize, input_y as usize, swatch_prev_w as usize, input_box_h as usize), 1, 0x00FFFFFF);

                            // Live Preview Badge
                            let prev_label = "Preview:";
                            let prev_label_w = prev_label.len() as f32 * self.renderer.cell_width;
                            let prev_x = swatch_prev_x + swatch_prev_w + (18.0 * scale);
                            self.renderer.draw_text(&mut buffer, width, height, prev_x, hex_text_y, prev_label, 0x00565F89);

                            let preview_bg = if is_bg { parsed_color } else { cur_bg_u32 };
                            let preview_accent = if !is_bg { parsed_color } else { cur_accent_u32 };

                            let badge_text = format!("● {}", ws_title);
                            let badge_w = badge_text.len() as f32 * self.renderer.cell_width + (16.0 * scale);
                            let badge_x = prev_x + prev_label_w + (8.0 * scale);
                            if badge_x + badge_w < modal_x + modal_w - (16.0 * scale) {
                                TextRenderer::draw_rect(&mut buffer, width, height, badge_x as usize, input_y as usize, badge_w as usize, input_box_h as usize, preview_bg);
                                draw_outline_rect(&mut buffer, (width, height), (badge_x as usize, input_y as usize, badge_w as usize, input_box_h as usize), 1, preview_accent);
                                self.renderer.draw_text(&mut buffer, width, height, badge_x + (8.0 * scale), hex_text_y, &badge_text, preview_accent);
                            }

                            // Footer toolbar in coloring mode
                            let footer_y = modal_y + modal_h - footer_h;
                            TextRenderer::draw_rect(&mut buffer, width, height, modal_x as usize, footer_y as usize, modal_w as usize, footer_h as usize, 0x001F2335);
                            TextRenderer::draw_rect(&mut buffer, width, height, modal_x as usize, footer_y as usize, modal_w as usize, 1, 0x00292E42);

                            let btn_h = (footer_h - 14.0 * scale).max(20.0);
                            let btn_y = footer_y + ((footer_h - btn_h) * 0.5);
                            let btn_w = (110.0 * scale).round();
                            let items = [
                                ("[Enter] Apply", 0x009ECE6A),
                                ("[r] Reset Default", 0x00F7768E),
                                ("[Tab] Switch Tab", 0x00BB9AF7),
                                ("[Esc] Back", 0x007AA2F7),
                            ];
                            let (mx, my) = (self.mouse_pos.0 as f32, self.mouse_pos.1 as f32);
                            for (i, (label, col)) in items.iter().enumerate() {
                                let bx = modal_x + (16.0 * scale) + (i as f32 * (btn_w + (8.0 * scale)));
                                if bx + btn_w <= modal_x + modal_w - (8.0 * scale) {
                                    let b_rect = crate::window::Rect { x: bx, y: btn_y, width: btn_w, height: btn_h };
                                    let is_h = b_rect.contains(mx, my);
                                    TextRenderer::draw_rect(&mut buffer, width, height, bx as usize, btn_y as usize, btn_w as usize, btn_h as usize, if is_h { 0x00283457 } else { 0x0024283B });
                                    draw_outline_rect(&mut buffer, (width, height), (bx as usize, btn_y as usize, btn_w as usize, btn_h as usize), 1, if is_h { *col } else { 0x003B4261 });
                                    let tw = label.chars().count() as f32 * self.renderer.cell_width;
                                    let tx = bx + ((btn_w - tw) * 0.5).max(0.0);
                                    let ty = btn_y + ((btn_h - self.renderer.cell_height) * 0.5).max(0.0);
                                    self.renderer.draw_text(&mut buffer, width, height, tx, ty, label, *col);
                                }
                            }
                        } else {
                            let modal_w = (580.0 * scale).min(width as f32 - 32.0);
                            let header_h = (36.0 * scale).round();
                            let row_h = (32.0 * scale).round();
                            let ws_count = self.workspace_mgr.workspaces.len();
                            let list_h = (ws_count as f32 * row_h).max(32.0 * scale);
                            let is_editing = matches!(self.workspace_modal.mode, WorkspaceModalMode::Renaming { .. } | WorkspaceModalMode::Creating { .. });
                            let edit_h = if is_editing { (36.0 * scale).round() } else { 0.0 };
                            let footer_h = (42.0 * scale).round();
                            let modal_h = header_h + list_h + edit_h + footer_h + (16.0 * scale);

                            let modal_x = ((width as f32 - modal_w) * 0.5).max(10.0);
                            let modal_y = ((height as f32 - modal_h) * 0.5).max(10.0);

                            // Modal background (#1A1B26)
                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                modal_x as usize,
                                modal_y as usize,
                                modal_w as usize,
                                modal_h as usize,
                                0x001A1B26,
                            );

                            // Modal border (#3B4261, 1px)
                            draw_outline_rect(
                                &mut buffer,
                                (width, height),
                                (modal_x as usize, modal_y as usize, modal_w as usize, modal_h as usize),
                                1,
                                0x003B4261,
                            );

                            // Header strip (#24283B)
                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                modal_x as usize,
                                modal_y as usize,
                                modal_w as usize,
                                header_h as usize,
                                0x0024283B,
                            );
                            let head_text_y = modal_y + ((header_h - self.renderer.cell_height) * 0.5).max(0.0);
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                modal_x + (16.0 * scale),
                                head_text_y,
                                "Workspaces",
                                0x00C0CAF5,
                            );
                            let esc_label = "[Esc] Close";
                            let esc_w = esc_label.len() as f32 * self.renderer.cell_width;
                            let esc_x = modal_x + modal_w - esc_w - (20.0 * scale);
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                esc_x,
                                head_text_y,
                                esc_label,
                                0x00787C99,
                            );

                            // Workspace list rows
                            let list_top = modal_y + header_h + (8.0 * scale);
                            let my_pid = std::process::id();
                            let other_instances = crate::workspace::get_all_active_instances();
                            for (idx, ws) in self.workspace_mgr.workspaces.iter().enumerate() {
                                let row_y = list_top + (idx as f32 * row_h);
                                let is_selected = idx == self.workspace_modal.selected_index;
                                let is_current = ws.id == self.workspace_mgr.active_workspace_id;
                                let row_bg = if is_selected { 0x00283457 } else { 0x001A1B26 };

                                TextRenderer::draw_rect(
                                    &mut buffer,
                                    width,
                                    height,
                                    (modal_x + (8.0 * scale)) as usize,
                                    row_y as usize,
                                    (modal_w - (16.0 * scale)) as usize,
                                    row_h as usize,
                                    row_bg,
                                );

                                let text_y = row_y + ((row_h - self.renderer.cell_height) * 0.5).max(0.0);
                                let num_str = format!("{}. ", idx + 1);
                                self.renderer.draw_text(&mut buffer, width, height, modal_x + (16.0 * scale), text_y, &num_str, 0x00565F89);

                                // Draw workspace color bullet dot ●
                                let ws_color = ws.effective_color_u32(idx);
                                let dot_x = modal_x + (34.0 * scale);
                                self.renderer.draw_text(&mut buffer, width, height, dot_x, text_y, "●", ws_color);

                                let name_x = modal_x + (48.0 * scale);
                                let name_fg = if is_selected { 0x007AA2F7 } else { 0x00C0CAF5 };
                                self.renderer.draw_text(&mut buffer, width, height, name_x, text_y, &ws.name, name_fg);

                                let mut tab_info = format!("({} tab{})", ws.tabs.len(), if ws.tabs.len() > 1 { "s" } else { "" });
                                if let Some(ref bg) = ws.background {
                                    tab_info.push_str(&format!(" [bg: {}]", bg));
                                }
                                let info_x = name_x + (ws.name.len() as f32 * self.renderer.cell_width) + (10.0 * scale);
                                self.renderer.draw_text(&mut buffer, width, height, info_x, text_y, &tab_info, 0x00565F89);

                                if is_current {
                                    let cur_x = modal_x + modal_w - (80.0 * scale);
                                    self.renderer.draw_text(&mut buffer, width, height, cur_x, text_y, "● Active", 0x009ECE6A);
                                } else if other_instances.iter().any(|inst| inst.pid != my_pid && (inst.workspace_id == ws.id || inst.workspace_name == ws.name)) {
                                    let cur_x = modal_x + modal_w - (105.0 * scale);
                                    self.renderer.draw_text(&mut buffer, width, height, cur_x, text_y, "● In Window", 0x007DCFFF);
                                }
                            }

                            // Inline text editing (Renaming / Creating)
                            if is_editing {
                                let edit_y = list_top + list_h + (4.0 * scale);
                                let (prompt, input_str) = match &self.workspace_modal.mode {
                                    WorkspaceModalMode::Renaming { input } => ("Rename to: ", input.as_str()),
                                    WorkspaceModalMode::Creating { input } => ("New workspace: ", input.as_str()),
                                    _ => ("", ""),
                                };

                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    modal_x + (16.0 * scale),
                                    edit_y,
                                    prompt,
                                    0x00BB9AF7,
                                );

                                let prompt_w = prompt.len() as f32 * self.renderer.cell_width;
                                let input_x = modal_x + (16.0 * scale) + prompt_w;
                                let input_w = ((input_str.len() + 3) as f32 * self.renderer.cell_width).max(120.0 * scale);
                                let input_h = (self.renderer.cell_height + 4.0 * scale).round();

                                TextRenderer::draw_rect(
                                    &mut buffer,
                                    width,
                                    height,
                                    input_x as usize,
                                    edit_y as usize,
                                    input_w as usize,
                                    input_h as usize,
                                    0x0016161E,
                                );
                                draw_outline_rect(
                                    &mut buffer,
                                    (width, height),
                                    (input_x as usize, edit_y as usize, input_w as usize, input_h as usize),
                                    1,
                                    0x007AA2F7,
                                );

                                let input_text = format!("{}_", input_str);
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    input_x + (4.0 * scale),
                                    edit_y + (2.0 * scale),
                                    &input_text,
                                    0x007AA2F7,
                                );
                            }

                            // Footer toolbar
                            let footer_y = modal_y + modal_h - footer_h;
                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                modal_x as usize,
                                footer_y as usize,
                                modal_w as usize,
                                footer_h as usize,
                                0x001F2335,
                            );
                            // Subtle top border for footer
                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                modal_x as usize,
                                footer_y as usize,
                                modal_w as usize,
                                1,
                                0x00292E42,
                            );

                            let modal_rect = crate::window::Rect {
                                x: modal_x,
                                y: modal_y,
                                width: modal_w,
                                height: modal_h,
                            };
                            let footer_buttons = crate::window::calculate_modal_buttons(
                                modal_rect,
                                footer_h,
                                scale,
                                self.renderer.cell_width,
                            );

                            let (mx, my) = (self.mouse_pos.0 as f32, self.mouse_pos.1 as f32);
                            for btn in &footer_buttons {
                                let is_hovered = btn.rect.contains(mx, my);
                                let bg_color = if is_hovered { 0x00283457 } else { 0x0024283B };
                                let border_color = if is_hovered { btn.color } else { 0x003B4261 };

                                TextRenderer::draw_rect(
                                    &mut buffer,
                                    width,
                                    height,
                                    btn.rect.x as usize,
                                    btn.rect.y as usize,
                                    btn.rect.width as usize,
                                    btn.rect.height as usize,
                                    bg_color,
                                );
                                draw_outline_rect(
                                    &mut buffer,
                                    (width, height),
                                    (btn.rect.x as usize, btn.rect.y as usize, btn.rect.width as usize, btn.rect.height as usize),
                                    1,
                                    border_color,
                                );
                                let text_w = btn.label.chars().count() as f32 * self.renderer.cell_width;
                                let text_x = btn.rect.x + ((btn.rect.width - text_w) * 0.5).max(0.0);
                                let text_y = btn.rect.y + ((btn.rect.height - self.renderer.cell_height) * 0.5).max(0.0);
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    text_x,
                                    text_y,
                                    btn.label,
                                    btn.color,
                                );
                            }
                        }
                    }

                    // 5. Render Software Update Modal if open
                    if self.update_modal.is_open {
                        let scale = self.scale_factor;
                        let modal_w = (640.0 * scale).min(width as f32 - 32.0);
                        let header_h = (36.0 * scale).round();
                        let footer_h = (42.0 * scale).round();
                        let content_h = (200.0 * scale).round();
                        let modal_h = header_h + content_h + footer_h;

                        let modal_x = ((width as f32 - modal_w) * 0.5).max(10.0);
                        let modal_y = ((height as f32 - modal_h) * 0.5).max(10.0);

                        // Modal background (#1A1B26)
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            modal_x as usize,
                            modal_y as usize,
                            modal_w as usize,
                            modal_h as usize,
                            0x001A1B26,
                        );

                        // Modal border (#3B4261, 1px)
                        draw_outline_rect(
                            &mut buffer,
                            (width, height),
                            (modal_x as usize, modal_y as usize, modal_w as usize, modal_h as usize),
                            1,
                            0x003B4261,
                        );

                        // Header strip (#24283B)
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            modal_x as usize,
                            modal_y as usize,
                            modal_w as usize,
                            header_h as usize,
                            0x0024283B,
                        );
                        let head_text_y = modal_y + ((header_h - self.renderer.cell_height) * 0.5).max(0.0);
                        self.renderer.draw_text(
                            &mut buffer,
                            width,
                            height,
                            modal_x + (16.0 * scale),
                            head_text_y,
                            "Check for Updates",
                            0x00C0CAF5,
                        );
                        let esc_label = "[Esc] Close";
                        let esc_w = esc_label.len() as f32 * self.renderer.cell_width;
                        let esc_x = modal_x + modal_w - esc_w - (20.0 * scale);
                        self.renderer.draw_text(
                            &mut buffer,
                            width,
                            height,
                            esc_x,
                            head_text_y,
                            esc_label,
                            0x00787C99,
                        );

                        // Content Body
                        let body_top = modal_y + header_h + (16.0 * scale);
                        let body_x = modal_x + (20.0 * scale);
                        let line_h = self.renderer.cell_height + (6.0 * scale);

                        match &self.update_modal.state {
                            crate::update::UpdateState::Idle | crate::update::UpdateState::Checking => {
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top,
                                    "Checking for updates...",
                                    0x007AA2F7,
                                );
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + line_h,
                                    &format!("Connecting to GitHub ({})", crate::update::GITHUB_REPO),
                                    0x00565F89,
                                );
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + (line_h * 2.0),
                                    &format!("Current version: v{}", crate::update::CURRENT_VERSION),
                                    0x00C0CAF5,
                                );
                            }
                            crate::update::UpdateState::UpToDate { current_version } => {
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top,
                                    "✔ CelerTerm is up to date!",
                                    0x009ECE6A,
                                );
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + line_h,
                                    &format!("You are running the latest version: v{}", current_version),
                                    0x00C0CAF5,
                                );
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + (line_h * 2.0),
                                    "No new updates are available at this time.",
                                    0x00787C99,
                                );
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + (line_h * 3.5),
                                    &format!("Repository: https://github.com/{}", crate::update::GITHUB_REPO),
                                    0x00565F89,
                                );
                            }
                            crate::update::UpdateState::Available {
                                current_version,
                                latest_version,
                                notes,
                                published_at,
                                ..
                            } => {
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top,
                                    "🚀 New version available!",
                                    0x007AA2F7,
                                );
                                let ver_info = format!("Current: v{}  ➜  Latest: {}", current_version, latest_version);
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + line_h,
                                    &ver_info,
                                    0x009ECE6A,
                                );
                                if let Some(date) = published_at {
                                    let date_short = date.split('T').next().unwrap_or(date);
                                    let date_str = format!("Released: {}", date_short);
                                    let date_x = body_x + (ver_info.len() as f32 * self.renderer.cell_width) + (16.0 * scale);
                                    if date_x + (date_str.len() as f32 * self.renderer.cell_width) <= modal_x + modal_w - (16.0 * scale) {
                                        self.renderer.draw_text(
                                            &mut buffer,
                                            width,
                                            height,
                                            date_x,
                                            body_top + line_h,
                                            &date_str,
                                            0x00565F89,
                                        );
                                    }
                                }

                                // Release notes frame
                                let notes_box_y = body_top + (line_h * 2.2);
                                let notes_box_w = modal_w - (40.0 * scale);
                                let notes_box_h = content_h - (line_h * 2.5) - (8.0 * scale);

                                TextRenderer::draw_rect(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x as usize,
                                    notes_box_y as usize,
                                    notes_box_w as usize,
                                    notes_box_h as usize,
                                    0x0016161E,
                                );
                                draw_outline_rect(
                                    &mut buffer,
                                    (width, height),
                                    (body_x as usize, notes_box_y as usize, notes_box_w as usize, notes_box_h as usize),
                                    1,
                                    0x00292E42,
                                );

                                let visible_lines = (notes_box_h / self.renderer.cell_height).floor() as usize;
                                let lines: Vec<&str> = notes.lines().collect();
                                let max_offset = lines.len().saturating_sub(visible_lines);
                                let offset = self.update_modal.scroll_offset.min(max_offset);

                                for (i, line) in lines.iter().skip(offset).take(visible_lines).enumerate() {
                                    let ly = notes_box_y + (4.0 * scale) + (i as f32 * self.renderer.cell_height);
                                    let line_w = line.len() as f32 * self.renderer.cell_width;
                                    let avail_text_w = notes_box_w - (12.0 * scale);
                                    let display_str = if line_w > avail_text_w {
                                        let max_chars = (avail_text_w / self.renderer.cell_width).floor() as usize;
                                        &line[..line.char_indices().map(|(i, _)| i).nth(max_chars).unwrap_or(line.len())]
                                    } else {
                                        line
                                    };
                                    self.renderer.draw_text(
                                        &mut buffer,
                                        width,
                                        height,
                                        body_x + (6.0 * scale),
                                        ly,
                                        display_str,
                                        0x00A9B1D6,
                                    );
                                }

                                if lines.len() > visible_lines {
                                    let scroll_hint = "▲▼ Scroll";
                                    let sh_w = scroll_hint.len() as f32 * self.renderer.cell_width;
                                    self.renderer.draw_text(
                                        &mut buffer,
                                        width,
                                        height,
                                        body_x + notes_box_w - sh_w - (6.0 * scale),
                                        notes_box_y + notes_box_h - self.renderer.cell_height - (2.0 * scale),
                                        scroll_hint,
                                        0x00565F89,
                                    );
                                }
                            }
                            crate::update::UpdateState::Downloading { latest_version, status_text } => {
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top,
                                    &format!("⏳ Downloading CelerTerm {}...", latest_version),
                                    0x007AA2F7,
                                );
                                let avail_body_w = modal_w - (40.0 * scale);
                                let max_status_chars = (avail_body_w / self.renderer.cell_width).floor().max(10.0) as usize;
                                let status_display = if status_text.chars().count() > max_status_chars {
                                    let cutoff = status_text.char_indices().map(|(i, _)| i).nth(max_status_chars.saturating_sub(3)).unwrap_or(status_text.len());
                                    format!("{}...", &status_text[..cutoff])
                                } else {
                                    status_text.clone()
                                };
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + line_h,
                                    &status_display,
                                    0x00C0CAF5,
                                );
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + (line_h * 2.0),
                                    "Please wait while the update archive is downloaded and verified.",
                                    0x00787C99,
                                );

                                // Progress bar strip
                                let prog_y = body_top + (line_h * 3.5);
                                let prog_w = modal_w - (40.0 * scale);
                                let prog_h = (8.0 * scale).round();
                                TextRenderer::draw_rect(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x as usize,
                                    prog_y as usize,
                                    prog_w as usize,
                                    prog_h as usize,
                                    0x0016161E,
                                );
                                TextRenderer::draw_rect(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x as usize,
                                    prog_y as usize,
                                    (prog_w * 0.75) as usize,
                                    prog_h as usize,
                                    0x007AA2F7,
                                );
                            }
                            crate::update::UpdateState::ReadyToRestart { latest_version, target_path, .. } => {
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top,
                                    "🎉 Update Ready to Install!",
                                    0x009ECE6A,
                                );
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + line_h,
                                    &format!("CelerTerm {} is ready to replace your current version.", latest_version),
                                    0x00C0CAF5,
                                );
                                let target_display = target_path.display().to_string();
                                let target_line = format!("Install Target: {}", target_display);
                                let avail_body_w = modal_w - (40.0 * scale);
                                let max_t_chars = (avail_body_w / self.renderer.cell_width).floor().max(10.0) as usize;
                                let t_line = if target_line.chars().count() > max_t_chars {
                                    let cutoff = target_line.char_indices().map(|(i, _)| i).nth(max_t_chars.saturating_sub(3)).unwrap_or(target_line.len());
                                    format!("{}...", &target_line[..cutoff])
                                } else {
                                    target_line
                                };
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + (line_h * 2.2),
                                    &t_line,
                                    0x00787C99,
                                );
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + (line_h * 3.5),
                                    "Click [Restart & Update] to apply the update and relaunch.",
                                    0x009ECE6A,
                                );
                            }
                            crate::update::UpdateState::Error(err) => {
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top,
                                    "✖ Update Check Failed",
                                    0x00F7768E,
                                );
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + line_h,
                                    "Could not connect to GitHub:",
                                    0x00C0CAF5,
                                );
                                let avail_body_w = modal_w - (40.0 * scale);
                                let max_err_chars = (avail_body_w / self.renderer.cell_width).floor().max(10.0) as usize;
                                let err_line = if err.chars().count() > max_err_chars {
                                    let cutoff = err.char_indices().map(|(i, _)| i).nth(max_err_chars.saturating_sub(3)).unwrap_or(err.len());
                                    format!("{}...", &err[..cutoff])
                                } else {
                                    err.clone()
                                };
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + (line_h * 2.0),
                                    &err_line,
                                    0x00E0AF68,
                                );
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    body_x,
                                    body_top + (line_h * 3.5),
                                    "Please check your internet connection or try again later.",
                                    0x00565F89,
                                );
                            }
                        }

                        // Footer toolbar
                        let footer_y = modal_y + modal_h - footer_h;
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            modal_x as usize,
                            footer_y as usize,
                            modal_w as usize,
                            footer_h as usize,
                            0x001F2335,
                        );
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            modal_x as usize,
                            footer_y as usize,
                            modal_w as usize,
                            1,
                            0x00292E42,
                        );

                        let modal_rect = crate::window::Rect {
                            x: modal_x,
                            y: modal_y,
                            width: modal_w,
                            height: modal_h,
                        };
                        let (primary_label, primary_color) = match &self.update_modal.state {
                            crate::update::UpdateState::Available { download_url, .. } => {
                                if download_url.is_some() {
                                    ("[Enter] Update Now", 0x007AA2F7)
                                } else {
                                    ("[Enter] Download", 0x007AA2F7)
                                }
                            }
                            crate::update::UpdateState::Downloading { .. } => {
                                ("[...] Downloading", 0x00565F89)
                            }
                            crate::update::UpdateState::ReadyToRestart { .. } => {
                                ("[Enter] Restart & Update", 0x009ECE6A)
                            }
                            crate::update::UpdateState::Error(_) => {
                                ("[Enter] Try Again", 0x00F7768E)
                            }
                            _ => {
                                ("[Enter] Check Again", 0x007AA2F7)
                            }
                        };
                        let footer_buttons = crate::window::calculate_update_modal_buttons_with_label(
                            modal_rect,
                            footer_h,
                            scale,
                            self.renderer.cell_width,
                            primary_label,
                            primary_color,
                        );

                        let (mx, my) = (self.mouse_pos.0 as f32, self.mouse_pos.1 as f32);
                        for btn in &footer_buttons {
                            let is_hovered = btn.rect.contains(mx, my);
                            let bg_color = if is_hovered { 0x00283457 } else { 0x0024283B };
                            let border_color = if is_hovered { btn.color } else { 0x003B4261 };

                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                btn.rect.x as usize,
                                btn.rect.y as usize,
                                btn.rect.width as usize,
                                btn.rect.height as usize,
                                bg_color,
                            );
                            draw_outline_rect(
                                &mut buffer,
                                (width, height),
                                (btn.rect.x as usize, btn.rect.y as usize, btn.rect.width as usize, btn.rect.height as usize),
                                1,
                                border_color,
                            );
                            let text_w = btn.label.chars().count() as f32 * self.renderer.cell_width;
                            let text_x = btn.rect.x + ((btn.rect.width - text_w) * 0.5).max(0.0);
                            let text_y = btn.rect.y + ((btn.rect.height - self.renderer.cell_height) * 0.5).max(0.0);
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                text_x,
                                text_y,
                                btn.label,
                                btn.color,
                            );
                        }
                    }

                    // 6. Render Dropdown App Menu if open
                    if self.app_menu_open {
                        let scale = self.scale_factor;
                        let menu_w = (220.0 * scale).round();
                        let item_h = (28.0 * scale).round();
                        let menu_pad = (6.0 * scale).round();
                        let menu_btn = header.menu_button_rect;
                        let menu_x = (menu_btn.x + menu_btn.width - menu_w).max(8.0 * scale);
                        let menu_y = header.height + (2.0 * scale);
                        let menu_h = (5.0 * item_h) + (menu_pad * 2.0);

                        // Menu background (#1F2335)
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            menu_x as usize,
                            menu_y as usize,
                            menu_w as usize,
                            menu_h as usize,
                            0x001F2335,
                        );
                        // Menu border (#3B4261)
                        draw_outline_rect(
                            &mut buffer,
                            (width, height),
                            (menu_x as usize, menu_y as usize, menu_w as usize, menu_h as usize),
                            1,
                            0x003B4261,
                        );

                        let (sc_u, sc_o, sc_r) = if cfg!(target_os = "macos") {
                            ("⌘⇧U", "⌘⇧O", "⌘⇧R")
                        } else {
                            ("Ctrl+Shift+U", "Ctrl+Shift+O", "Ctrl+Shift+R")
                        };

                        let menu_items: [(&str, &str); 5] = [
                            ("Check for Updates...", sc_u),
                            ("Workspaces", sc_o),
                            ("Reload Config", sc_r),
                            ("GitHub Repository", ""),
                            ("About CelerTerm", ""),
                        ];

                        let (mx, my) = (self.mouse_pos.0 as f32, self.mouse_pos.1 as f32);
                        for (i, (label, shortcut)) in menu_items.iter().enumerate() {
                            let item_y = menu_y + menu_pad + (i as f32 * item_h);
                            let is_hovered = mx >= menu_x && mx <= menu_x + menu_w && my >= item_y && my < item_y + item_h;
                            if is_hovered {
                                TextRenderer::draw_rect(
                                    &mut buffer,
                                    width,
                                    height,
                                    (menu_x + 4.0 * scale) as usize,
                                    item_y as usize,
                                    (menu_w - 8.0 * scale) as usize,
                                    item_h as usize,
                                    0x00283457,
                                );
                            }
                            let text_y = item_y + ((item_h - self.renderer.cell_height) * 0.5).max(0.0);
                            let text_color = if is_hovered { 0x007AA2F7 } else { 0x00C0CAF5 };
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                menu_x + (12.0 * scale),
                                text_y,
                                label,
                                text_color,
                            );
                            if !shortcut.is_empty() {
                                let sc_w = shortcut.chars().count() as f32 * self.renderer.cell_width;
                                let sc_x = menu_x + menu_w - sc_w - (12.0 * scale);
                                self.renderer.draw_text(
                                    &mut buffer,
                                    width,
                                    height,
                                    sc_x,
                                    text_y,
                                    shortcut,
                                    0x00565F89,
                                );
                            }
                        }
                    }

                    // 7. Render Tab Color Customization Popover if open
                    if self.tab_color_modal.is_open {
                        let scale = self.scale_factor;
                        let modal_w = (460.0 * scale).min(width as f32 - 32.0);
                        let header_h = (34.0 * scale).round();
                        let swatches_h = (68.0 * scale).round();
                        let input_preview_h = (40.0 * scale).round();
                        let footer_h = (38.0 * scale).round();
                        let modal_h = header_h + swatches_h + input_preview_h + footer_h + (16.0 * scale);

                        let modal_x = if self.tab_color_modal.anchor_x > 0.0 || self.tab_color_modal.anchor_y > 0.0 {
                            self.tab_color_modal.anchor_x.min(width as f32 - modal_w - 16.0).max(16.0)
                        } else {
                            ((width as f32 - modal_w) * 0.5).max(10.0)
                        };
                        let modal_y = if self.tab_color_modal.anchor_x > 0.0 || self.tab_color_modal.anchor_y > 0.0 {
                            self.tab_color_modal.anchor_y.min(height as f32 - modal_h - 16.0).max(16.0)
                        } else {
                            (header.height + (8.0 * scale)).min(height as f32 - modal_h - 16.0).max(10.0)
                        };

                        // Modal background (#1A1B26)
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            modal_x as usize,
                            modal_y as usize,
                            modal_w as usize,
                            modal_h as usize,
                            0x001A1B26,
                        );

                        // Modal border (#3B4261, 1px)
                        draw_outline_rect(
                            &mut buffer,
                            (width, height),
                            (modal_x as usize, modal_y as usize, modal_w as usize, modal_h as usize),
                            1,
                            0x003B4261,
                        );

                        // Header bar (#24283B)
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            modal_x as usize,
                            modal_y as usize,
                            modal_w as usize,
                            header_h as usize,
                            0x0024283B,
                        );

                        let head_text_y = modal_y + ((header_h - self.renderer.cell_height) * 0.5).max(0.0);
                        let title_text = format!("Tab Color: {}", self.tab_color_modal.target_tab_title);
                        self.renderer.draw_text(
                            &mut buffer,
                            width,
                            height,
                            modal_x + (16.0 * scale),
                            head_text_y,
                            &title_text,
                            0x00C0CAF5,
                        );

                        let esc_label = "[Esc] Close";
                        let esc_w = esc_label.len() as f32 * self.renderer.cell_width;
                        let esc_x = modal_x + modal_w - esc_w - (16.0 * scale);
                        self.renderer.draw_text(
                            &mut buffer,
                            width,
                            height,
                            esc_x,
                            head_text_y,
                            esc_label,
                            0x00787C99,
                        );

                        // Swatches: 20 colors in 2 rows of 10
                        let swatches_y = modal_y + header_h + (10.0 * scale);
                        let box_w = (32.0 * scale).round();
                        let box_h = (24.0 * scale).round();
                        let gap = (8.0 * scale).round();
                        let swatches_total_w = 10.0 * box_w + 9.0 * gap;
                        let swatches_x = modal_x + ((modal_w - swatches_total_w) * 0.5).max(0.0);

                        for (i, p) in crate::workspace::WORKSPACE_ACCENT_PALETTE.iter().enumerate() {
                            let col = i % 10;
                            let row = i / 10;
                            let sx = swatches_x + col as f32 * (box_w + gap);
                            let sy = swatches_y + row as f32 * (box_h + gap);

                            TextRenderer::draw_rect(
                                &mut buffer,
                                width,
                                height,
                                sx as usize,
                                sy as usize,
                                box_w as usize,
                                box_h as usize,
                                p.u32_val,
                            );

                            if i == self.tab_color_modal.selected_swatch_idx {
                                draw_outline_rect(
                                    &mut buffer,
                                    (width, height),
                                    (sx as usize, sy as usize, box_w as usize, box_h as usize),
                                    2,
                                    0x00FFFFFF,
                                );
                            } else {
                                draw_outline_rect(
                                    &mut buffer,
                                    (width, height),
                                    (sx as usize, sy as usize, box_w as usize, box_h as usize),
                                    1,
                                    0x0016161E,
                                );
                            }
                        }

                        // HEX Input & Preview Row
                        let input_y = swatches_y + (2.0 * box_h) + gap + (12.0 * scale);
                        let preview_w = (36.0 * scale).round();
                        let preview_h = (28.0 * scale).round();
                        let preview_x = modal_x + (16.0 * scale);

                        let chosen_preview_u32 = if self.tab_color_modal.hex_input.len() == 6 && self.tab_color_modal.hex_input.chars().all(|c| c.is_ascii_hexdigit()) {
                            crate::renderer::color::parse_hex_color(&format!("#{}", self.tab_color_modal.hex_input), 0x007AA2F7)
                        } else {
                            crate::workspace::WORKSPACE_ACCENT_PALETTE[self.tab_color_modal.selected_swatch_idx % crate::workspace::WORKSPACE_ACCENT_PALETTE.len()].u32_val
                        };

                        // Color preview swatch box
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            preview_x as usize,
                            input_y as usize,
                            preview_w as usize,
                            preview_h as usize,
                            chosen_preview_u32,
                        );
                        draw_outline_rect(
                            &mut buffer,
                            (width, height),
                            (preview_x as usize, input_y as usize, preview_w as usize, preview_h as usize),
                            1,
                            0x00FFFFFF,
                        );

                        // Input text display
                        let input_box_x = preview_x + preview_w + (12.0 * scale);
                        let input_box_w = (140.0 * scale).round();
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            input_box_x as usize,
                            input_y as usize,
                            input_box_w as usize,
                            preview_h as usize,
                            0x001F2335,
                        );
                        draw_outline_rect(
                            &mut buffer,
                            (width, height),
                            (input_box_x as usize, input_y as usize, input_box_w as usize, preview_h as usize),
                            1,
                            0x007AA2F7,
                        );

                        let input_text = format!("#{}█", self.tab_color_modal.hex_input);
                        let itext_y = input_y + ((preview_h - self.renderer.cell_height) * 0.5).max(0.0);
                        self.renderer.draw_text(
                            &mut buffer,
                            width,
                            height,
                            input_box_x + (8.0 * scale),
                            itext_y,
                            &input_text,
                            0x00C0CAF5,
                        );

                        // Reset button [r] Default
                        let reset_btn_w = (150.0 * scale).round();
                        let reset_btn_x = modal_x + modal_w - reset_btn_w - (16.0 * scale);
                        let (mx, my) = (self.mouse_pos.0 as f32, self.mouse_pos.1 as f32);
                        let reset_hovered = mx >= reset_btn_x && mx <= reset_btn_x + reset_btn_w && my >= input_y && my <= input_y + preview_h;
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            reset_btn_x as usize,
                            input_y as usize,
                            reset_btn_w as usize,
                            preview_h as usize,
                            if reset_hovered { 0x00283457 } else { 0x0024283B },
                        );
                        draw_outline_rect(
                            &mut buffer,
                            (width, height),
                            (reset_btn_x as usize, input_y as usize, reset_btn_w as usize, preview_h as usize),
                            1,
                            0x003B4261,
                        );
                        self.renderer.draw_text(
                            &mut buffer,
                            width,
                            height,
                            reset_btn_x + (8.0 * scale),
                            itext_y,
                            "↺ [r] Reset to Default",
                            0x00A9B1D6,
                        );

                        // Footer: Apply [Enter] button & Cancel [Esc]
                        let footer_y = modal_y + modal_h - footer_h;
                        let apply_btn_w = (120.0 * scale).round();
                        let apply_btn_h = (28.0 * scale).round();
                        let apply_hovered = mx >= modal_x + (16.0 * scale) && mx <= modal_x + (16.0 * scale) + apply_btn_w && my >= footer_y + (4.0 * scale) && my <= footer_y + (4.0 * scale) + apply_btn_h;
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            (modal_x + (16.0 * scale)) as usize,
                            (footer_y + (4.0 * scale)) as usize,
                            apply_btn_w as usize,
                            apply_btn_h as usize,
                            if apply_hovered { 0x003D59A1 } else { 0x00283457 },
                        );
                        draw_outline_rect(
                            &mut buffer,
                            (width, height),
                            ((modal_x + (16.0 * scale)) as usize, (footer_y + (4.0 * scale)) as usize, apply_btn_w as usize, apply_btn_h as usize),
                            1,
                            chosen_preview_u32,
                        );
                        let apply_text_y = footer_y + (4.0 * scale) + ((apply_btn_h - self.renderer.cell_height) * 0.5).max(0.0);
                        self.renderer.draw_text(
                            &mut buffer,
                            width,
                            height,
                            modal_x + (24.0 * scale),
                            apply_text_y,
                            "✓ [Enter] Apply",
                            0x00C0CAF5,
                        );

                        let cancel_btn_w = (80.0 * scale).round();
                        let cancel_btn_x = modal_x + (28.0 * scale) + apply_btn_w;
                        let cancel_hovered = mx >= cancel_btn_x && mx <= cancel_btn_x + cancel_btn_w && my >= footer_y + (4.0 * scale) && my <= footer_y + (4.0 * scale) + apply_btn_h;
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            cancel_btn_x as usize,
                            (footer_y + (4.0 * scale)) as usize,
                            cancel_btn_w as usize,
                            apply_btn_h as usize,
                            if cancel_hovered { 0x00283457 } else { 0x001F2335 },
                        );
                        draw_outline_rect(
                            &mut buffer,
                            (width, height),
                            (cancel_btn_x as usize, (footer_y + (4.0 * scale)) as usize, cancel_btn_w as usize, apply_btn_h as usize),
                            1,
                            0x003B4261,
                        );
                        self.renderer.draw_text(
                            &mut buffer,
                            width,
                            height,
                            cancel_btn_x + (12.0 * scale),
                            apply_text_y,
                            "Cancel",
                            0x00787C99,
                        );
                    }

                    let _ = buffer.present();

                    #[cfg(target_os = "macos")]
                    unsafe {
                        use objc2::class;
                        use objc2::msg_send;
                        let _: () = msg_send![class!(CATransaction), flush];
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.workspace_modal.is_open {
            if self.reload_workspaces_from_disk() {
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
        }
        if let Some(active_id) = self.active_tab_id()
            && let Some(session) = self.tab_sessions.get(&active_id)
            && session.screen.dirty
            && let Some(ref window) = self.window
        {
            window.request_redraw();
        }
    }
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.save_workspace_state();
    }
}

impl Drop for CelerApp {
    fn drop(&mut self) {
        self.save_workspace_state();
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    crate::pty::bootstrap_env_path();
    crate::window::disable_app_nap();
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let proxy = event_loop.create_proxy();
    let mut app = CelerApp::new();
    app.proxy = Some(proxy);

    event_loop.run_app(&mut app)?;
    Ok(())
}

#[inline]
fn is_ligature_punctuation(c: char) -> bool {
    matches!(c, '-' | '>' | '=' | '<' | '!' | ':' | '/' | '*' | '.' | '|' | '&' | '~' | '#' | '+' | '%' | '?' | '^')
}

#[inline]
pub fn is_japanese_char(c: char) -> bool {
    matches!(c,
        // Hiragana
        '\u{3040}'..='\u{309F}'
        // Katakana
        | '\u{30A0}'..='\u{30FF}'
        // Katakana Phonetic Extensions
        | '\u{31F0}'..='\u{31FF}'
        // CJK Unified Ideographs (Kanji)
        | '\u{4E00}'..='\u{9FFF}'
        // CJK Extension A
        | '\u{3400}'..='\u{4DBF}'
        // CJK Compatibility Ideographs
        | '\u{F900}'..='\u{FAFF}'
        // Japanese Punctuation
        | '\u{3000}'..='\u{303F}'
        // Halfwidth and Fullwidth Forms
        | '\u{FF01}'..='\u{FF60}'
        | '\u{FFE0}'..='\u{FFE6}'
    )
}

#[cfg(target_os = "macos")]
#[inline]
fn keycode_to_digit(code: u16) -> Option<u8> {
    match code {
        29 | 82 => Some(b'0'),
        18 | 83 => Some(b'1'),
        19 | 84 => Some(b'2'),
        20 | 85 => Some(b'3'),
        21 | 86 => Some(b'4'),
        23 | 87 => Some(b'5'),
        22 | 88 => Some(b'6'),
        26 | 89 => Some(b'7'),
        28 | 91 => Some(b'8'),
        25 | 92 => Some(b'9'),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn get_current_input_source_id() -> Option<String> {
    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn TISCopyCurrentKeyboardInputSource() -> *const std::ffi::c_void;
        fn TISGetInputSourceProperty(source: *const std::ffi::c_void, property_key: *const std::ffi::c_void) -> *const std::ffi::c_void;
        static kTISPropertyInputSourceID: *const std::ffi::c_void;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringGetCString(the_string: *const std::ffi::c_void, buffer: *mut std::ffi::c_char, buffer_size: isize, encoding: u32) -> bool;
        fn CFRelease(cf: *const std::ffi::c_void);
    }

    unsafe {
        let src = TISCopyCurrentKeyboardInputSource();
        if src.is_null() {
            return None;
        }
        let prop = TISGetInputSourceProperty(src, kTISPropertyInputSourceID);
        let mut result = None;
        if !prop.is_null() {
            let mut buf = vec![0u8; 256];
            // kCFStringEncodingUTF8 = 0x08000100
            if CFStringGetCString(prop, buf.as_mut_ptr() as *mut std::ffi::c_char, buf.len() as isize, 0x08000100) {
                if let Ok(c_str) = std::ffi::CStr::from_ptr(buf.as_ptr() as *const std::ffi::c_char).to_str() {
                    result = Some(c_str.to_string());
                }
            }
        }
        CFRelease(src);
        result
    }
}

#[cfg(target_os = "macos")]
pub fn is_japanese_input_source() -> bool {
    get_current_input_source_id().map(|id| {
        let lower = id.to_lowercase();
        lower.contains("japanese") || lower.contains("kotoeri") || lower.contains("atok")
    }).unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub fn is_japanese_input_source() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn get_ime_commit_action() -> ImeCommitAction {
    use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
    use objc2_foundation::MainThreadMarker;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
    }

    let is_key_down = |k: u16| -> bool {
        unsafe {
            // 0 = kCGEventSourceStateCombinedSessionState, 1 = kCGEventSourceStateHIDSystemState
            CGEventSourceKeyState(0, k) || CGEventSourceKeyState(1, k)
        }
    };

    let is_shift_active = || -> bool {
        unsafe {
            let class_flags = NSEvent::modifierFlags_class();
            class_flags.contains(NSEventModifierFlags::NSEventModifierFlagShift)
                || is_key_down(56) // Left Shift
                || is_key_down(60) // Right Shift
        }
    };

    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        if let Some(event) = app.currentEvent() {
            unsafe {
                if event.r#type() == NSEventType::KeyDown || event.r#type() == NSEventType::KeyUp {
                    let code = event.keyCode();
                    if code == 51 || code == 117 { // Backspace (kVK_Delete) / ForwardDelete
                        return ImeCommitAction::Backspace;
                    } else if code == 49 { // Space (kVK_Space)
                        return ImeCommitAction::Append(b" ".to_vec());
                    } else if code == 36 || code == 76 { // Return / KeypadEnter
                        let is_shift = event.modifierFlags().contains(NSEventModifierFlags::NSEventModifierFlagShift)
                            || is_shift_active();
                        if is_shift {
                            return ImeCommitAction::ShiftEnter;
                        } else {
                            return ImeCommitAction::Enter;
                        }
                    } else if code == 48 { // Tab
                        return ImeCommitAction::Append(b"\t".to_vec());
                    } else if let Some(chars) = event.characters() {
                        let s = chars.to_string();
                        if !s.is_empty() {
                            let c = s.chars().next().unwrap();
                            if c.is_ascii_punctuation() || c.is_ascii_digit() {
                                return ImeCommitAction::Append(s.into_bytes());
                            }
                        }
                    } else if let Some(digit) = keycode_to_digit(code) {
                        return ImeCommitAction::Append(vec![digit]);
                    }
                }
            }
        }
    }

    // Hardware state fallback if event was already popped
    if is_key_down(51) || is_key_down(117) {
        return ImeCommitAction::Backspace;
    }
    if is_key_down(49) {
        return ImeCommitAction::Append(b" ".to_vec());
    }
    if is_key_down(36) || is_key_down(76) {
        if is_shift_active() {
            return ImeCommitAction::ShiftEnter;
        } else {
            return ImeCommitAction::Enter;
        }
    }
    if is_key_down(48) {
        return ImeCommitAction::Append(b"\t".to_vec());
    }
    const DIGIT_KEYS: &[(u16, u8)] = &[
        (29, b'0'), (82, b'0'),
        (18, b'1'), (83, b'1'),
        (19, b'2'), (84, b'2'),
        (20, b'3'), (85, b'3'),
        (21, b'4'), (86, b'4'),
        (23, b'5'), (87, b'5'),
        (22, b'6'), (88, b'6'),
        (26, b'7'), (89, b'7'),
        (28, b'8'), (91, b'8'),
        (25, b'9'), (92, b'9'),
    ];
    for &(kc, digit) in DIGIT_KEYS {
        if is_key_down(kc) {
            return ImeCommitAction::Append(vec![digit]);
        }
    }

    ImeCommitAction::None
}

#[cfg(not(target_os = "macos"))]
fn get_ime_commit_action() -> ImeCommitAction {
    ImeCommitAction::None
}


fn draw_outline_rect(
    buffer: &mut [u32],
    screen_size: (usize, usize),
    rect: (usize, usize, usize, usize),
    border_width: usize,
    color: u32,
) {
    let (width, height) = screen_size;
    let (x, y, w, h) = rect;
    TextRenderer::draw_rect(buffer, width, height, x, y, w, border_width, color);
    TextRenderer::draw_rect(buffer, width, height, x, y + h.saturating_sub(border_width), w, border_width, color);
    TextRenderer::draw_rect(buffer, width, height, x, y, border_width, h, color);
    TextRenderer::draw_rect(buffer, width, height, x + w.saturating_sub(border_width), y, border_width, h, color);
}


