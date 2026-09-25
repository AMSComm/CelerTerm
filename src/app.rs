use std::num::NonZeroU32;
use std::sync::Arc;
use std::io::Read;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowAttributes, WindowId};
use crate::config::load_config;
use crate::window::macos::{configure_macos_window, apply_traffic_lights_visibility};
use crate::window::tabs::calculate_header_layout;
use crate::workspace::WorkspaceManager;
use crate::pty::PtySession;
use crate::term::{TermScreen, translate_key, Modifiers, KeyAction};
use crate::renderer::TextRenderer;
use log::info;

#[derive(Debug)]
pub enum UserEvent {
    PtyOutput(Vec<u8>),
}

pub struct CelerApp {
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    config: crate::config::Config,
    workspace_mgr: WorkspaceManager,
    pty: Option<PtySession>,
    screen: TermScreen,
    renderer: TextRenderer,
    modifiers: ModifiersState,
    proxy: Option<EventLoopProxy<UserEvent>>,
    mouse_pos: (f64, f64),
}

impl Default for CelerApp {
    fn default() -> Self {
        Self::new()
    }
}

impl CelerApp {
    pub fn new() -> Self {
        let config = load_config();
        let workspace_mgr = WorkspaceManager::new();
        let screen = TermScreen::new(100, 30);
        let renderer = TextRenderer::new(&config.font.family, config.font.size, config.font.line_height);

        Self {
            window: None,
            surface: None,
            config,
            workspace_mgr,
            pty: None,
            screen,
            renderer,
            modifiers: ModifiersState::default(),
            proxy: None,
            mouse_pos: (0.0, 0.0),
        }
    }
}

impl ApplicationHandler<UserEvent> for CelerApp {
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::PtyOutput(bytes) => {
                self.screen.process_bytes(&bytes);
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
            .with_inner_size(winit::dpi::LogicalSize::new(960.0, 600.0));

        attrs = configure_macos_window(attrs, &self.config.window);

        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window = Arc::new(window);
                #[cfg(target_os = "macos")]
                apply_traffic_lights_visibility(&window, self.config.window.hide_traffic_lights);

                // Setup softbuffer surface for zero-latency 60/120fps blitting
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

                // Spawn PTY session with user shell
                let mut pty_res = PtySession::spawn(100, 30, None);
                if let Ok(ref mut pty) = pty_res
                    && let Some(proxy) = &self.proxy
                {
                    let proxy_clone = proxy.clone();
                    // Split off reader into dedicated background thread
                    // We use reader to stream shell output directly into screen buffer
                    let mut reader = std::mem::replace(
                        &mut pty.reader,
                        Box::new(std::io::empty()) as Box<dyn Read + Send>,
                    );

                    std::thread::spawn(move || {
                        let mut buf = [0u8; 4096];
                        loop {
                            match reader.read(&mut buf) {
                                Ok(0) => break, // Process EOF
                                Ok(n) => {
                                    if proxy_clone.send_event(UserEvent::PtyOutput(buf[..n].to_vec())).is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    });
                }

                match pty_res {
                    Ok(pty) => self.pty = Some(pty),
                    Err(e) => eprintln!("Failed to spawn PTY: {e}"),
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
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(new_mods) => {
                self.modifiers = new_mods.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x, position.y);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(ref window) = self.window {
                    let size = window.inner_size();
                    let tabs: Vec<(String, String)> = self.workspace_mgr.get_active_workspace()
                        .map(|ws| ws.tabs.iter().map(|t| (t.id.clone(), t.title.clone())).collect())
                        .unwrap_or_default();

                    let header = calculate_header_layout(
                        size.width as f32,
                        &tabs,
                        self.config.window.hide_traffic_lights,
                        self.config.window.tabs_in_titlebar,
                    );

                    let (mx, my) = (self.mouse_pos.0 as f32, self.mouse_pos.1 as f32);

                    // Check if clicked in header
                    if my <= header.height {
                        let mut clicked_tab = false;
                        for (tab_id, rect) in &header.tab_rects {
                            if rect.contains(mx, my) {
                                if let Some(ws) = self.workspace_mgr.get_active_workspace_mut() {
                                    ws.active_tab_id = tab_id.clone();
                                    window.request_redraw();
                                }
                                clicked_tab = true;
                                break;
                            }
                        }

                        if !clicked_tab && header.add_button_rect.contains(mx, my) {
                            let _ = self.workspace_mgr.new_tab(std::env::current_dir().unwrap_or_default());
                            window.request_redraw();
                        } else if !clicked_tab {
                            // Drag window when clicking empty header space
                            let _ = window.drag_window();
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    logical_key,
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } => {
                let mods = Modifiers {
                    alt: self.modifiers.alt_key(),
                    ctrl: self.modifiers.control_key(),
                    shift: self.modifiers.shift_key(),
                    logo: self.modifiers.super_key(),
                };

                if let Some(action) = translate_key(&logical_key, mods, self.config.macos.option_as_alt) {
                    match action {
                        KeyAction::SelectTab(idx) => {
                            if self.workspace_mgr.select_tab_by_1_index(idx).is_ok()
                                && let Some(ref window) = self.window
                            {
                                window.request_redraw();
                            }
                        }
                        KeyAction::PreviousTab => {
                            if self.workspace_mgr.select_previous_tab().is_ok()
                                && let Some(ref window) = self.window
                            {
                                window.request_redraw();
                            }
                        }
                        KeyAction::NextTab => {
                            if self.workspace_mgr.select_next_tab().is_ok()
                                && let Some(ref window) = self.window
                            {
                                window.request_redraw();
                            }
                        }
                        KeyAction::Bytes(bytes) => {
                            if let Some(ref mut pty) = self.pty {
                                let _ = pty.write_all(&bytes);
                            }
                        }
                        KeyAction::Text(text) => {
                            if let Some(ref mut pty) = self.pty {
                                let _ = pty.write_all(text.as_bytes());
                            }
                        }
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if let (Some(w), Some(h)) = (NonZeroU32::new(new_size.width), NonZeroU32::new(new_size.height)) {
                    if let Some(ref mut surface) = self.surface {
                        let _ = surface.resize(w, h);
                    }

                    let header_h = if self.config.window.tabs_in_titlebar { 34.0 } else { 0.0 };
                    let term_h = (new_size.height as f32 - header_h - 8.0).max(10.0);
                    let term_w = (new_size.width as f32 - 16.0).max(10.0);

                    let cols = (term_w / self.renderer.cell_width).floor() as usize;
                    let rows = (term_h / self.renderer.cell_height).floor() as usize;

                    if cols > 0 && rows > 0 {
                        self.screen.resize(cols, rows);
                        if let Some(ref pty) = self.pty {
                            let _ = pty.resize(cols as u16, rows as u16);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
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

                    // 1. Fill terminal background (Tokyo Night deep navy #1a1b26)
                    let bg_color = 0x001A1B26;
                    buffer.fill(bg_color);

                    // 2. Render Tab Header
                    let tabs: Vec<(String, String)> = self.workspace_mgr.get_active_workspace()
                        .map(|ws| ws.tabs.iter().map(|t| (t.id.clone(), t.title.clone())).collect())
                        .unwrap_or_default();

                    let active_tab_id = self.workspace_mgr.get_active_workspace()
                        .map(|ws| ws.active_tab_id.clone())
                        .unwrap_or_default();

                    let header = calculate_header_layout(
                        width as f32,
                        &tabs,
                        self.config.window.hide_traffic_lights,
                        self.config.window.tabs_in_titlebar,
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

                            let tab_title = format!("{}. {}", idx + 1, tabs.get(idx).map(|t| t.1.as_str()).unwrap_or("Tab"));
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                rect.x + 8.0,
                                rect.y + 2.0,
                                &tab_title,
                                tab_fg,
                            );
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
                        self.renderer.draw_text(
                            &mut buffer,
                            width,
                            height,
                            header.add_button_rect.x + 7.0,
                            header.add_button_rect.y + 2.0,
                            "+",
                            0x007AA2F7,
                        );
                    }

                    // 3. Render Terminal Screen Grid
                    let start_y = header.height + self.config.window.padding_y;
                    let pad_x = self.config.window.padding_x;
                    let cell_h = self.renderer.cell_height;

                    for line_idx in 0..self.screen.size.lines {
                        let y = start_y + (line_idx as f32) * cell_h;
                        if y + cell_h > height as f32 {
                            break;
                        }

                        let line_str = self.screen.get_line_string(line_idx);
                        if !line_str.is_empty() {
                            self.renderer.draw_text(
                                &mut buffer,
                                width,
                                height,
                                pad_x,
                                y,
                                &line_str,
                                0x00C0CAF5, // Bright terminal foreground
                            );
                        }
                    }

                    // 4. Render Cursor
                    let (cursor_col, cursor_row) = self.screen.cursor_position();
                    let cursor_x = pad_x + (cursor_col as f32) * self.renderer.cell_width;
                    let cursor_y = start_y + (cursor_row as f32) * cell_h;

                    if cursor_y + cell_h <= height as f32 && cursor_x + self.renderer.cell_width <= width as f32 {
                        TextRenderer::draw_rect(
                            &mut buffer,
                            width,
                            height,
                            cursor_x as usize,
                            cursor_y as usize,
                            self.renderer.cell_width as usize,
                            cell_h as usize,
                            0x007AA2F7, // Neon blue block cursor
                        );
                    }

                    let _ = buffer.present();
                }
            }
            _ => {}
        }
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
