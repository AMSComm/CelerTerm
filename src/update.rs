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
        notes: String,
        published_at: Option<String>,
    },
    Error(String),
}

#[derive(Debug, Deserialize)]
pub struct GithubRelease {
    pub tag_name: Option<String>,
    pub html_url: Option<String>,
    pub body: Option<String>,
    pub published_at: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub html_url: String,
    pub body: String,
    pub published_at: Option<String>,
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

    Ok(Some(ReleaseInfo {
        tag_name,
        html_url,
        body,
        published_at: release.published_at,
    }))
}

pub fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();

    #[cfg(target_os = "linux")]
    let _ = Command::new("xdg-open").arg(url).spawn();

    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd").args(["/C", "start", url]).spawn();
}
