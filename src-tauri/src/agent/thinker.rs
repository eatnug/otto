use crate::computer;
use crate::llm;
use crate::types::{ActionParams, ActionResult, ActionType, AtomicAction, Goal, ScreenState};
use regex::Regex;

pub struct Thinker;

impl Thinker {
    pub fn new() -> Self {
        Self
    }

    /// Decide the next atomic action based on goal and current screen state
    pub async fn decide_action(
        &self,
        goal: &Goal,
        screen: &ScreenState,
        history: &[ActionResult],
    ) -> Result<AtomicAction, String> {
        println!("[THINKER] Deciding action for goal: \"{}\"", goal.description);
        println!("[THINKER] Screen context: \"{}\"", screen.description.chars().take(100).collect::<String>());

        // Build recent actions summary
        let recent_actions = format_recent_actions(history);

        let prompt = super::prompts::action_decision_prompt(
            &goal.description,
            &goal.success_criteria,
            &screen.description,
            &recent_actions,
        );

        println!("[THINKER] Calling LLM (non-deterministic)...");
        let response = llm::call_ollama_raw(&prompt).await?;
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
