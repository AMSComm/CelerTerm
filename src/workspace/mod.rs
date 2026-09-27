pub mod manager;
pub mod storage;
pub mod instances;

pub use manager::{Tab, Workspace, WorkspaceManager};
pub use storage::{load_snapshot_from_file, save_snapshot_to_file, get_default_snapshot_path, merge_workspace_managers};
pub use instances::{
    ActiveInstance, get_default_instances_path, get_all_active_instances,
    register_active_instance, unregister_active_instance, is_process_alive,
};
