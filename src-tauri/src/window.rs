use tauri::{AppHandle, Manager};

/// Set up the window to float above all other windows (like Spotlight)
#[cfg(target_os = "macos")]
pub fn setup_floating_window(app: &AppHandle) -> Result<(), String> {
    use std::ffi::c_void;

    println!("[WINDOW] setup_floating_window called");

    if let Some(window) = app.get_webview_window("main") {
        println!("[WINDOW] Found main window");

        // Get the native NSWindow pointer
        let ns_window_ptr = match window.ns_window() {
            Ok(w) => {
                println!("[WINDOW] Got NSWindow pointer: {:?}", w);
                w
            }
            Err(e) => {
                println!("[WINDOW] Failed to get NSWindow: {:?}", e);
                return Err(e.to_string());
            }
        };

        unsafe {
            // NSStatusWindowLevel = 25
            let level: i64 = 25;

            // NSWindowCollectionBehavior flags:
            // canJoinAllSpaces = 1 << 0 = 1
            // stationary = 1 << 4 = 16
            // fullScreenAuxiliary = 1 << 8 = 256
            let behavior: u64 = 1 | 16 | 256;

            // Use raw FFI to call setLevel: and setCollectionBehavior:
            #[link(name = "AppKit", kind = "framework")]
            extern "C" {
                fn objc_msgSend(obj: *mut c_void, sel: *mut c_void, ...) -> *mut c_void;
                fn sel_registerName(name: *const i8) -> *mut c_void;
            }

            let set_level_sel = sel_registerName(b"setLevel:\0".as_ptr() as *const i8);
            let set_behavior_sel = sel_registerName(b"setCollectionBehavior:\0".as_ptr() as *const i8);

            objc_msgSend(ns_window_ptr as *mut c_void, set_level_sel, level);
            objc_msgSend(ns_window_ptr as *mut c_void, set_behavior_sel, behavior);

            println!("[WINDOW] Set level={} and behavior={}", level, behavior);
        }

        println!("[WINDOW] Set up floating window (level=25, all spaces)");
        Ok(())
    } else {
        println!("[WINDOW] Window not found");
        Err("Window not found".to_string())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn setup_floating_window(_app: &AppHandle) -> Result<(), String> {
    Ok(()) // No-op on other platforms
}

pub fn show_overlay(app: &AppHandle) -> Result<(), String> {
    // Ensure window is set up as floating before showing
    let _ = setup_floating_window(app);

    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Window not found".to_string())
    }
}

pub fn hide_overlay(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Window not found".to_string())
    }
}

pub fn toggle_overlay(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            hide_overlay(app)
        } else {
            show_overlay(app)
        }
    } else {
        Err("Window not found".to_string())
    }
}
