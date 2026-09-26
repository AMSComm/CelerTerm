use std::path::{Path, PathBuf};
use std::process::Command;
use serde::Deserialize;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "AMSComm/CelerTerm";

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate {
        current_version: String,
    },
    Available {
        current_version: String,
        latest_version: String,
        html_url: String,
        download_url: Option<String>,
        asset_name: Option<String>,
        notes: String,
        published_at: Option<String>,
    },
    Downloading {
        latest_version: String,
        status_text: String,
    },
    ReadyToRestart {
        latest_version: String,
        staged_path: PathBuf,
        target_path: PathBuf,
    },
    Error(String),
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: usize,
}

#[derive(Debug, Deserialize)]
pub struct GithubRelease {
    pub tag_name: Option<String>,
    pub html_url: Option<String>,
    pub body: Option<String>,
    pub published_at: Option<String>,
    pub message: Option<String>,
    #[serde(default)]
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub html_url: String,
    pub body: String,
    pub published_at: Option<String>,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
}

pub fn compare_versions(current: &str, latest: &str) -> std::cmp::Ordering {
    let parse_nums = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split('.')
            .map(|s| {
                s.split(|c: char| !c.is_ascii_digit())
                    .next()
                    .unwrap_or("0")
                    .parse::<u64>()
                    .unwrap_or(0)
            })
            .collect()
    };
    let c = parse_nums(current);
    let l = parse_nums(latest);
    let max_len = c.len().max(l.len());
    for i in 0..max_len {
        let ci = c.get(i).copied().unwrap_or(0);
        let li = l.get(i).copied().unwrap_or(0);
        match ci.cmp(&li) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

pub fn fetch_latest_release(repo: &str) -> Result<Option<ReleaseInfo>, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let output = Command::new("curl")
        .arg("-s")
        .arg("-H")
        .arg("User-Agent: CelerTerm")
        .arg("-H")
        .arg("Accept: application/vnd.github.v3+json")
        .arg(&url)
        .output()
        .map_err(|e| format!("Failed to run curl: {}", e))?;

    if !output.status.success() {
        return Err(format!("curl exited with code {:?}", output.status.code()));
    }

    if output.stdout.is_empty() {
        return Err("Empty response from GitHub API".to_string());
    }

    let release: GithubRelease = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse release response: {}", e))?;

    if let Some(ref msg) = release.message {
        if msg.eq_ignore_ascii_case("Not Found") {
            return Ok(None);
        }
        return Err(msg.clone());
    }

    let tag_name = match release.tag_name {
        Some(tag) if !tag.is_empty() => tag,
        _ => return Ok(None),
    };

    let html_url = release
        .html_url
        .unwrap_or_else(|| format!("https://github.com/{}/releases", repo));
    let body = release
        .body
        .unwrap_or_else(|| "New features and performance improvements.".to_string());

    let platform_asset = find_platform_asset(&release.assets);
    let download_url = platform_asset.map(|a| a.browser_download_url.clone());
    let asset_name = platform_asset.map(|a| a.name.clone());

    Ok(Some(ReleaseInfo {
        tag_name,
        html_url,
        body,
        published_at: release.published_at,
        download_url,
        asset_name,
    }))
}

pub fn find_platform_asset(assets: &[GithubAsset]) -> Option<&GithubAsset> {
    #[cfg(target_os = "macos")]
    {
        assets.iter().find(|a| a.name.ends_with(".app.zip"))
            .or_else(|| assets.iter().find(|a| a.name.ends_with(".dmg")))
    }
    #[cfg(target_os = "linux")]
    {
        assets.iter().find(|a| a.name.ends_with(".tar.gz"))
            .or_else(|| assets.iter().find(|a| a.name.ends_with(".deb")))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        assets.first()
    }
}

pub fn get_target_app_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Ok(exe) = std::env::current_exe() {
            for ancestor in exe.ancestors() {
                if ancestor.extension().and_then(|s| s.to_str()) == Some("app") {
                    return ancestor.to_path_buf();
                }
            }
        }
        let apps_path = PathBuf::from("/Applications/CelerTerm.app");
        if apps_path.exists() {
            return apps_path;
        }
        if let Some(home) = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf()) {
            let user_apps = home.join("Applications").join("CelerTerm.app");
            if user_apps.exists() {
                return user_apps;
            }
        }
        apps_path
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("/usr/local/bin/celerterm"))
    }
}

pub fn download_and_stage_update(url: &str, asset_name: &str) -> Result<PathBuf, String> {
    let update_dir = std::env::temp_dir().join("celerterm_update");
    let _ = std::fs::remove_dir_all(&update_dir);
    std::fs::create_dir_all(&update_dir)
        .map_err(|e| format!("Failed to create update temp directory: {}", e))?;

    let downloaded_file = update_dir.join(asset_name);

    let output = Command::new("curl")
        .arg("-L")
        .arg("-f")
        .arg("-s")
        .arg("-o")
        .arg(&downloaded_file)
        .arg(url)
        .output()
        .map_err(|e| format!("Failed to download update: {}", e))?;

    if !output.status.success() {
        return Err(format!("Download failed with curl code {:?}", output.status.code()));
    }

    if !downloaded_file.exists() || std::fs::metadata(&downloaded_file).map(|m| m.len()).unwrap_or(0) == 0 {
        return Err("Downloaded archive is empty or invalid".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        if asset_name.ends_with(".zip") {
            let unzip_status = Command::new("unzip")
                .arg("-q")
                .arg("-o")
                .arg(&downloaded_file)
                .arg("-d")
                .arg(&update_dir)
                .status()
                .map_err(|e| format!("Failed to run unzip: {}", e))?;

            if !unzip_status.success() {
                return Err("Failed to extract update zip archive".to_string());
            }

            let app_bundle = update_dir.join("CelerTerm.app");
            if !app_bundle.exists() {
                return Err("CelerTerm.app not found inside update archive".to_string());
            }

            let binary = app_bundle.join("Contents/MacOS/celerterm");
            if !binary.exists() {
                return Err("Binary inside CelerTerm.app is missing".to_string());
            }

            Ok(app_bundle)
        } else {
            Err("Unsupported asset format for macOS auto-update".to_string())
        }
    }

    #[cfg(target_os = "linux")]
    {
        if asset_name.ends_with(".tar.gz") {
            let tar_status = Command::new("tar")
                .arg("-xzf")
                .arg(&downloaded_file)
                .arg("-C")
                .arg(&update_dir)
                .status()
                .map_err(|e| format!("Failed to run tar: {}", e))?;

            if !tar_status.success() {
                return Err("Failed to extract update archive".to_string());
            }

            let candidate1 = update_dir.join("celerterm-linux-x86_64/celerterm");
            if candidate1.exists() {
                Ok(candidate1)
            } else {
                let candidate2 = update_dir.join("celerterm");
                if candidate2.exists() {
                    Ok(candidate2)
                } else {
                    Err("Extracted celerterm binary not found".to_string())
                }
            }
        } else {
            Err("Unsupported asset format for Linux auto-update".to_string())
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err("Auto-update is not supported on this platform".to_string())
    }
}

pub fn apply_update_and_restart(staged_path: &Path, target_path: &Path) -> Result<(), String> {
    let pid = std::process::id();

    #[cfg(target_os = "macos")]
    {
        let staged_str = staged_path.to_str().ok_or("Invalid staged path")?;
        let target_str = target_path.to_str().ok_or("Invalid target path")?;

        let script = format!(
            "while kill -0 {pid} 2>/dev/null; do sleep 0.05; done; \
             rm -rf \"{target}\"; \
             mv \"{staged}\" \"{target}\"; \
             open -n \"{target}\"",
            pid = pid,
            target = target_str,
            staged = staged_str
        );

        Command::new("sh")
            .arg("-c")
            .arg(script)
            .spawn()
            .map_err(|e| format!("Failed to spawn update restart script: {}", e))?;

        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        let staged_str = staged_path.to_str().ok_or("Invalid staged path")?;
        let target_str = target_path.to_str().ok_or("Invalid target path")?;

        let script = format!(
            "while kill -0 {pid} 2>/dev/null; do sleep 0.05; done; \
             cp -f \"{staged}\" \"{target}\"; \
             chmod +x \"{target}\"; \
             \"{target}\" &",
            pid = pid,
            target = target_str,
            staged = staged_str
        );

        Command::new("sh")
            .arg("-c")
            .arg(script)
            .spawn()
            .map_err(|e| format!("Failed to spawn update restart script: {}", e))?;

        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err("Auto-restart not supported on this platform".to_string())
    }
}

pub fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();

    #[cfg(target_os = "linux")]
    let _ = Command::new("xdg-open").arg(url).spawn();

    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd").args(["/C", "start", url]).spawn();
}
