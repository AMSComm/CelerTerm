pub mod session;
pub mod process;
pub mod resource;

pub use session::{PtySession, bootstrap_env_path, get_bootstrapped_path};
pub use process::{
    format_tab_title, format_tab_title_to_fit, truncate_tab_title, MAX_TAB_TITLE_CHARS, get_process_cwd, get_process_name,
    get_raw_process_name, get_child_pids, extract_command_from_shell_args, is_generic_runner, is_shell_name,
};
pub use resource::{
    ResourceSnapshot, WorkspaceResource, TabResource, PaneResource, ProcessUsage,
    collect_process_tree, format_bytes, format_duration_ns,
};
