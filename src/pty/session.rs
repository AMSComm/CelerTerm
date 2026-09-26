use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use parking_lot::Mutex;

pub struct PtySession {
    pub master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child_pid: Option<u32>,
}

impl PtySession {
    pub fn spawn(cols: u16, rows: u16, cwd: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        #[cfg(unix)]
        cmd.arg("-l");

        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "CelerTerm");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));

        let bootstrapped_path = get_bootstrapped_path();
        cmd.env("PATH", &bootstrapped_path);

        if let Some(dir) = cwd
            && dir.exists()
        {
            cmd.cwd(dir);
        }

        let child = pair.slave.spawn_command(cmd)?;
        let child_pid = child.process_id();
        drop(pair.slave); // Required on Unix so EOF is triggered when child exits

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok(Self {
            master: Arc::new(Mutex::new(pair.master)),
            reader,
            writer,
            child_pid,
        })
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), Box<dyn std::error::Error>> {
        let master = self.master.lock();
        master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    pub fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(buf)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn foreground_process_id(&self) -> Option<u32> {
        #[cfg(unix)]
        {
            let master = self.master.lock();
            master.process_group_leader().map(|pid| pid as u32)
        }
        #[cfg(not(unix))]
        {
            None
        }
    }
}

pub fn get_bootstrapped_path() -> String {
    let current_path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".to_string());
    let paths: Vec<String> = current_path.split(':').map(|s| s.to_string()).collect();

    let mut candidate_dirs: Vec<std::path::PathBuf> = Vec::new();
    #[cfg(target_os = "macos")]
    {
        candidate_dirs.push(std::path::PathBuf::from("/opt/homebrew/bin"));
        candidate_dirs.push(std::path::PathBuf::from("/opt/homebrew/sbin"));
        candidate_dirs.push(std::path::PathBuf::from("/usr/local/bin"));
        candidate_dirs.push(std::path::PathBuf::from("/usr/local/sbin"));
    }
    #[cfg(target_os = "linux")]
    {
        candidate_dirs.push(std::path::PathBuf::from("/usr/local/bin"));
        candidate_dirs.push(std::path::PathBuf::from("/usr/local/sbin"));
    }

    if let Some(base_dirs) = directories::BaseDirs::new() {
        let home = base_dirs.home_dir();
        candidate_dirs.push(home.join(".local/bin"));
        candidate_dirs.push(home.join(".cargo/bin"));
    }

    let mut prepend_dirs = Vec::new();
    for dir in candidate_dirs {
        if dir.exists() {
            let dir_str = dir.to_string_lossy().to_string();
            if !paths.contains(&dir_str) {
                prepend_dirs.push(dir_str);
            }
        }
    }

    if !prepend_dirs.is_empty() {
        prepend_dirs.extend(paths);
        prepend_dirs.join(":")
    } else {
        current_path
    }
}

pub fn bootstrap_env_path() {
    let bootstrapped = get_bootstrapped_path();
    unsafe {
        std::env::set_var("PATH", bootstrapped);
    }
}

