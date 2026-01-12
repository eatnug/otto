use crate::screenshot;
use crate::types::{DetectedElement, ScreenState, UIElement};
use crate::vision;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

pub struct Observer;

impl Observer {
    pub fn new() -> Self {
        Self
    }

    /// Observe the current screen state - detect UI elements
    pub async fn observe(&self, app_handle: &AppHandle, goal_context: &str) -> Result<ScreenState, String> {
        println!("[OBSERVER] Capturing screenshot...");
        // Capture and resize screenshot
        let (screenshot_bytes, scale_x, scale_y) = screenshot::capture_and_resize()
            .map_err(|e| {
                println!("[OBSERVER] Screenshot capture FAILED: {}", e);
                e
            })?;
        println!("[OBSERVER] Screenshot captured: {} bytes", screenshot_bytes.len());
        let screenshot_hash = hash_bytes(&screenshot_bytes);

        println!("[OBSERVER] Calling vision model to detect UI elements...");
        // Detect UI elements from screenshot
        let vision_elements = vision::detect_ui_elements(app_handle, &screenshot_bytes, goal_context).await
            .map_err(|e| {
                println!("[OBSERVER] Vision model FAILED: {}", e);
                e
            })?;

        // Scale coordinates back to original screen size
        let ui_elements: Vec<UIElement> = vision_elements
            .into_iter()
            .map(|e| UIElement {
                label: e.label,
                element_type: e.element_type,
                x1: (e.x1 as f64 * scale_x) as i32,
                y1: (e.y1 as f64 * scale_y) as i32,
                x2: (e.x2 as f64 * scale_x) as i32,
                y2: (e.y2 as f64 * scale_y) as i32,
            })
            .collect();

        println!("[OBSERVER] Detected {} UI elements", ui_elements.len());

        // Generate description from elements
        let description = if ui_elements.is_empty() {
            "No UI elements detected".to_string()
        } else {
            format!("Found {} UI elements", ui_elements.len())
        };

        // Get active app
        let active_app = get_frontmost_app();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Ok(ScreenState {
            timestamp,
            description,
            ui_elements,
            detected_elements: vec![],
            active_app,
            screenshot_hash,
        })
    }

    /// Find a specific element on screen
    pub async fn find_element(&self, element_description: &str) -> Result<DetectedElement, String> {
        let screen_element = vision::find_element(element_description).await?;

        Ok(DetectedElement {
            description: screen_element.description,
            location: Some((screen_element.x, screen_element.y)),
            confidence: 1.0, // moondream doesn't provide confidence scores
        })
    }

    /// Quick check if screen has changed (by hash comparison)
    pub fn screen_changed(&self, old_hash: &str, new_hash: &str) -> bool {
        old_hash != new_hash
    }
}

/// Get the frontmost application name
fn get_frontmost_app() -> Option<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to get name of first application process whose frontmost is true"#)
        .output()
        .ok()?;

    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

/// Hash bytes for quick comparison
fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

impl Default for Observer {
    fn default() -> Self {
        Self::new()
    }
}
