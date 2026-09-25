use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowAttributes, WindowId};
use crate::config::load_config;
use crate::window::macos::{configure_macos_window, apply_traffic_lights_visibility};
use crate::window::tabs::calculate_header_layout;
use crate::workspace::WorkspaceManager;
use crate::pty::PtySession;
use crate::term::{TermScreen, translate_key, Modifiers, KeyAction};
use log::info;

pub struct CelerApp {
    window: Option<Arc<Window>>,
    config: crate::config::Config,
    workspace_mgr: WorkspaceManager,
    pty: Option<PtySession>,
    _screen: TermScreen,
    modifiers: ModifiersState,
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
        let screen = TermScreen::new(80, 24);

        Self {
            window: None,
            config,
            workspace_mgr,
            pty: None,
            _screen: screen,
            modifiers: ModifiersState::default(),
        }
    }
}

impl ApplicationHandler for CelerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attrs = WindowAttributes::default()
            .with_title("CelerTerm")
            .with_inner_size(winit::dpi::LogicalSize::new(900.0, 600.0));

        attrs = configure_macos_window(attrs, &self.config.window);

        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window = Arc::new(window);
                #[cfg(target_os = "macos")]
                apply_traffic_lights_visibility(&window, self.config.window.hide_traffic_lights);

                // Spawn PTY session with user shell
                let pty_res = PtySession::spawn(80, 24, None);
                match pty_res {
                    Ok(pty) => self.pty = Some(pty),
                    Err(e) => eprintln!("Failed to spawn PTY: {e}"),
                }

                self.window = Some(window);
                info!("CelerTerm window created successfully.");
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
                            if let Ok(id) = self.workspace_mgr.select_tab_by_1_index(idx) {
                                println!("[Shortcut] Switched to Tab {idx} ({id})");
                            }
                        }
                        KeyAction::PreviousTab => {
                            if let Ok(id) = self.workspace_mgr.select_previous_tab() {
                                println!("[Shortcut] Previous Tab ({id})");
                            }
                        }
                        KeyAction::NextTab => {
                            if let Ok(id) = self.workspace_mgr.select_next_tab() {
                                println!("[Shortcut] Next Tab ({id})");
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
            WindowEvent::RedrawRequested => {
                if let Some(ref window) = self.window {
                    let size = window.inner_size();
                    let tabs: Vec<(String, String)> = self.workspace_mgr.get_active_workspace()
                        .map(|ws| ws.tabs.iter().map(|t| (t.id.clone(), t.title.clone())).collect())
                        .unwrap_or_default();

                    let _header = calculate_header_layout(
                        size.width as f32,
                        &tabs,
                        self.config.window.hide_traffic_lights,
                        self.config.window.tabs_in_titlebar,
                    );
                }
            }
            _ => {}
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = CelerApp::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
