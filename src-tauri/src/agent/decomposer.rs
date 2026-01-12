use crate::llm;
use crate::types::{Goal, DecompositionInfo, LlmCallType};
use regex::Regex;
use tauri::AppHandle;

/// Result of decomposition including method info
pub struct DecomposeResult {
    pub goals: Vec<Goal>,
    pub info: DecompositionInfo,
}

/// Decompose a user command into a list of goals
pub async fn decompose(app_handle: &AppHandle, command: &str) -> Result<DecomposeResult, String> {
    println!("[DECOMPOSER] Input command: \"{}\"", command);

    // Try pattern matching first (fast path, but still LLM-driven execution)
    if let Some((goals, pattern_name)) = try_pattern_match(command) {
        println!("[DECOMPOSER] Used PATTERN MATCHING: {}", pattern_name);
        for (i, goal) in goals.iter().enumerate() {
            println!("[DECOMPOSER]   Goal {}: \"{}\"", i + 1, goal.description);
        }
        return Ok(DecomposeResult {
            goals,
            info: DecompositionInfo {
                method: "pattern".to_string(),
                pattern_name: Some(pattern_name),
                original_command: command.to_string(),
            },
        });
    }

    println!("[DECOMPOSER] No pattern matched, using LLM for decomposition");
    // Fall back to LLM for complex commands
    let goals = decompose_with_llm(app_handle, command).await?;
    Ok(DecomposeResult {
        goals,
        info: DecompositionInfo {
            method: "llm".to_string(),
            pattern_name: None,
            original_command: command.to_string(),
        },
    })
}

/// Try to match common command patterns directly
/// Returns (goals, pattern_name) if matched
fn try_pattern_match(command: &str) -> Option<(Vec<Goal>, String)> {
    let cmd = command.to_lowercase();

    // Pattern: "open X and search Y" / "open X and search for Y"
    let search_pattern = Regex::new(r"(?i)open\s+(\w+)\s+and\s+search\s+(?:for\s+)?(.+)").ok()?;
    if let Some(caps) = search_pattern.captures(&cmd) {
        let app = caps.get(1)?.as_str().trim();
        let query = caps.get(2)?.as_str().trim();
        return Some((build_search_goals(app, query), "open_and_search".to_string()));
    }

    // Pattern: "search X in Y"
    let search_in_pattern = Regex::new(r"(?i)search\s+(.+?)\s+in\s+(\w+)").ok()?;
    if let Some(caps) = search_in_pattern.captures(&cmd) {
        let query = caps.get(1)?.as_str().trim();
        let app = caps.get(2)?.as_str().trim();
        return Some((build_search_goals(app, query), "search_in".to_string()));
    }

    // Pattern: "open X" (simple app open)
    let open_pattern = Regex::new(r"(?i)^open\s+(\w+)$").ok()?;
    if let Some(caps) = open_pattern.captures(&cmd) {
        let app = caps.get(1)?.as_str().trim();
        let app_name = capitalize(app);
        return Some((vec![Goal::new(
            format!("Open {}", app_name),
            format!("{} window is visible and focused", app_name),
        )], "open_app".to_string()));
    }

    // Pattern: "click X" / "click on X" / "click the X"
    let click_pattern = Regex::new(r"(?i)click\s+(?:on\s+)?(?:the\s+)?(.+)").ok()?;
    if let Some(caps) = click_pattern.captures(&cmd) {
        let element = caps.get(1)?.as_str().trim();
        return Some((vec![Goal::new(
            format!("Click on {}", element),
            format!("{} has been clicked and responded", element),
        )], "click".to_string()));
    }

    // Pattern: "type X"
    let type_pattern = Regex::new(r"(?i)^type\s+(.+)$").ok()?;
    if let Some(caps) = type_pattern.captures(&cmd) {
        let text = caps.get(1)?.as_str().trim();
        return Some((vec![Goal::new(
            format!("Type: {}", text),
            format!("\"{}\" has been typed", text),
        )], "type".to_string()));
    }

    None
}

/// Build goals for search workflow (browser or finder)
fn build_search_goals(app: &str, query: &str) -> Vec<Goal> {
    let app_name = capitalize(app);
    vec![
        Goal::new(
            format!("Open {}", app_name),
            format!("{} window is visible and focused", app_name),
        ),
        Goal::new(
            "Focus URL bar".to_string(),
            "Cursor is in the URL/search input field".to_string(),
        ),
        Goal::new(
            format!("Type search query: {}", query),
            format!("\"{}\" appears in the search field", query),
        ),
        Goal::new(
            "Execute search".to_string(),
            "Search results are displayed".to_string(),
        ),
    ]
}

/// Use LLM to decompose complex commands
async fn decompose_with_llm(app_handle: &AppHandle, command: &str) -> Result<Vec<Goal>, String> {
    let prompt = super::prompts::decomposition_prompt(command);
    let response = llm::call_ollama_with_debug(app_handle, &prompt, LlmCallType::Decomposition).await?;
    println!("[DECOMPOSER] LLM response:\n{}", response);
    parse_goals_from_response(&response)
}

/// Parse goals from LLM response
fn parse_goals_from_response(response: &str) -> Result<Vec<Goal>, String> {
    let mut goals = Vec::new();

    // Primary pattern: "N. description | success_criteria"
    let pipe_pattern = Regex::new(r"^\d+\.\s*(.+?)\s*\|\s*(.+)$")
        .map_err(|e| format!("Regex error: {}", e))?;

    // Fallback pattern: "N. description - success_criteria" (common LLM mistake)
    let dash_pattern = Regex::new(r"^\d+\.\s*(.+?)\s+-\s+(.+)$")
        .map_err(|e| format!("Regex error: {}", e))?;

    // Simple pattern: "N. description" (generate default success criteria)
    let simple_pattern = Regex::new(r"^\d+\.\s*(.+)$")
        .map_err(|e| format!("Regex error: {}", e))?;

    for line in response.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try pipe separator first (preferred)
        if let Some(caps) = pipe_pattern.captures(line) {
            let description = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            let success_criteria = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");

            if !description.is_empty() && !success_criteria.is_empty() {
                goals.push(Goal::new(
                    description.to_string(),
                    success_criteria.to_string(),
                ));
                continue;
            }
        }

        // Try dash separator (fallback)
        if let Some(caps) = dash_pattern.captures(line) {
            let description = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            let success_criteria = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");

            if !description.is_empty() && !success_criteria.is_empty() {
                goals.push(Goal::new(
                    description.to_string(),
                    success_criteria.to_string(),
                ));
                continue;
            }
        }

        // Try simple pattern (last resort - generate default success criteria)
        if let Some(caps) = simple_pattern.captures(line) {
            let description = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            // Skip if it looks like garbage or meta-text
            if !description.is_empty()
                && !description.to_lowercase().contains("no specific")
                && !description.to_lowercase().contains("goals listed")
            {
                let success_criteria = format!("{} is completed", description);
                goals.push(Goal::new(
                    description.to_string(),
                    success_criteria,
                ));
            }
        }
    }

    if goals.is_empty() {
        return Err(format!(
            "Could not parse goals from LLM response: {}",
            response
        ));
    }

    for (i, goal) in goals.iter().enumerate() {
        println!("[DECOMPOSER]   Goal {}: \"{}\"", i + 1, goal.description);
    }

    Ok(goals)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_pattern() {
        let goals = try_pattern_match("open safari").unwrap();
        assert_eq!(goals.len(), 1);
        assert!(goals[0].description.contains("Safari"));
    }

    #[test]
    fn test_search_pattern() {
        let goals = try_pattern_match("open chrome and search for rust").unwrap();
        assert_eq!(goals.len(), 4);
        assert!(goals[0].description.contains("Chrome"));
    }

    #[test]
    fn test_click_pattern() {
        let goals = try_pattern_match("click on the submit button").unwrap();
        assert_eq!(goals.len(), 1);
        assert!(goals[0].description.contains("submit button"));
    }
}
