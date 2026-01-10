use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::window::toggle_overlay;

pub fn register_global_hotkey(app: &AppHandle) -> Result<(), String> {
    let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space);

    app.global_shortcut()
        .on_shortcut(shortcut, {
            let app_handle = app.clone();
            move |_app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    let _ = toggle_overlay(&app_handle);
                }
            }
        })
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[allow(dead_code)]
pub fn unregister_global_hotkey(app: &AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;
    Ok(())
}
