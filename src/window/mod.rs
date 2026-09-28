pub mod macos;
pub mod tabs;

pub use tabs::{
    calculate_header_layout, calculate_modal_buttons, calculate_update_modal_buttons,
    calculate_update_modal_buttons_with_label, ModalButton, Rect, TabHeaderLayout,
};
pub use macos::{
    apply_traffic_lights_visibility, configure_macos_window, disable_app_nap,
    set_macos_app_icon, set_macos_dock_badge, set_macos_menu_title, set_macos_process_name,
};
