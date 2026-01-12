use crate::screenshot;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const VISION_TIMEOUT_SECS: u64 = 30;

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
        model: "moondream".to_string(),
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
        model: "moondream".to_string(),
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
