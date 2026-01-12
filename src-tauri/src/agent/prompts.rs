/// Centralized prompts for small LLMs (qwen2.5:0.5b, moondream)
/// All prompts are designed to be short, use examples, and expect short outputs.

/// Prompt for decomposing a user command into goals
pub fn decomposition_prompt(command: &str) -> String {
    format!(
        r#"Break command into goals. FORMAT: "N. what to do | how to know it worked"

Command: open safari
Goals:
1. Open Safari browser | Safari window is visible

Command: open chrome and search for rust
Goals:
1. Open Chrome browser | Chrome window is visible
2. Focus URL bar | Cursor is in URL bar
3. Type "rust" | Text appears in URL bar
4. Press Enter | Search results page loads

Command: open finder and search for notes
Goals:
1. Open Finder | Finder window is visible
2. Open search (CMD+F) | Search field is active
3. Type "notes" | Text appears in search
4. Press Enter | Search results shown

Command: {}
Goals:
"#,
        command
    )
}

/// Prompt for the vision model to describe the current screen state
pub fn screen_description_prompt(goal_context: &str) -> String {
    format!(
        r#"Look at this screen. Describe in 2-3 sentences:
1. What app/window is shown
2. Notable UI elements (buttons, text fields, menus)
3. What could be clicked or typed

Goal context: {}

Description:"#,
        goal_context
    )
}

/// Prompt for deciding the next atomic action
pub fn action_decision_prompt(
    goal: &str,
    _success_criteria: &str,
    screen_description: &str,
    _recent_actions: &str,
) -> String {
    format!(
        r#"Pick ONE action. Output ONLY the action, nothing else.

Goal: {}

Actions:
open APP, click X Y, type "TEXT", key KEY, key MOD+KEY, wait MS

Rules:
- "Type search query: X" -> type "X" (extract X, use exact text)
- "Focus URL bar" -> key CMD+L
- "Execute search" or "Submit" -> key return
- "Open X" -> open X

Examples:
Goal: Open Safari -> open Safari
Goal: Focus URL bar -> key CMD+L
Goal: Type search query: rust -> type "rust"
Goal: Type search query: hello world -> type "hello world"
Goal: Execute search -> key return
Goal: Submit search -> key return

Goal: {} ->"#,
        goal, goal
    )
}

/// Prompt for verifying if a goal was achieved
pub fn verification_prompt(
    goal: &str,
    success_criteria: &str,
    before_description: &str,
    after_description: &str,
) -> String {
    format!(
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
    )
}

/// Prompt for finding an element on screen
pub fn find_element_prompt(element_description: &str) -> String {
    format!(
        r#"Find the {} on this screen.
Output ONLY x,y coordinates of its center.
Example: 640, 360

Coordinates:"#,
        element_description
    )
}
