#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::window::WindowAttributes;
use crate::config::schema::WindowConfig;

pub fn configure_macos_window(
    #[allow(unused_mut)] mut attrs: WindowAttributes,
    config: &WindowConfig,
    option_as_alt: bool,
) -> WindowAttributes {
    #[cfg(target_os = "macos")]
    {
        attrs = attrs
            .with_titlebar_transparent(config.tabs_in_titlebar)
            .with_fullsize_content_view(config.tabs_in_titlebar)
            .with_title_hidden(config.tabs_in_titlebar);

        if !config.decorations {
            attrs = attrs.with_titlebar_hidden(true);
        }

        if option_as_alt {
            attrs = attrs.with_option_as_alt(winit::platform::macos::OptionAsAlt::Both);
        }
    }
    let _ = config;
    let _ = option_as_alt;
    attrs
}

#[cfg(target_os = "macos")]
pub fn apply_traffic_lights_visibility(window: &winit::window::Window, hide_traffic_lights: bool) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use objc2::runtime::AnyObject;
    use objc2::msg_send;

    if !hide_traffic_lights {
        return;
    }

    if let Ok(handle) = window.window_handle()
        && let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw()
    {
        let ns_view = appkit_handle.ns_view.as_ptr() as *mut AnyObject;
        unsafe {
            if !ns_view.is_null() {
                let ns_window: *mut AnyObject = msg_send![ns_view, window];
                if !ns_window.is_null() {
                    // NSWindowButtonClose = 0, NSWindowButtonMiniaturize = 1, NSWindowButtonZoom = 2
                    for button_type in 0..3isize {
                        let btn: *mut AnyObject = msg_send![ns_window, standardWindowButton: button_type];
                        if !btn.is_null() {
                            let () = msg_send![btn, setHidden: true];
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn apply_traffic_lights_visibility(_window: &winit::window::Window, _hide_traffic_lights: bool) {}


#[cfg(target_os = "macos")]
pub fn set_macos_app_icon(png_bytes: &[u8]) {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    if let Some(mtm) = MainThreadMarker::new() {
        unsafe {
            let app = NSApplication::sharedApplication(mtm);
            if let (Some(nsdata_cls), Some(nsimage_cls)) = (AnyClass::get("NSData"), AnyClass::get("NSImage")) {
                let data: *mut AnyObject = msg_send![nsdata_cls, dataWithBytes: png_bytes.as_ptr(), length: png_bytes.len()];
                if !data.is_null() {
                    let img_alloc: *mut AnyObject = msg_send![nsimage_cls, alloc];
                    let img: *mut AnyObject = msg_send![img_alloc, initWithData: data];
                    if !img.is_null() {
                        let () = msg_send![&*app, setApplicationIconImage: img];
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_macos_app_icon(_png_bytes: &[u8]) {}

#[cfg(target_os = "macos")]
pub fn disable_app_nap() {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};

    unsafe {
        if let Some(process_info_cls) = AnyClass::get("NSProcessInfo") {
            let process_info: *mut AnyObject = msg_send![process_info_cls, processInfo];
            if !process_info.is_null() {
                if let Some(nsstring_cls) = AnyClass::get("NSString") {
                    let reason_bytes = b"CelerTerm active terminal sessions\0";
                    let reason: *mut AnyObject = msg_send![
                        nsstring_cls,
                        stringWithUTF8String: reason_bytes.as_ptr() as *const std::ffi::c_char
                    ];
                    // NSActivityUserInitiatedAllowingIdleSystemSleep = 0x00FFFFFFULL & ~0x00100000ULL = 0x00EFFFFFULL
                    // NSActivityLatencyCritical = 0xFF00000000ULL
                    let options: u64 = 0x00EFFFFF | 0xFF00000000;
                    let activity: *mut AnyObject = msg_send![
                        process_info,
                        beginActivityWithOptions: options,
                        reason: reason
                    ];
                    if !activity.is_null() {
                        let _: *mut AnyObject = msg_send![activity, retain];
                        log::info!("macOS App Nap disabled for CelerTerm");
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn disable_app_nap() {}

#[cfg(target_os = "macos")]
pub fn set_macos_process_name(name: &str) {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};

    if let Ok(c_str) = std::ffi::CString::new(name) {
        unsafe {
            if let Some(process_info_cls) = AnyClass::get("NSProcessInfo") {
                let process_info: *mut AnyObject = msg_send![process_info_cls, processInfo];
                if !process_info.is_null() {
                    if let Some(nsstring_cls) = AnyClass::get("NSString") {
                        let ns_name: *mut AnyObject = msg_send![
                            nsstring_cls,
                            stringWithUTF8String: c_str.as_ptr()
                        ];
                        if !ns_name.is_null() {
                            let () = msg_send![process_info, setProcessName: ns_name];
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_macos_process_name(_name: &str) {}

#[cfg(target_os = "macos")]
pub fn set_macos_dock_badge(badge: Option<&str>) {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    if let Some(mtm) = MainThreadMarker::new() {
        unsafe {
            let app = NSApplication::sharedApplication(mtm);
            let dock_tile: *mut AnyObject = msg_send![&*app, dockTile];
            if !dock_tile.is_null() {
                if let Some(badge_str) = badge {
                    if let Ok(c_str) = std::ffi::CString::new(badge_str) {
                        if let Some(nsstring_cls) = AnyClass::get("NSString") {
                            let ns_badge: *mut AnyObject = msg_send![
                                nsstring_cls,
                                stringWithUTF8String: c_str.as_ptr()
                            ];
                            let () = msg_send![dock_tile, setBadgeLabel: ns_badge];
                        }
                    }
                } else {
                    let null_str: *mut AnyObject = std::ptr::null_mut();
                    let () = msg_send![dock_tile, setBadgeLabel: null_str];
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_macos_dock_badge(_badge: Option<&str>) {}

#[cfg(target_os = "macos")]
pub fn set_macos_menu_title(title: &str) {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    if let Some(mtm) = MainThreadMarker::new() {
        unsafe {
            let app = NSApplication::sharedApplication(mtm);
            let main_menu: *mut AnyObject = msg_send![&*app, mainMenu];
            if !main_menu.is_null() {
                let count: usize = msg_send![main_menu, numberOfItems];
                if count > 0 {
                    let first_item: *mut AnyObject = msg_send![main_menu, itemAtIndex: 0isize];
                    if !first_item.is_null() {
                        if let Ok(c_str) = std::ffi::CString::new(title) {
                            if let Some(nsstring_cls) = AnyClass::get("NSString") {
                                let ns_title: *mut AnyObject = msg_send![
                                    nsstring_cls,
                                    stringWithUTF8String: c_str.as_ptr()
                                ];
                                let () = msg_send![first_item, setTitle: ns_title];
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_macos_menu_title(_title: &str) {}



