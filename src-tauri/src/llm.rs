use crate::types::{ActionParams, ActionPlan, ActionStep, ActionType};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

const OLLAMA_TIMEOUT_SECS: u64 = 30;
const LLM_MODEL: &str = "qwen2.5:0.5b";

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

#[derive(Deserialize)]
struct LlmPlanResponse {
    steps: Vec<LlmStep>,
}

#[derive(Deserialize)]
struct LlmStep {
    #[serde(rename = "type")]
    step_type: String,
    description: String,
    params: serde_json::Value,
}

fn build_prompt(command: &str) -> String {
    format!(
        r#"Convert the command to JSON steps. Available actions: open_app, type_text, press_key, wait.
For browser search: open app, wait, press Cmd+L, type query, press return.

Command: open safari
Response: {{"steps":[{{"type":"open_app","description":"Open Safari","params":{{"app_name":"Safari"}}}}]}}

Command: open safari and search hello
Response: {{"steps":[{{"type":"open_app","description":"Open Safari","params":{{"app_name":"Safari"}}}},{{"type":"wait","description":"Wait","params":{{"ms":1000}}}},{{"type":"press_key","description":"Focus URL bar","params":{{"key":"l","modifiers":["cmd"]}}}},{{"type":"wait","description":"Wait","params":{{"ms":200}}}},{{"type":"type_text","description":"Type query","params":{{"text":"hello"}}}},{{"type":"press_key","description":"Search","params":{{"key":"return","modifiers":[]}}}}]}}

Command: {}
Response: "#,
        command
    )
}

async fn call_ollama(prompt: &str) -> Result<String, String> {
    let request = OllamaRequest {
        model: LLM_MODEL.to_string(),
        prompt: prompt.to_string(),
        stream: false,
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(OLLAMA_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "LLM request timed out".to_string()
            } else {
                format!("Failed to call Ollama: {}. Is Ollama running?", e)
            }
        })?;

    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    Ok(ollama_response.response)
}

fn parse_step(step: &LlmStep) -> Result<ActionStep, String> {
    let action_type = match step.step_type.as_str() {
        "open_app" => ActionType::OpenApp,
        "type_text" => ActionType::TypeText,
        "press_key" => ActionType::PressKey,
        "mouse_click" => ActionType::MouseClick,
        "mouse_move" => ActionType::MouseMove,
        "wait" => ActionType::Wait,
        "find_and_click" => ActionType::FindAndClick,
        other => return Err(format!("Unknown action type: {}", other)),
    };

    let params = match step.step_type.as_str() {
        "open_app" => {
            let app_name = step.params["app_name"]
                .as_str()
                .ok_or("Missing app_name")?
                .to_string();
            ActionParams::OpenApp { app_name }
        }
        "type_text" => {
            let text = step.params["text"]
                .as_str()
                .ok_or("Missing text")?
                .to_string();
            ActionParams::TypeText { text }
        }
        "press_key" => {
            let key = step.params["key"]
                .as_str()
                .ok_or("Missing key")?
                .to_string();
            let modifiers = step.params["modifiers"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                });
            ActionParams::PressKey { key, modifiers }
        }
        "mouse_click" => {
            let x = step.params["x"].as_i64().ok_or("Missing x")? as i32;
            let y = step.params["y"].as_i64().ok_or("Missing y")? as i32;
            let button = step.params["button"].as_str().map(|s| s.to_string());
            ActionParams::MouseClick { x, y, button }
        }
        "mouse_move" => {
            let x = step.params["x"].as_i64().ok_or("Missing x")? as i32;
            let y = step.params["y"].as_i64().ok_or("Missing y")? as i32;
            ActionParams::MouseMove { x, y }
        }
        "wait" => {
            let ms = step.params["ms"].as_u64().ok_or("Missing ms")?;
            ActionParams::Wait { ms }
        }
        "find_and_click" => {
            let element = step.params["element"]
                .as_str()
                .ok_or("Missing element")?
                .to_string();
            ActionParams::FindAndClick { element }
        }
        _ => return Err("Invalid step type".to_string()),
    };

    Ok(ActionStep {
        id: Uuid::new_v4().to_string(),
        action_type,
        description: step.description.clone(),
        params,
    })
}

fn extract_json(text: &str) -> Option<String> {
    let text = text.trim();

    // Find {"steps":[ and extract until matching ]}
    let start = text.find(r#"{"steps":["#)?;
    let after_start = &text[start + 10..];

    let mut depth = 1;
    let mut end_of_array = 0;

    for (i, c) in after_start.char_indices() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    end_of_array = i;
                    break;
                }
            }
            _ => {}
        }
    }

    if end_of_array > 0 {
        let steps_content = &after_start[..end_of_array];
        Some(format!(r#"{{"steps":[{}]}}"#, steps_content))
    } else {
        None
    }
}

// Try to parse common command patterns directly without LLM
fn try_parse_direct(command: &str) -> Option<Vec<ActionStep>> {
    let cmd = command.to_lowercase();

    // Pattern: "search X in safari" or "safari에서 X 검색"
    if let Some(caps) = regex::Regex::new(r"(?i)search\s+(.+?)\s+in\s+(\w+)").ok()?.captures(&cmd) {
        let query = caps.get(1)?.as_str().trim();
        let app = caps.get(2)?.as_str().trim();
        return Some(build_search_steps(app, query));
    }

    // Pattern: "open safari and search X"
    if let Some(caps) = regex::Regex::new(r"(?i)open\s+(\w+)\s+and\s+search\s+(.+)").ok()?.captures(&cmd) {
        let app = caps.get(1)?.as_str().trim();
        let query = caps.get(2)?.as_str().trim();
        return Some(build_search_steps(app, query));
    }

    // Pattern: "open X"
    if let Some(caps) = regex::Regex::new(r"(?i)^open\s+(\w+)$").ok()?.captures(&cmd) {
        let app = caps.get(1)?.as_str().trim();
        return Some(vec![
            ActionStep {
                id: Uuid::new_v4().to_string(),
                action_type: ActionType::OpenApp,
                description: format!("Open {}", capitalize(app)),
                params: ActionParams::OpenApp { app_name: capitalize(app) },
            }
        ]);
    }

    // Pattern: "click on X" or "click the X"
    if let Some(caps) = regex::Regex::new(r"(?i)click\s+(?:on\s+)?(?:the\s+)?(.+)").ok()?.captures(&cmd) {
        let element = caps.get(1)?.as_str().trim();
        return Some(vec![
            ActionStep {
                id: Uuid::new_v4().to_string(),
                action_type: ActionType::FindAndClick,
                description: format!("Find and click: {}", element),
                params: ActionParams::FindAndClick { element: element.to_string() },
            }
        ]);
    }

    // Pattern: "find and click X"
    if let Some(caps) = regex::Regex::new(r"(?i)find\s+and\s+click\s+(.+)").ok()?.captures(&cmd) {
        let element = caps.get(1)?.as_str().trim();
        return Some(vec![
            ActionStep {
                id: Uuid::new_v4().to_string(),
                action_type: ActionType::FindAndClick,
                description: format!("Find and click: {}", element),
                params: ActionParams::FindAndClick { element: element.to_string() },
            }
        ]);
    }

    // Pattern: "type X"
    if let Some(caps) = regex::Regex::new(r"(?i)^type\s+(.+)$").ok()?.captures(&cmd) {
        let text = caps.get(1)?.as_str().trim();
        return Some(vec![
            ActionStep {
                id: Uuid::new_v4().to_string(),
                action_type: ActionType::TypeText,
                description: format!("Type: {}", text),
                params: ActionParams::TypeText { text: text.to_string() },
            }
        ]);
    }

    None
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn build_search_steps(app: &str, query: &str) -> Vec<ActionStep> {
    vec![
        ActionStep {
            id: Uuid::new_v4().to_string(),
            action_type: ActionType::OpenApp,
            description: format!("Open {}", capitalize(app)),
            params: ActionParams::OpenApp { app_name: capitalize(app) },
        },
        ActionStep {
            id: Uuid::new_v4().to_string(),
            action_type: ActionType::Wait,
            description: "Wait for app".to_string(),
            params: ActionParams::Wait { ms: 1000 },
        },
        ActionStep {
            id: Uuid::new_v4().to_string(),
            action_type: ActionType::PressKey,
            description: "Focus URL bar (Cmd+L)".to_string(),
            params: ActionParams::PressKey { key: "l".to_string(), modifiers: Some(vec!["cmd".to_string()]) },
        },
        ActionStep {
            id: Uuid::new_v4().to_string(),
            action_type: ActionType::Wait,
            description: "Wait".to_string(),
            params: ActionParams::Wait { ms: 200 },
        },
        ActionStep {
            id: Uuid::new_v4().to_string(),
            action_type: ActionType::TypeText,
            description: format!("Type: {}", query),
            params: ActionParams::TypeText { text: query.to_string() },
        },
        ActionStep {
            id: Uuid::new_v4().to_string(),
            action_type: ActionType::PressKey,
            description: "Search".to_string(),
            params: ActionParams::PressKey { key: "return".to_string(), modifiers: Some(vec![]) },
        },
    ]
}

pub async fn generate_plan(command: &str) -> Result<ActionPlan, String> {
    // Try direct parsing first (fast path)
    if let Some(steps) = try_parse_direct(command) {
        return Ok(ActionPlan {
            id: Uuid::new_v4().to_string(),
            original_command: command.to_string(),
            steps,
            requires_confirmation: false,
        });
    }

    // Fall back to LLM for complex commands
    let prompt = build_prompt(command);
    let response = call_ollama(&prompt).await?;

    // Extract first valid JSON object
    let json_str = extract_json(&response)
        .ok_or_else(|| format!("No valid JSON found in response: {}", response))?;

    let llm_plan: LlmPlanResponse = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse LLM response: {}. Response was: {}", e, json_str))?;

    let steps: Result<Vec<ActionStep>, String> = llm_plan.steps.iter().map(parse_step).collect();

    Ok(ActionPlan {
        id: Uuid::new_v4().to_string(),
        original_command: command.to_string(),
        steps: steps?,
        requires_confirmation: false,
    })
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
