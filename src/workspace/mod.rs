pub mod manager;
pub mod storage;
pub mod instances;
pub mod palette;

pub use manager::{Tab, Workspace, WorkspaceManager};
pub use palette::{WORKSPACE_ACCENT_PALETTE, WORKSPACE_BACKGROUND_PALETTE, PaletteColor, normalize_hex};
pub use storage::{load_snapshot_from_file, save_snapshot_to_file, get_default_snapshot_path, merge_workspace_managers};
pub use instances::{
    ActiveInstance, get_default_instances_path, get_all_active_instances,
    register_active_instance, unregister_active_instance, is_process_alive,
    focus_instance, find_other_instance_in, find_other_instance_for_workspace,
};

