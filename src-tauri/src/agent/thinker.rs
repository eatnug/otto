use crate::computer;
use crate::llm;
use crate::types::{ActionParams, ActionResult, ActionType, AtomicAction, Goal, LlmCallType, ScreenState};
use regex::Regex;
use tauri::AppHandle;

pub struct Thinker;

impl Thinker {
    pub fn new() -> Self {
        Self
    }

    /// Decide action WITHOUT seeing the screen (first pass)
    /// Returns an action that may or may not need coordinates
    pub async fn decide_action_blind(
        &self,
        app_handle: &AppHandle,
        goal: &Goal,
    ) -> Result<AtomicAction, String> {
        println!("[THINKER] Deciding action BLIND for goal: \"{}\"", goal.description);

        let prompt = format!(
            r#"Goal: {}

Pick ONE action to achieve this goal.
Output ONLY the action, nothing else.

Actions:
- open APP_NAME (to open an app)
- click ELEMENT_DESCRIPTION (to click something - I'll find it on screen)
- type "TEXT" (to type text)
- key KEY (e.g., key return, key CMD+L)
- wait MS (to wait)

Examples:
- Open Safari: open Safari
- Click submit button: click submit button
- Type hello: type "hello"
- Press Cmd+L: key CMD+L
- Check messages: open Messages

Action:"#,
            goal.description
        );

        println!("[THINKER] Calling LLM (blind)...");
        let response = llm::call_ollama_with_debug(app_handle, &prompt, LlmCallType::ActionDecision).await?;
        println!("[THINKER] LLM raw response: \"{}\"", response.lines().next().unwrap_or(""));

        let action = parse_action_blind(&response, goal)?;
        println!("[THINKER] Parsed action: {:?} -> {:?}", action.action_type, action.params);

        Ok(action)
    }

    /// Decide action WITH screen info (for actions that need coordinates)
    pub async fn decide_action_with_screen(
        &self,
        app_handle: &AppHandle,
        goal: &Goal,
        element_to_find: &str,
        screen: &ScreenState,
    ) -> Result<AtomicAction, String> {
        println!("[THINKER] Finding \"{}\" on screen", element_to_find);
        println!("[THINKER] UI elements detected: {}", screen.ui_elements.len());

        let ui_elements_str = screen.format_ui_elements();

        let prompt = format!(
            r#"I need to click: "{}"

UI Elements on screen:
{}

Find the best matching element and output ONLY: click X Y
Use the CENTER coordinates.

If no good match, output: not_found

Answer:"#,
            element_to_find,
            ui_elements_str
        );

        println!("[THINKER] Calling LLM (with screen)...");
        let response = llm::call_ollama_with_debug(app_handle, &prompt, LlmCallType::ActionDecision).await?;
        println!("[THINKER] LLM raw response: \"{}\"", response.lines().next().unwrap_or(""));

        let action = parse_click_action(&response, goal, element_to_find)?;
        println!("[THINKER] Parsed action: {:?} -> {:?}", action.action_type, action.params);

        Ok(action)
    }

    /// Legacy: Decide action with full screen state (for complex scenarios)
    pub async fn decide_action(
        &self,
        app_handle: &AppHandle,
        goal: &Goal,
        screen: &ScreenState,
        _history: &[ActionResult],
    ) -> Result<AtomicAction, String> {
        println!("[THINKER] Deciding action for goal: \"{}\"", goal.description);
        println!("[THINKER] UI elements detected: {}", screen.ui_elements.len());

        let ui_elements_str = screen.format_ui_elements();

        let prompt = format!(
            r#"Goal: {}

UI Elements on screen (with bounding boxes and center points):
{}

Active app: {}

Pick ONE action to achieve the goal.
Output ONLY the action, nothing else.

Actions:
- open APP_NAME (to open an app)
- click X Y (use CENTER coordinates from the list to click an element)
- type "TEXT" (to type text)
- key KEY (e.g., key return, key CMD+L)
- wait MS (to wait)

Examples:
- To click a button with center at (450, 320): click 450 320
- To open Slack: open Slack
- To type hello: type "hello"
- To press Enter: key return

Action:"#,
            goal.description,
            ui_elements_str,
            screen.active_app.as_deref().unwrap_or("Unknown")
        );

        println!("[THINKER] Calling LLM...");
        let response = llm::call_ollama_with_debug(app_handle, &prompt, LlmCallType::ActionDecision).await?;
        println!("[THINKER] LLM raw response: \"{}\"", response.lines().next().unwrap_or(""));

        let action = parse_action(&response, goal)?;
        println!("[THINKER] Parsed action: {:?} -> {:?}", action.action_type, action.params);

        Ok(action)
    }
}

/// Format recent actions for prompt context
fn format_recent_actions(history: &[ActionResult]) -> String {
    if history.is_empty() {
        return "None".to_string();
    }

    let recent: Vec<_> = history
        .iter()
        .rev()
        .take(3)
        .map(|r| {
            if r.success {
                format!("OK: {}", r.action_id)
            } else {
                format!("FAIL: {} - {}", r.action_id, r.error_message.as_deref().unwrap_or("unknown"))
            }
        })
        .collect();

    recent.join(", ")
}

/// Parse LLM response from blind decision (no screen info)
/// Click actions will have element descriptions instead of coordinates
fn parse_action_blind(response: &str, goal: &Goal) -> Result<AtomicAction, String> {
    let line = response.lines().next().unwrap_or("").trim().to_lowercase();
    let original_line = response.lines().next().unwrap_or("").trim();

    // Parse: open APP
    if let Some(caps) = Regex::new(r"^open\s+(.+)$").ok().and_then(|re| re.captures(&line)) {
        let raw_app = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        let app = normalize_app_name(raw_app);
        return Ok(AtomicAction::new(
            ActionType::OpenApp,
            ActionParams::OpenApp { app_name: app.clone() },
            format!("Opening {} for: {}", app, goal.description),
        ));
    }

    // Parse: click ELEMENT_DESCRIPTION (not coordinates)
    // This will need screen observation to resolve to coordinates
    if let Some(caps) = Regex::new(r"^click\s+(.+)$").ok().and_then(|re| re.captures(&line)) {
        let element = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        // Check if it's coordinates (digits only) or element description
        if Regex::new(r"^\d+\s+\d+$").ok().map(|re| re.is_match(element)).unwrap_or(false) {
            // It's coordinates - parse as regular click
            let parts: Vec<&str> = element.split_whitespace().collect();
            let x: i32 = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
            let y: i32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            return Ok(AtomicAction::new(
                ActionType::MouseClick,
                ActionParams::MouseClick { x, y, button: None },
                format!("Clicking at ({}, {}) for: {}", x, y, goal.description),
            ));
        }
        // It's an element description - use FindAndClick
        return Ok(AtomicAction::new(
            ActionType::FindAndClick,
            ActionParams::FindAndClick { element: element.to_string() },
            format!("Finding and clicking '{}' for: {}", element, goal.description),
        ));
    }

    // Parse: type "text"
    if let Some(caps) = Regex::new(r#"^type\s+"([^"]+)"$"#).ok().and_then(|re| re.captures(&line)) {
        let text = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        return Ok(AtomicAction::new(
            ActionType::TypeText,
            ActionParams::TypeText { text: text.to_string() },
            format!("Typing '{}' for: {}", text, goal.description),
        ));
    }

    // Parse: type text (without quotes) - preserve original case
    if let Some(caps) = Regex::new(r"^type\s+(.+)$").ok().and_then(|re| re.captures(&line)) {
        // Use original line to preserve case
        let text = if original_line.to_lowercase().starts_with("type ") {
            original_line[5..].trim()
        } else {
            caps.get(1).map(|m| m.as_str().trim()).unwrap_or("")
        };
        return Ok(AtomicAction::new(
            ActionType::TypeText,
            ActionParams::TypeText { text: text.to_string() },
            format!("Typing '{}' for: {}", text, goal.description),
        ));
    }

    // Parse: key CMD+KEY or key KEY
    if let Some(caps) = Regex::new(r"^key\s+(.+)$").ok().and_then(|re| re.captures(&line)) {
        let key_str = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        let (key, modifiers) = parse_key_combo(key_str);
        return Ok(AtomicAction::new(
            ActionType::PressKey,
            ActionParams::PressKey { key, modifiers },
            format!("Pressing {} for: {}", key_str, goal.description),
        ));
    }

    // Parse: wait MS
    if let Some(caps) = Regex::new(r"^wait\s+(\d+)$").ok().and_then(|re| re.captures(&line)) {
        let ms: u64 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(500);
        return Ok(AtomicAction::new(
            ActionType::Wait,
            ActionParams::Wait { ms },
            format!("Waiting {}ms for: {}", ms, goal.description),
        ));
    }

    Err(format!(
        "Could not parse blind action from response: '{}'. Expected: open APP, click ELEMENT, type \"text\", key KEY, or wait MS",
        line
    ))
}

/// Parse click action when we have screen coordinates
fn parse_click_action(response: &str, goal: &Goal, element: &str) -> Result<AtomicAction, String> {
    let line = response.lines().next().unwrap_or("").trim().to_lowercase();

    // Parse: click X Y
    if let Some(caps) = Regex::new(r"^click\s+(\d+)\s+(\d+)$").ok().and_then(|re| re.captures(&line)) {
        let x: i32 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        let y: i32 = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        return Ok(AtomicAction::new(
            ActionType::MouseClick,
            ActionParams::MouseClick { x, y, button: None },
            format!("Clicking '{}' at ({}, {}) for: {}", element, x, y, goal.description),
        ));
    }

    // not_found response
    if line.contains("not_found") || line.contains("no match") {
        return Err(format!("Could not find '{}' on screen", element));
    }

    Err(format!(
        "Could not parse click action from response: '{}'. Expected: click X Y or not_found",
        line
    ))
}

/// Parse LLM response into an AtomicAction
fn parse_action(response: &str, goal: &Goal) -> Result<AtomicAction, String> {
    let line = response.lines().next().unwrap_or("").trim().to_lowercase();

    // Parse: open APP
    if let Some(caps) = Regex::new(r"^open\s+(.+)$").ok().and_then(|re| re.captures(&line)) {
        let raw_app = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        let app = normalize_app_name(raw_app);
        return Ok(AtomicAction::new(
            ActionType::OpenApp,
            ActionParams::OpenApp {
                app_name: app.clone(),
            },
            format!("Opening {} for: {}", app, goal.description),
        ));
    }

    // Parse: click X Y
    if let Some(caps) = Regex::new(r"^click\s+(\d+)\s+(\d+)$")
        .ok()
        .and_then(|re| re.captures(&line))
    {
        let x: i32 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        let y: i32 = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        return Ok(AtomicAction::new(
            ActionType::MouseClick,
            ActionParams::MouseClick {
                x,
                y,
                button: None,
            },
            format!("Clicking at ({}, {}) for: {}", x, y, goal.description),
        ));
    }

    // Parse: type "text"
    if let Some(caps) = Regex::new(r#"^type\s+"([^"]+)"$"#)
        .ok()
        .and_then(|re| re.captures(&line))
    {
        let text = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        return Ok(AtomicAction::new(
            ActionType::TypeText,
            ActionParams::TypeText {
                text: text.to_string(),
            },
            format!("Typing '{}' for: {}", text, goal.description),
        ));
    }

    // Parse: type text (without quotes)
    if let Some(caps) = Regex::new(r"^type\s+(.+)$")
        .ok()
        .and_then(|re| re.captures(&line))
    {
        let text = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        return Ok(AtomicAction::new(
            ActionType::TypeText,
            ActionParams::TypeText {
                text: text.to_string(),
            },
            format!("Typing '{}' for: {}", text, goal.description),
        ));
    }

    // Parse: key CMD+KEY or key KEY
    if let Some(caps) = Regex::new(r"^key\s+(.+)$")
        .ok()
        .and_then(|re| re.captures(&line))
    {
        let key_str = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        let (key, modifiers) = parse_key_combo(key_str);
        return Ok(AtomicAction::new(
            ActionType::PressKey,
            ActionParams::PressKey { key, modifiers },
            format!("Pressing {} for: {}", key_str, goal.description),
        ));
    }

    // Parse: wait MS
    if let Some(caps) = Regex::new(r"^wait\s+(\d+)$")
        .ok()
        .and_then(|re| re.captures(&line))
    {
        let ms: u64 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(500);
        return Ok(AtomicAction::new(
            ActionType::Wait,
            ActionParams::Wait { ms },
            format!("Waiting {}ms for: {}", ms, goal.description),
        ));
    }

    Err(format!(
        "Could not parse action from response: '{}'. Expected: open APP, click X Y, type \"text\", key KEY, or wait MS",
        line
    ))
}

/// Parse key combination like "CMD+L" into (key, modifiers)
fn parse_key_combo(combo: &str) -> (String, Option<Vec<String>>) {
    let parts: Vec<&str> = combo.split('+').map(|s| s.trim()).collect();

    if parts.len() == 1 {
        return (parts[0].to_lowercase(), None);
    }

    let key = parts.last().unwrap().to_lowercase();
    let modifiers: Vec<String> = parts[..parts.len() - 1]
        .iter()
        .map(|m| m.to_lowercase())
        .collect();

    (key, Some(modifiers))
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Normalize app name by searching installed apps
fn normalize_app_name(raw: &str) -> String {
    // Remove common suffixes first
    let cleaned = raw
        .to_lowercase()
        .trim_end_matches(" browser")
        .trim_end_matches(" app")
        .trim_end_matches(" application")
        .trim()
        .to_string();

    // Try to find in installed apps
    if let Some(found) = computer::find_app(&cleaned) {
        println!("[THINKER] Found app '{}' for query '{}'", found, raw);
        return found;
    }

    // Fallback to common mappings
    match cleaned.as_str() {
        "safari" => "Safari".to_string(),
        "chrome" | "google chrome" => "Google Chrome".to_string(),
        "firefox" | "mozilla firefox" => "Firefox".to_string(),
        "finder" => "Finder".to_string(),
        "terminal" => "Terminal".to_string(),
        "vscode" | "vs code" | "visual studio code" => "Visual Studio Code".to_string(),
        "kakao" | "kakaotalk" => "KakaoTalk".to_string(),
        _ => capitalize(&cleaned),
    }
}

impl Default for Thinker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_combo() {
        let (key, mods) = parse_key_combo("CMD+L");
        assert_eq!(key, "l");
        assert_eq!(mods, Some(vec!["cmd".to_string()]));

        let (key, mods) = parse_key_combo("return");
        assert_eq!(key, "return");
        assert_eq!(mods, None);

        let (key, mods) = parse_key_combo("CMD+SHIFT+N");
        assert_eq!(key, "n");
        assert_eq!(mods, Some(vec!["cmd".to_string(), "shift".to_string()]));
    }
}
