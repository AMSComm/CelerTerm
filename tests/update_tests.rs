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
