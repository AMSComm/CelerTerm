use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
mod sys {
    use std::path::PathBuf;
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int, c_void};

    const PROC_PIDVNODEPATHINFO: c_int = 9;

    #[repr(C)]
    struct VnodeInfo {
        vip_vi: [c_char; 152],
        vip_path: [c_char; 1024],
    }

    #[repr(C)]
    struct VnodePathInfo {
        pvi_cdir: VnodeInfo,
        pvi_rdir: VnodeInfo,
    }

    unsafe extern "C" {
        fn proc_pidinfo(
            pid: c_int,
            flavor: c_int,
            arg: u64,
            buffer: *mut c_void,
            buffersize: c_int,
        ) -> c_int;

        fn proc_name(
            pid: c_int,
            buffer: *mut c_char,
            buffersize: u32,
        ) -> c_int;
    }

    pub fn get_process_cwd(pid: u32) -> Option<PathBuf> {
        unsafe {
            let mut info: VnodePathInfo = std::mem::zeroed();
            let size = std::mem::size_of::<VnodePathInfo>() as c_int;
            let ret = proc_pidinfo(
                pid as c_int,
                PROC_PIDVNODEPATHINFO,
                0,
                &mut info as *mut _ as *mut c_void,
                size,
            );
            if ret > 0 {
                let path_str = CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr());
                if let Ok(s) = path_str.to_str() {
                    let trimmed = s.trim_end_matches('\0');
                    if !trimmed.is_empty() {
                        return Some(PathBuf::from(trimmed));
                    }
                }
            }
        }
        None
    }

    pub fn get_process_name(pid: u32) -> Option<String> {
        unsafe {
            let mut buf = [0 as c_char; 256];
            let ret = proc_name(pid as c_int, buf.as_mut_ptr(), 256);
            if ret > 0 {
                let s = CStr::from_ptr(buf.as_ptr());
                if let Ok(name) = s.to_str() {
                    let trimmed = name.trim_end_matches('\0');
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
        None
    }
}

#[cfg(target_os = "linux")]
mod sys {
    use std::path::PathBuf;
    use std::fs;

    pub fn get_process_cwd(pid: u32) -> Option<PathBuf> {
        fs::read_link(format!("/proc/{}/cwd", pid)).ok()
    }

    pub fn get_process_name(pid: u32) -> Option<String> {
        fs::read_to_string(format!("/proc/{}/comm", pid))
            .ok()
            .map(|s| s.trim().to_string())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod sys {
    use std::path::PathBuf;
    pub fn get_process_cwd(_pid: u32) -> Option<PathBuf> { None }
    pub fn get_process_name(_pid: u32) -> Option<String> { None }
}

pub fn get_process_cwd(pid: u32) -> Option<PathBuf> {
    sys::get_process_cwd(pid)
}

pub fn get_process_name(pid: u32) -> Option<String> {
    sys::get_process_name(pid)
}

pub fn format_tab_title(
    foreground_proc: Option<&str>,
    cwd: Option<&Path>,
) -> String {
    // 1. If a foreground process is running and it's not a common shell
    if let Some(proc) = foreground_proc {
        let p = proc.trim();
        let is_shell = matches!(p, "zsh" | "bash" | "sh" | "fish" | "csh" | "tcsh" | "dash");
        if !is_shell && !p.is_empty() {
            return p.to_string();
        }
    }

    // 2. Otherwise, show the folder name (or ~ for home)
    if let Some(path) = cwd {
        if let Some(base_dirs) = directories::BaseDirs::new()
            && path == base_dirs.home_dir()
        {
            return "~".to_string();
        }
        if let Some(file_name) = path.file_name() {
            let name = file_name.to_string_lossy().to_string();
            if !name.is_empty() {
                return name;
            }
        }
        return path.to_string_lossy().to_string();
    }

    "Shell".to_string()
}
