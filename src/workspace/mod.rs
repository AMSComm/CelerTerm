pub mod manager;
pub mod storage;

pub use manager::{Tab, Workspace, WorkspaceManager};
pub use storage::{load_snapshot_from_file, save_snapshot_to_file, get_default_snapshot_path};
