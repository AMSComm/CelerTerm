pub mod macos;
pub mod tabs;

pub use tabs::{
    calculate_header_layout, calculate_modal_buttons, calculate_update_modal_buttons, ModalButton,
    Rect, TabHeaderLayout,
};
