use crate::screenshot;
use crate::types::{LlmCallType, LlmDebugEvent, LlmResponseEvent};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

const VISION_TIMEOUT_SECS: u64 = 60;
const VISION_MODEL: &str = "llava";

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    images: Vec<String>,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

pub struct ScreenElement {
    pub x: i32,
    pub y: i32,
    pub description: String,
}

pub async fn find_element(description: &str) -> Result<ScreenElement, String> {
    // Capture and resize screenshot for faster processing
    let (screenshot_bytes, scale_x, scale_y) = screenshot::capture_and_resize()?;
    let base64_image = STANDARD.encode(&screenshot_bytes);

    let prompt = format!(
        "Look at this screenshot and find the {}. \
         Output ONLY the x and y pixel coordinates of its center as two numbers separated by a comma. \
         Example output: 640, 360",
        description
    );

    let request = OllamaRequest {
        model: VISION_MODEL.to_string(),
        prompt,
        images: vec![base64_image],
        stream: false,
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(VISION_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Vision model timed out. Try a simpler element description.".to_string()
            } else {
                format!("Failed to call Ollama: {}. Is Ollama running?", e)
            }
        })?;

    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    // Parse coordinates and scale back to original screen coordinates
    let element = parse_coordinates(&ollama_response.response, description)?;

    // Scale coordinates back to original screen size
    Ok(ScreenElement {
        x: (element.x as f64 * scale_x) as i32,
        y: (element.y as f64 * scale_y) as i32,
        description: element.description,
    })
}

fn parse_coordinates(response: &str, description: &str) -> Result<ScreenElement, String> {
    // Clean up the response - remove common prefixes
    let cleaned = response
        .trim()
        .trim_start_matches(|c: char| !c.is_ascii_digit());

    // Pattern 1: NUMBER, NUMBER or NUMBER NUMBER (most common)
    let re1 = regex::Regex::new(r"(\d{1,4})[,\s]+(\d{1,4})").ok();
    if let Some(re) = re1 {
        if let Some(caps) = re.captures(cleaned) {
            if let (Some(x), Some(y)) = (
                caps.get(1).and_then(|m| m.as_str().parse().ok()),
                caps.get(2).and_then(|m| m.as_str().parse().ok()),
            ) {
                return Ok(ScreenElement { x, y, description: description.to_string() });
            }
        }
    }

    // Pattern 2: x=NUMBER y=NUMBER
    let re2 = regex::Regex::new(r"x\s*[=:]\s*(\d+).*?y\s*[=:]\s*(\d+)").ok();
    if let Some(re) = re2 {
        if let Some(caps) = re.captures(response) {
            if let (Some(x), Some(y)) = (
                caps.get(1).and_then(|m| m.as_str().parse().ok()),
                caps.get(2).and_then(|m| m.as_str().parse().ok()),
            ) {
                return Ok(ScreenElement { x, y, description: description.to_string() });
            }
        }
    }

    // Pattern 3: (NUMBER, NUMBER)
    let re3 = regex::Regex::new(r"\((\d+)[,\s]+(\d+)\)").ok();
    if let Some(re) = re3 {
        if let Some(caps) = re.captures(response) {
            if let (Some(x), Some(y)) = (
                caps.get(1).and_then(|m| m.as_str().parse().ok()),
                caps.get(2).and_then(|m| m.as_str().parse().ok()),
            ) {
                return Ok(ScreenElement { x, y, description: description.to_string() });
            }
        }
    }

    Err(format!("Could not parse coordinates from: '{}'", response.trim()))
}

/// Describe the current screen state for agent observation
pub async fn describe_screen(screenshot_bytes: &[u8], goal_context: &str) -> Result<String, String> {
    println!("[VISION] Encoding screenshot to base64...");
    let base64_image = STANDARD.encode(screenshot_bytes);
    println!("[VISION] Base64 length: {} chars", base64_image.len());

    let prompt = format!(
        r#"Look at this screen. Describe in 2-3 sentences:
1. What app/window is shown
2. Notable UI elements (buttons, text fields, menus)
3. What could be clicked or typed

Goal context: {}

Description:"#,
        goal_context
    );

    let request = OllamaRequest {
        model: VISION_MODEL.to_string(),
        prompt,
        images: vec![base64_image],
        stream: false,
    };

    println!("[VISION] Sending request to Ollama (moondream)...");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(VISION_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            println!("[VISION] HTTP request FAILED: {}", e);
            if e.is_timeout() {
                "Vision model timed out".to_string()
            } else {
                format!("Failed to call Ollama: {}. Is Ollama running?", e)
            }
        })?;

    println!("[VISION] Response status: {}", response.status());

    let response_text = response.text().await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    println!("[VISION] Raw response: {}", &response_text.chars().take(200).collect::<String>());

    let ollama_response: OllamaResponse = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse Ollama response: {}. Body: {}", e, &response_text.chars().take(500).collect::<String>()))?;

    println!("[VISION] Parsed response: \"{}\"", ollama_response.response.chars().take(100).collect::<String>());
    Ok(ollama_response.response.trim().to_string())
}

/// Verify if a goal was achieved by comparing before/after screen descriptions
pub async fn verify_goal(
    goal: &str,
    success_criteria: &str,
    before_description: &str,
    after_description: &str,
) -> Result<String, String> {
    // Note: This uses text-only LLM since we already have descriptions
    // Use qwen2.5:0.5b for this
    let prompt = format!(
        r#"Did the action achieve the goal?

Goal: {}
Success means: {}
Before: {}
After: {}

Answer format:
ACHIEVED or NOT_ACHIEVED
PROGRESS or NO_PROGRESS
Brief observation (10 words max)

Answer:"#,
        goal, success_criteria, before_description, after_description
    );

    // Use text LLM for verification since we have descriptions
    crate::llm::call_ollama_raw(&prompt).await
}

pub async fn check_ollama_available() -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok();

    if let Some(client) = client {
        client
            .get("http://localhost:11434/api/tags")
            .send()
            .await
            .is_ok()
    } else {
        false
    }
}

// ============================================
// Debug versions with event emission
// ============================================

fn emit_debug_prompt(app_handle: &AppHandle, call_type: LlmCallType, prompt: &str) -> String {
    let call_id = Uuid::new_v4().to_string();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let event = LlmDebugEvent {
        call_id: call_id.clone(),
        call_type,
        model: VISION_MODEL.to_string(),
        prompt: prompt.to_string(),
        timestamp,
    };
    let _ = app_handle.emit("llm_prompt", &event);
    call_id
}

fn emit_debug_response(
    app_handle: &AppHandle,
    call_id: &str,
    raw_response: &str,
    duration_ms: u64,
    success: bool,
    error: Option<String>,
) {
    let event = LlmResponseEvent {
        call_id: call_id.to_string(),
        raw_response: raw_response.to_string(),
        parsed_result: None,
        duration_ms,
        success,
        error,
    };
    let _ = app_handle.emit("llm_response", &event);
}

/// Detected UI element with bounding box
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIElement {
    pub label: String,
    pub element_type: String,  // button, text_field, menu, icon, text, etc.
    pub x1: i32,  // top-left x
    pub y1: i32,  // top-left y
    pub x2: i32,  // bottom-right x
    pub y2: i32,  // bottom-right y
}

/// Detect UI elements on screen with bounding boxes
pub async fn detect_ui_elements(
    app_handle: &AppHandle,
    screenshot_bytes: &[u8],
    goal_context: &str,
) -> Result<Vec<UIElement>, String> {
    let base64_image = STANDARD.encode(screenshot_bytes);

    let prompt = format!(
        r#"List all clickable UI elements on this screen.
For each element, output ONE LINE in this exact format:
TYPE | LABEL | X1, Y1, X2, Y2

TYPE is one of: button, text_field, menu, icon, link, tab, checkbox
LABEL is what the element says or does (e.g., "Submit", "Search box", "Settings icon")
X1, Y1 is the top-left corner coordinate
X2, Y2 is the bottom-right corner coordinate

Example output:
button | Submit | 400, 300, 500, 340
text_field | Search box | 200, 40, 400, 60
icon | Settings gear | 760, 20, 800, 50
menu | File | 30, 15, 70, 35

Goal: {}

Elements:"#,
        goal_context
    );

    let call_id = emit_debug_prompt(app_handle, LlmCallType::ScreenDescription, &prompt);
    let start = Instant::now();

    let request = OllamaRequest {
        model: VISION_MODEL.to_string(),
        prompt: prompt.clone(),
        images: vec![base64_image],
        stream: false,
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(VISION_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let result = client
        .post("http://localhost:11434/api/generate")
        .json(&request)
        .send()
        .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(response) => {
            let response_text = response.text().await
                .map_err(|e| format!("Failed to read response body: {}", e))?;

            println!("[VISION] Raw response: {}", &response_text.chars().take(500).collect::<String>());

            let ollama_response: OllamaResponse = serde_json::from_str(&response_text)
                .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

            let raw_response = ollama_response.response.trim().to_string();
            emit_debug_response(app_handle, &call_id, &raw_response, duration_ms, true, None);

            // Parse UI elements from response
            let elements = parse_ui_elements(&raw_response);
            println!("[VISION] Detected {} UI elements", elements.len());
            for elem in &elements {
                println!("[VISION]   - {} '{}' at ({},{}) to ({},{})", elem.element_type, elem.label, elem.x1, elem.y1, elem.x2, elem.y2);
            }

            Ok(elements)
        }
        Err(e) => {
            let error_msg = if e.is_timeout() {
                "Vision model timed out".to_string()
            } else {
                format!("Failed to call Ollama: {}. Is Ollama running?", e)
            };
            emit_debug_response(app_handle, &call_id, "", duration_ms, false, Some(error_msg.clone()));
            Err(error_msg)
        }
    }
}

/// Parse UI elements from vision model response
fn parse_ui_elements(response: &str) -> Vec<UIElement> {
    let mut elements = Vec::new();

    // Pattern: TYPE | LABEL | X1, Y1, X2, Y2
    let re_bbox = regex::Regex::new(r"(\w+)\s*\|\s*(.+?)\s*\|\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)").ok();
    // Fallback pattern: TYPE | LABEL | X, Y (center point - convert to bbox with default size)
    let re_center = regex::Regex::new(r"(\w+)\s*\|\s*(.+?)\s*\|\s*(\d+)\s*,\s*(\d+)").ok();

    for line in response.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try bounding box format first
        if let Some(ref re) = re_bbox {
            if let Some(caps) = re.captures(line) {
                let element_type = caps.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
                let label = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
                let x1: i32 = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                let y1: i32 = caps.get(4).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                let x2: i32 = caps.get(5).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                let y2: i32 = caps.get(6).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);

                if !label.is_empty() && (x1 > 0 || y1 > 0 || x2 > 0 || y2 > 0) {
                    elements.push(UIElement {
                        label,
                        element_type,
                        x1,
                        y1,
                        x2,
                        y2,
                    });
                    continue;
                }
            }
        }

        // Fallback: center point format (convert to bbox with default size 50x30)
        if let Some(ref re) = re_center {
            if let Some(caps) = re.captures(line) {
                let element_type = caps.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
                let label = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
                let cx: i32 = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                let cy: i32 = caps.get(4).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);

                if !label.is_empty() && (cx > 0 || cy > 0) {
                    // Convert center to bbox with default size
                    elements.push(UIElement {
                        label,
                        element_type,
                        x1: cx - 25,
                        y1: cy - 15,
                        x2: cx + 25,
                        y2: cy + 15,
                    });
                }
            }
        }
    }

    elements
}

/// Find element with debug event emission
pub async fn find_element_with_debug(
    app_handle: &AppHandle,
    description: &str,
) -> Result<ScreenElement, String> {
    let (screenshot_bytes, scale_x, scale_y) = screenshot::capture_and_resize()?;
    let base64_image = STANDARD.encode(&screenshot_bytes);

    let prompt = format!(
        "Look at this screenshot and find the {}. \
         Output ONLY the x and y pixel coordinates of its center as two numbers separated by a comma. \
         Example output: 640, 360",
        description
    );

    let call_id = emit_debug_prompt(app_handle, LlmCallType::FindElement, &prompt);
    let start = Instant::now();

    let request = OllamaRequest {
        model: VISION_MODEL.to_string(),
        prompt: prompt.clone(),
        images: vec![base64_image],
        stream: false,
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(VISION_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let result = client
        .post("http://localhost:11434/api/generate")
        .json(&request)
        .send()
        .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(response) => {
            let ollama_response: OllamaResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

            emit_debug_response(app_handle, &call_id, &ollama_response.response, duration_ms, true, None);

            let element = parse_coordinates(&ollama_response.response, description)?;
            Ok(ScreenElement {
                x: (element.x as f64 * scale_x) as i32,
                y: (element.y as f64 * scale_y) as i32,
                description: element.description,
            })
        }
        Err(e) => {
            let error_msg = if e.is_timeout() {
                "Vision model timed out. Try a simpler element description.".to_string()
            } else {
                format!("Failed to call Ollama: {}. Is Ollama running?", e)
            };
            emit_debug_response(app_handle, &call_id, "", duration_ms, false, Some(error_msg.clone()));
            Err(error_msg)
        }
    }
}

/// Verify goal with debug event emission
pub async fn verify_goal_with_debug(
    app_handle: &AppHandle,
    goal: &str,
    success_criteria: &str,
    before_description: &str,
    after_description: &str,
) -> Result<String, String> {
    let prompt = format!(
        r#"Did the action achieve the goal?

Goal: {}
Success means: {}
Before: {}
After: {}

Answer format:
ACHIEVED or NOT_ACHIEVED
PROGRESS or NO_PROGRESS
Brief observation (10 words max)

Answer:"#,
        goal, success_criteria, before_description, after_description
    );

    crate::llm::call_ollama_with_debug(app_handle, &prompt, LlmCallType::Verification).await
}
