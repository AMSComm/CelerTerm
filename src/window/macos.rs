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

