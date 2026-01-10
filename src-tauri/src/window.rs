use tauri::{AppHandle, Manager};

pub fn show_overlay(app: &AppHandle) -> Result<(), String> {
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
