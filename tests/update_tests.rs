use celerterm::update::{compare_versions, GithubRelease};
use celerterm::window::{calculate_update_modal_buttons, Rect};
use std::cmp::Ordering;

#[test]
fn test_compare_versions_basic() {
    assert_eq!(compare_versions("0.1.0", "0.1.0"), Ordering::Equal);
    assert_eq!(compare_versions("v0.1.0", "0.1.0"), Ordering::Equal);
    assert_eq!(compare_versions("0.1.0", "v0.1.0"), Ordering::Equal);
    assert_eq!(compare_versions("v0.1.0", "v0.1.0"), Ordering::Equal);

    assert_eq!(compare_versions("0.1.0", "0.1.1"), Ordering::Less);
    assert_eq!(compare_versions("0.1.0", "v0.2.0"), Ordering::Less);
    assert_eq!(compare_versions("0.1.0", "1.0.0"), Ordering::Less);

    assert_eq!(compare_versions("0.2.0", "0.1.9"), Ordering::Greater);
    assert_eq!(compare_versions("1.0.0", "0.99.99"), Ordering::Greater);
    assert_eq!(compare_versions("v2.1.0", "v2.0.9"), Ordering::Greater);
}

#[test]
fn test_compare_versions_numeric_not_lexicographical() {
    // 0.10.0 is greater than 0.2.0, unlike lexicographical sorting
    assert_eq!(compare_versions("0.10.0", "0.2.0"), Ordering::Greater);
    assert_eq!(compare_versions("0.2.0", "0.10.0"), Ordering::Less);
    assert_eq!(compare_versions("1.100.0", "1.99.0"), Ordering::Greater);
}

#[test]
fn test_compare_versions_different_component_counts() {
    assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Equal);
    assert_eq!(compare_versions("1.0.0.1", "1.0.0"), Ordering::Greater);
    assert_eq!(compare_versions("1.0.0", "1.0.0.1"), Ordering::Less);
}

#[test]
fn test_github_release_deserialization_success() {
    let json_data = r###"{
        "tag_name": "v0.1.0",
        "html_url": "https://github.com/AMSComm/CelerTerm/releases/tag/v0.1.0",
        "body": "Features:\n- Blazing fast terminal rendering\n- Workspace tabs",
        "published_at": "2026-09-26T12:00:00Z"
    }"###;

    let release: GithubRelease = serde_json::from_str(json_data).expect("Must parse valid JSON");
    assert_eq!(release.tag_name.as_deref(), Some("v0.1.0"));
    assert_eq!(
        release.html_url.as_deref(),
        Some("https://github.com/AMSComm/CelerTerm/releases/tag/v0.1.0")
    );
    assert!(release.body.unwrap().contains("Blazing fast"));
    assert_eq!(release.published_at.as_deref(), Some("2026-09-26T12:00:00Z"));
    assert!(release.message.is_none());
}

#[test]
fn test_github_release_deserialization_not_found() {
    let json_data = r###"{
        "message": "Not Found",
        "documentation_url": "https://docs.github.com/rest/releases/releases#get-the-latest-release"
    }"###;

    let release: GithubRelease = serde_json::from_str(json_data).expect("Must parse 404 JSON");
    assert_eq!(release.message.as_deref(), Some("Not Found"));
    assert!(release.tag_name.is_none());
}

#[test]
fn test_update_modal_buttons_layout_available() {
    let modal = Rect {
        x: 100.0,
        y: 80.0,
        width: 520.0,
        height: 280.0,
    };
    let buttons = calculate_update_modal_buttons(modal, 42.0, 1.0, 8.5, true);

    assert_eq!(buttons.len(), 3);
    assert_eq!(buttons[0].id, "primary");
    assert_eq!(buttons[0].label, "[Enter] Download");
    assert_eq!(buttons[1].id, "github");
    assert_eq!(buttons[1].label, "[g] GitHub");
    assert_eq!(buttons[2].id, "close");
    assert_eq!(buttons[2].label, "[Esc] Close");

    for btn in &buttons {
        assert!(btn.rect.x >= modal.x);
        assert!(btn.rect.x + btn.rect.width <= modal.x + modal.width);
        assert!(btn.rect.y >= modal.y);
        assert!(btn.rect.y + btn.rect.height <= modal.y + modal.height);
    }

    // Sequentially arranged left to right
    assert!(buttons[0].rect.x + buttons[0].rect.width <= buttons[1].rect.x);
    assert!(buttons[1].rect.x + buttons[1].rect.width <= buttons[2].rect.x);
}

#[test]
fn test_update_modal_buttons_layout_up_to_date() {
    let modal = Rect {
        x: 50.0,
        y: 50.0,
        width: 480.0,
        height: 260.0,
    };
    let buttons = calculate_update_modal_buttons(modal, 40.0, 1.0, 8.5, false);

    assert_eq!(buttons.len(), 3);
    assert_eq!(buttons[0].id, "primary");
    assert_eq!(buttons[0].label, "[Enter] Check Again");
    assert_eq!(buttons[1].id, "github");
    assert_eq!(buttons[2].id, "close");
}

#[test]
fn test_fetch_latest_release_live() {
    let res = celerterm::update::fetch_latest_release("AMSComm/CelerTerm");
    assert!(res.is_ok(), "Live GitHub API fetch must not fail: {:?}", res);
}

#[test]
fn test_find_platform_asset_matching() {
    use celerterm::update::{find_platform_asset, GithubAsset};

    let assets = vec![
        GithubAsset {
            name: "celerterm-linux-x86_64.tar.gz".to_string(),
            browser_download_url: "https://example.com/linux.tar.gz".to_string(),
            size: 1000,
        },
        GithubAsset {
            name: "CelerTerm-macOS.dmg".to_string(),
            browser_download_url: "https://example.com/mac.dmg".to_string(),
            size: 2000,
        },
        GithubAsset {
            name: "CelerTerm-macOS.app.zip".to_string(),
            browser_download_url: "https://example.com/mac.app.zip".to_string(),
            size: 3000,
        },
    ];

    let matched = find_platform_asset(&assets);
    assert!(matched.is_some());
    #[cfg(target_os = "macos")]
    {
        // On macOS, .app.zip is preferred for in-place auto-update
        assert_eq!(matched.unwrap().name, "CelerTerm-macOS.app.zip");
    }
    #[cfg(target_os = "linux")]
    {
        // On Linux, .tar.gz is preferred
        assert_eq!(matched.unwrap().name, "celerterm-linux-x86_64.tar.gz");
    }
}

#[test]
fn test_update_modal_buttons_with_custom_labels() {
    use celerterm::window::calculate_update_modal_buttons_with_label;

    let modal = Rect {
        x: 100.0,
        y: 80.0,
        width: 520.0,
        height: 280.0,
    };

    // 1. Update Now
    let btns1 = calculate_update_modal_buttons_with_label(modal, 42.0, 1.0, 8.5, "[Enter] Update Now", 0x009ECE6A);
    assert_eq!(btns1[0].label, "[Enter] Update Now");
    assert_eq!(btns1[0].color, 0x009ECE6A);

    // 2. Downloading
    let btns2 = calculate_update_modal_buttons_with_label(modal, 42.0, 1.0, 8.5, "[...] Downloading", 0x00E0AF68);
    assert_eq!(btns2[0].label, "[...] Downloading");
    assert_eq!(btns2[0].color, 0x00E0AF68);

    // 3. Restart & Update
    let btns3 = calculate_update_modal_buttons_with_label(modal, 42.0, 1.0, 8.5, "[Enter] Restart & Update", 0x007AA2F7);
    assert_eq!(btns3[0].label, "[Enter] Restart & Update");
    assert_eq!(btns3[0].color, 0x007AA2F7);
}

#[test]
fn test_get_target_app_path_validity() {
    let target = celerterm::update::get_target_app_path();
    assert!(!target.as_os_str().is_empty());
    #[cfg(target_os = "macos")]
    {
        assert!(target.to_string_lossy().ends_with(".app"));
    }
}

