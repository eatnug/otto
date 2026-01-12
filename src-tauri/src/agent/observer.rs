use crate::screenshot;
use crate::types::{DetectedElement, ScreenState};
use crate::vision;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Observer;

impl Observer {
    pub fn new() -> Self {
        Self
    }

    /// Observe the current screen state
    pub async fn observe(&self, goal_context: &str) -> Result<ScreenState, String> {
        println!("[OBSERVER] Capturing screenshot...");
        // Capture and resize screenshot
        let (screenshot_bytes, _scale_x, _scale_y) = screenshot::capture_and_resize()
            .map_err(|e| {
                println!("[OBSERVER] Screenshot capture FAILED: {}", e);
                e
            })?;
        println!("[OBSERVER] Screenshot captured: {} bytes", screenshot_bytes.len());
        let screenshot_hash = hash_bytes(&screenshot_bytes);

        println!("[OBSERVER] Calling vision model (moondream)...");
        // Get screen description from vision model
        let description = vision::describe_screen(&screenshot_bytes, goal_context).await
            .map_err(|e| {
                println!("[OBSERVER] Vision model FAILED: {}", e);
                e
            })?;
        println!("[OBSERVER] Vision response length: {} chars", description.len());
        if description.is_empty() {
            println!("[OBSERVER] WARNING: Vision returned empty string!");
        }

        // Get active app
        let active_app = get_frontmost_app();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Ok(ScreenState {
            timestamp,
            description,
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
