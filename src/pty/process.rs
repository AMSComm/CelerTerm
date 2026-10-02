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

    pub fn get_raw_process_name(pid: u32) -> Option<String> {
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

    const CTL_KERN: c_int = 1;
    const KERN_PROCARGS2: c_int = 49;

    unsafe extern "C" {
        fn sysctl(
            name: *const c_int,
            namelen: u32,
            oldp: *mut c_void,
            oldlenp: *mut usize,
            newp: *mut c_void,
            newlen: usize,
        ) -> c_int;

        fn proc_listchildpids(
            ppid: c_int,
            buffer: *mut c_void,
            buffersize: c_int,
        ) -> c_int;
    }

    pub fn get_process_args(pid: u32) -> Option<Vec<String>> {
        let mib = [CTL_KERN, KERN_PROCARGS2, pid as c_int];
        let mut size: usize = 0;
        unsafe {
            if sysctl(
                mib.as_ptr(),
                3,
                std::ptr::null_mut(),
                &mut size,
                std::ptr::null_mut(),
                0,
            ) != 0
                || size < 4
            {
                return None;
            }

            let mut size = size.min(16384);
            let mut buf = vec![0u8; size];
            if sysctl(
                mib.as_ptr(),
                3,
                buf.as_mut_ptr() as *mut c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            ) != 0
                || size < 4
            {
                return None;
            }

            buf.truncate(size);
            let argc = i32::from_ne_bytes(buf[0..4].try_into().ok()?) as usize;
            if argc == 0 {
                return None;
            }

            let mut rest = &buf[4..];
            let null_pos = rest.iter().position(|&b| b == 0)?;
            rest = &rest[null_pos..];
            while !rest.is_empty() && rest[0] == 0 {
                rest = &rest[1..];
            }

            let mut args = Vec::with_capacity(argc);
            for _ in 0..argc {
                if rest.is_empty() {
                    break;
                }
                if let Some(pos) = rest.iter().position(|&b| b == 0) {
                    if let Ok(s) = std::str::from_utf8(&rest[..pos]) {
                        args.push(s.to_string());
                    }
                    rest = &rest[pos + 1..];
                } else {
                    if let Ok(s) = std::str::from_utf8(rest) {
                        args.push(s.to_string());
                    }
                    break;
                }
            }
            Some(args)
        }
    }

    pub fn get_child_pids(pid: u32) -> Vec<u32> {
        unsafe {
            let mut pids = [0 as c_int; 32];
            let bytes_needed = proc_listchildpids(
                pid as c_int,
                pids.as_mut_ptr() as *mut c_void,
                (pids.len() * std::mem::size_of::<c_int>()) as c_int,
            );
            if bytes_needed <= 0 {
                return Vec::new();
            }
            let count = (bytes_needed as usize).min(pids.len());
            pids[..count].iter().filter(|&&p| p > 0).map(|&p| p as u32).collect()
        }
    }
}

#[cfg(target_os = "linux")]
mod sys {
    use std::path::PathBuf;
    use std::fs;

    pub fn get_process_cwd(pid: u32) -> Option<PathBuf> {
        fs::read_link(format!("/proc/{}/cwd", pid)).ok()
    }

    pub fn get_raw_process_name(pid: u32) -> Option<String> {
        fs::read_to_string(format!("/proc/{}/comm", pid))
            .ok()
            .map(|s| s.trim().to_string())
    }

    pub fn get_process_args(pid: u32) -> Option<Vec<String>> {
        let bytes = fs::read(format!("/proc/{}/cmdline", pid)).ok()?;
        let args: Vec<String> = bytes
            .split(|&b| b == 0)
            .filter(|slice| !slice.is_empty())
            .filter_map(|slice| std::str::from_utf8(slice).ok().map(|s| s.to_string()))
            .collect();
        Some(args)
    }

    pub fn get_child_pids(pid: u32) -> Vec<u32> {
        if let Ok(content) = fs::read_to_string(format!("/proc/{}/task/{}/children", pid, pid)) {
            return content
                .split_whitespace()
                .filter_map(|s| s.parse::<u32>().ok())
                .collect();
        }
        Vec::new()
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod sys {
    use std::path::PathBuf;
    pub fn get_process_cwd(_pid: u32) -> Option<PathBuf> { None }
    pub fn get_raw_process_name(_pid: u32) -> Option<String> { None }
    pub fn get_process_args(_pid: u32) -> Option<Vec<String>> { None }
    pub fn get_child_pids(_pid: u32) -> Vec<u32> { Vec::new() }
}

pub fn is_shell_name(name: &str) -> bool {
    matches!(name, "zsh" | "bash" | "sh" | "fish" | "csh" | "tcsh" | "dash")
}

pub fn is_generic_runner(name: &str) -> bool {
    matches!(name, "docker" | "podman" | "sudo" | "env" | "xargs" | "nohup")
}

pub fn extract_command_from_shell_args(args: &[String]) -> Option<String> {
    if args.len() <= 1 {
        return None;
    }

    let mut iter = args[1..].iter();
    while let Some(arg) = iter.next() {
        if arg == "-c" {
            if let Some(cmd_str) = iter.next() {
                let trimmed = cmd_str.trim();
                if let Some(first_word) = trimmed.split_whitespace().next() {
                    let path = Path::new(first_word);
                    if let Some(file_name) = path.file_name() {
                        let name = file_name.to_string_lossy().trim().to_string();
                        if !name.is_empty() && !is_shell_name(&name) {
                            return Some(name);
                        }
                    }
                }
            }
            return None;
        }

        if arg.starts_with('-') {
            continue;
        }

        // First non-flag argument is the script file path
        let path = Path::new(arg);
        if let Some(file_name) = path.file_name() {
            let name = file_name.to_string_lossy().trim().to_string();
            if !name.is_empty() && !is_shell_name(&name) {
                return Some(name);
            }
        }
    }
    None
}

pub fn get_process_cwd(pid: u32) -> Option<PathBuf> {
    sys::get_process_cwd(pid)
}

pub fn get_process_name(pid: u32) -> Option<String> {
    let raw_name = sys::get_raw_process_name(pid)?;
    if !is_shell_name(&raw_name) {
        return Some(raw_name);
    }

    // Shell detected: check script argument first
    let mut resolved_command = sys::get_process_args(pid)
        .as_deref()
        .and_then(extract_command_from_shell_args);

    let mut current_pid = pid;
    for _ in 0..4 {
        let children = sys::get_child_pids(current_pid);
        if let Some(&child_pid) = children.first() {
            if let Some(child_name) = sys::get_raw_process_name(child_pid) {
                if is_shell_name(&child_name) {
                    if let Some(child_script) = sys::get_process_args(child_pid)
                        .as_deref()
                        .and_then(extract_command_from_shell_args)
                    {
                        resolved_command = Some(child_script);
                    }
                } else {
                    if let Some(ref _script) = resolved_command {
                        if !is_generic_runner(&child_name) {
                            resolved_command = Some(child_name);
                        }
                    } else {
                        resolved_command = Some(child_name);
                    }
                }
            }
            current_pid = child_pid;
        } else {
            break;
        }
    }

    resolved_command.or(Some(raw_name))
}

pub const MAX_TAB_TITLE_CHARS: usize = 20;

/// Truncate a tab title to `max_chars` unicode characters, appending an ellipsis `…` if truncated.
pub fn truncate_tab_title(title: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if title.chars().count() > max_chars {
        let prefix: String = title.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", prefix)
    } else {
        title.to_string()
    }
}

pub fn format_tab_title(
    foreground_proc: Option<&str>,
    cwd: Option<&Path>,
    dynamic_title: Option<&str>,
) -> String {
    let raw = format_tab_title_raw(foreground_proc, cwd, dynamic_title);
    truncate_tab_title(&raw, MAX_TAB_TITLE_CHARS)
}

fn format_tab_title_raw(
    foreground_proc: Option<&str>,
    cwd: Option<&Path>,
    dynamic_title: Option<&str>,
) -> String {
    let proc = foreground_proc.map(|p| p.trim()).unwrap_or("");
    let is_ssh = proc == "ssh" || proc.starts_with("ssh ") || proc == "mosh-client";

    // 1. If inside an SSH session: prefix with [🌐...]
    if is_ssh {
        if let Some(dyn_title) = dynamic_title {
            let t = dyn_title.trim();
            if !t.is_empty() {
                let proc_name = extract_ssh_process_or_title(t);
                return format!("[🌐{}]", proc_name);
            }
        }
        return "[🌐ssh]".to_string();
    }

    // 2. If a local foreground process is running and it's not a common shell
    if !proc.is_empty() {
        let is_shell = matches!(proc, "zsh" | "bash" | "sh" | "fish" | "csh" | "tcsh" | "dash");
        if !is_shell {
            return proc.to_string();
        }
    }

    // 4. Otherwise, show the folder name (or ~ for home)
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

pub fn extract_ssh_process_or_title(t: &str) -> String {
    let trimmed = t.trim();
    if trimmed.is_empty() {
        return "ssh".to_string();
    }
    // Remote bash/zsh titles often look like "user@host: ~/path" or "user@host: tail -f ..." or "user@host: ~/dir - tail"
    if let Some((_, after_colon)) = trimmed.split_once(':') {
        let after = after_colon.trim();
        // Check for trailing command: "user@host: ~/path - tail"
        if let Some((_, cmd)) = after.rsplit_once(" - ") {
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                return cmd.split_whitespace().next().unwrap_or(cmd).to_string();
            }
        }
        // If it starts with / or ~, it might be "user@host: ~" or "user@host: /var/log"
        if after.starts_with('/') || after.starts_with('~') {
            if let Some((_path, cmd)) = after.split_once(' ') {
                let cmd = cmd.trim();
                if !cmd.is_empty() {
                    return cmd.split_whitespace().next().unwrap_or(cmd).to_string();
                }
            }
            if let Some(folder) = Path::new(after).file_name() {
                return folder.to_string_lossy().to_string();
            }
            return after.to_string();
        } else {
            // e.g. "user@host: tail -f ..." -> cmd is "tail"
            if let Some(cmd) = after.split_whitespace().next() {
                return cmd.to_string();
            }
        }
    }
    // If no colon, take the first word or the full string
    if let Some(cmd) = trimmed.split_whitespace().next() {
        cmd.to_string()
    } else {
        trimmed.to_string()
    }
}
