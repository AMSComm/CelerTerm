use std::collections::HashMap;
use std::io::{Read, Write};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{ModifiersState, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};
use crate::config::load_config;
use crate::window::macos::{configure_macos_window, apply_traffic_lights_visibility};
use crate::window::tabs::calculate_header_layout;
use crate::workspace::WorkspaceManager;
use crate::pty::PtySession;
use crate::term::{TermScreen, translate_key_event, Modifiers, KeyAction};
use crate::renderer::{TextRenderer, resolve_color};
use parking_lot::Mutex;
use log::info;

#[derive(Debug)]
pub enum UserEvent {
    PtyOutput {
        tab_id: String,
        bytes: Vec<u8>,
    },
    PtyExited {
        tab_id: String,
    },
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

pub struct CelerApp {
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    config: crate::config::Config,
    workspace_mgr: WorkspaceManager,
    tab_sessions: HashMap<String, TabSession>,
    renderer: TextRenderer,
    modifiers: ModifiersState,
    proxy: Option<EventLoopProxy<UserEvent>>,
    mouse_pos: (f64, f64),
    ime_preedit: Option<(String, Option<(usize, usize)>)>,
    last_preedit: Option<(String, std::time::Instant)>,
    scale_factor: f32,
    cols: usize,
    rows: usize,
}

impl Default for CelerApp {
    fn default() -> Self {
        Self::new()
    }
}

impl CelerApp {
    pub fn new() -> Self {
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

        let cols = 100;
        let rows = 30;
        let renderer = TextRenderer::with_options(
            &config.font.family,
            &config.font.fallback_families,
            config.font.size,
            config.font.line_height,
            config.font.ligatures,
        );

        Self {
            window: None,
            surface: None,
            config,
            workspace_mgr,
            tab_sessions: HashMap::new(),
            renderer,
            modifiers: ModifiersState::default(),
            proxy: None,
            mouse_pos: (0.0, 0.0),
            ime_preedit: None,
            last_preedit: None,
            scale_factor: 1.0,
            cols,
            rows,
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

    pub fn recalculate_grid(&mut self, width: f32, height: f32) {
        let scale = self.scale_factor;
        let header_h = if self.config.window.tabs_in_titlebar { (26.0 * scale).round() } else { 0.0 };
        let pad_x = (self.config.window.padding_x * scale).round();
        let pad_y = (self.config.window.padding_y * scale).round();
        let term_h = (height - header_h - pad_y * 2.0).max(10.0);
        let term_w = (width - pad_x * 2.0).max(10.0);

        let cols = (term_w / self.renderer.cell_width).floor() as usize;
        let rows = (term_h / self.renderer.cell_height).floor() as usize;

        if cols > 0 && rows > 0 {
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

    pub fn spawn_tab_session(&mut self, tab_id: &str, cwd: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
        let mut pty = PtySession::spawn(self.cols as u16, self.rows as u16, cwd)?;
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
            let cwd = self.workspace_mgr.get_active_workspace()
                .and_then(|ws| ws.tabs.iter().find(|t| t.id == tab_id))
                .map(|t| t.cwd.clone());
            let _ = self.spawn_tab_session(tab_id, cwd.as_deref());
        }
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

                    // Determine title: foreground process name if running, or cwd folder name
                    if let Some(fpid) = fg_pid
                        && child_pid != Some(fpid) && fpid > 0
                    {
                        let proc_name = crate::pty::get_process_name(fpid);
                        tab.title = crate::pty::format_tab_title(proc_name.as_deref(), Some(&tab.cwd));
                        continue;
                    }

                    tab.title = crate::pty::format_tab_title(None, Some(&tab.cwd));
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
                    }
                    if save_scrollback {
                        tab.scrollback_cache = session.screen.get_scrollback_lines(max_lines);
                    }
                }
            }
        }

        if let Some(path) = crate::workspace::get_default_snapshot_path() {
            if let Err(e) = crate::workspace::save_snapshot_to_file(&self.workspace_mgr, &path) {
                log::warn!("Failed to save workspace snapshot: {e}");
            } else {
                info!("Saved workspace snapshot to {}", path.display());
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for CelerApp {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
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

        let mut attrs = WindowAttributes::default()
            .with_title("CelerTerm")
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
                let tabs_to_spawn: Vec<(String, PathBuf)> = self.workspace_mgr.get_active_workspace()
                    .map(|ws| ws.tabs.iter().map(|t| (t.id.clone(), t.cwd.clone())).collect())
                    .unwrap_or_default();

                for (tab_id, cwd) in tabs_to_spawn {
                    if let Err(e) = self.spawn_tab_session(&tab_id, Some(&cwd)) {
                        eprintln!("Failed to spawn tab PTY {tab_id}: {e}");
                    }
                }

                self.surface = Some(surface);
                self.window = Some(window);
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
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x, position.y);
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
                self.ime_preedit = None;
                self.last_preedit = None;

                // Send committed IME text (Vietnamese / Japanese) to the active tab's PTY
                if let Some(active_id) = self.active_tab_id()
                    && let Some(session) = self.tab_sessions.get_mut(&active_id)
                {
                    let _ = session.write_all(text.as_bytes());
                    // If text was committed from preedit, also send the commit key (Space, Return, Tab, punctuation)
                    // unless text already contains or ends with that suffix
                    if had_preedit {
                        if let Some(extra) = get_ime_commit_extra() {
                            let extra_str = String::from_utf8_lossy(&extra);
                            if !text.ends_with(extra_str.as_ref()) {
                                let _ = session.write_all(&extra);
                            }
                        } else if !text.ends_with(' ') && !text.ends_with('\n') && !text.ends_with('\r') {
                            // Default fallback for Vietnamese IME commit: space key
                            let _ = session.write_all(b" ");
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
                state: ElementState::Pressed,
                button,
                ..
            } => {
                let window = self.window.clone();
                if let Some(ref window) = window {
                    let size = window.inner_size();
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

                    let (mx, my) = (self.mouse_pos.0 as f32, self.mouse_pos.1 as f32);

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
                                }
                                clicked_tab = true;
                                break;
                            }
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
                            if self.workspace_mgr.previous_workspace().is_ok() {
                                if let Some(active_id) = self.active_tab_id() {
                                    self.ensure_tab_session(&active_id);
                                }
                                self.save_workspace_state();
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
                        }
                        KeyAction::NextWorkspace => {
                            if self.workspace_mgr.next_workspace().is_ok() {
                                if let Some(active_id) = self.active_tab_id() {
                                    self.ensure_tab_session(&active_id);
                                }
                                self.save_workspace_state();
                                if let Some(ref win) = window {
                                    win.request_redraw();
                                }
                            }
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
                            // Copy shortcut recognized; ready for text selection integration
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
                        let _ = surface.resize(w, h);
                    }

                    let mut buffer = match surface.buffer_mut() {
                        Ok(buf) => buf,
                        Err(_) => return,
                    };

                    #[cfg(target_os = "macos")]
                    apply_traffic_lights_visibility(window, self.config.window.hide_traffic_lights);

                    // 1. Fill terminal background (Tokyo Night navy #1a1b26)
                    let bg_color = 0x001A1B26;
                    buffer.fill(bg_color);

                    // 2. Render Tab Header
                    let raw_tabs = self.workspace_mgr.get_active_workspace()
                        .map(|ws| ws.tabs.clone())
                        .unwrap_or_default();
                    let tabs: Vec<(String, String)> = raw_tabs.iter().enumerate().map(|(idx, t)| {
                        (t.id.clone(), format!("{}. {}", idx + 1, t.title))
                    }).collect();

                    let active_tab_id = self.workspace_mgr.get_active_workspace()
                        .map(|ws| ws.active_tab_id.clone())
                        .unwrap_or_default();

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

                        // Draw Workspace Indicator badge on the right
                        let ws_label = format!("[WS: {}]", active_ws_name);
                        let ws_x = (width as f32) - (ws_label.len() as f32 * self.renderer.cell_width) - (16.0 * self.scale_factor);
                        let ws_y = ((header.height - self.renderer.cell_height) * 0.5).max(0.0);
                        if ws_x > header.add_button_rect.x + (30.0 * self.scale_factor) {
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                ws_x,
                                ws_y,
                                &ws_label,
                                0x007AA2F7,
                            );
                        }

                        // Draw tabs
                        for (idx, (tab_id, rect)) in header.tab_rects.iter().enumerate() {
                            let is_active = tab_id == &active_tab_id;
                            let tab_bg = if is_active { 0x0024283B } else { 0x001A1B26 };
                            let tab_fg = if is_active { 0x00C0CAF5 } else { 0x00787C99 };

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

                            let tab_text_y = rect.y + ((rect.height - self.renderer.cell_height) * 0.5).max(0.0);
                            let tab_text_x = rect.x + (6.0 * self.scale_factor);
                            let tab_title = tabs.get(idx).map(|t| t.1.as_str()).unwrap_or("Tab");
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                tab_text_x,
                                tab_text_y,
                                tab_title,
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
                        let default_bg = 0x001A1B26;

                        // Pass 1: Draw all cell backgrounds across lines
                        for line_idx in 0..lines {
                            let y = start_y + (line_idx as f32) * cell_h;
                            if y + cell_h > height as f32 {
                                break;
                            }
                            for col in 0..cols {
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
                                // Anchor native macOS IME candidate window right below cursor
                                if let Some(ref window) = self.window {
                                    window.set_ime_cursor_area(
                                        winit::dpi::Position::Logical(winit::dpi::LogicalPosition::new(cursor_x as f64, (cursor_y + cell_h) as f64)),
                                        winit::dpi::Size::Logical(winit::dpi::LogicalSize::new(cell_w as f64, cell_h as f64)),
                                    );
                                }

                                if let Some((ref preedit_text, _)) = self.ime_preedit {
                                    let preedit_len = preedit_text.chars().count();
                                    let preedit_w = (preedit_len as f32 * cell_w).max(cell_w);

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

                                    // Active cursor positioned immediately after the preedit text
                                    let active_cursor_x = cursor_x + preedit_w;
                                    if active_cursor_x + cell_w <= width as f32 {
                                        TextRenderer::draw_rect(
                                            &mut buffer,
                                            width,
                                            height,
                                            active_cursor_x as usize,
                                            cursor_y as usize,
                                            cell_w as usize,
                                            cell_h as usize,
                                            0x007AA2F7,
                                        );
                                    }
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

#[cfg(target_os = "macos")]
fn get_ime_commit_extra() -> Option<Vec<u8>> {
    use objc2_app_kit::{NSApplication, NSEventType};
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

    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        if let Some(event) = app.currentEvent() {
            unsafe {
                if event.r#type() == NSEventType::KeyDown || event.r#type() == NSEventType::KeyUp {
                    let code = event.keyCode();
                    if code == 49 { // Space (kVK_Space)
                        return Some(b" ".to_vec());
                    } else if code == 36 || code == 76 { // Return / KeypadEnter
                        return Some(b"\r".to_vec());
                    } else if code == 48 { // Tab
                        return Some(b"\t".to_vec());
                    } else if let Some(chars) = event.characters() {
                        let s = chars.to_string();
                        if s.len() == 1 {
                            let c = s.chars().next().unwrap();
                            if c.is_ascii_punctuation() {
                                return Some(s.into_bytes());
                            }
                        }
                    }
                }
            }
        }
    }

    // Hardware state fallback if event was already popped
    if is_key_down(49) {
        return Some(b" ".to_vec());
    }
    if is_key_down(36) || is_key_down(76) {
        return Some(b"\r".to_vec());
    }
    if is_key_down(48) {
        return Some(b"\t".to_vec());
    }

    None
}

#[cfg(not(target_os = "macos"))]
fn get_ime_commit_extra() -> Option<Vec<u8>> {
    None
}
