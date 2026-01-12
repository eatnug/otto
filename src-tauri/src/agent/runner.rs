use crate::agent::tools::{
    AgentSession, AgentState, Plan, ScrollDirection, Tool, ToolOutput, ToolResult,
    UIElement,
};
use crate::computer;
use crate::llm::call_ollama_with_debug;
use crate::screenshot;
use crate::types::{LlmCallType, MouseButton};
use crate::vision;
use serde::Serialize;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Duration};

static CANCELLED: AtomicBool = AtomicBool::new(false);

const MAX_STEPS: usize = 50;

pub fn cancel() {
    CANCELLED.store(true, Ordering::SeqCst);
}

fn is_cancelled() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

/// A single step in execution history
#[derive(Debug, Clone, Serialize)]
struct Step {
    tool: String,
    params: Option<serde_json::Value>,
    success: bool,
    output: Option<String>,
    error: Option<String>,
}

pub struct Agent {
    app_handle: AppHandle,
    session: AgentSession,
    history: Vec<Step>,
}

impl Agent {
    pub fn new(app_handle: AppHandle, task: String) -> Self {
        Self {
            app_handle,
            session: AgentSession::new(task),
            history: vec![],
        }
    }

    /// Main agent loop: Plan then Execute
    pub async fn run(&mut self) -> Result<String, String> {
        CANCELLED.store(false, Ordering::SeqCst);

        println!("\n========================================");
        println!("[AGENT] Starting task: \"{}\"", self.session.task);
        println!("========================================\n");

        // Emit session started
        self.session.state = AgentState::Planning;
        self.emit_session();

        // Phase 1: Create plan
        println!("[PHASE 1] Creating plan...");
        let plan = match self.create_plan().await {
            Ok(p) => {
                println!("[PHASE 1] Plan created with {} steps:", p.steps.len());
                for step in &p.steps {
                    println!("  {}. {}", step.id + 1, step.description);
                }
                p
            }
            Err(e) => {
                println!("[PHASE 1] Planning failed: {}", e);
                self.session.state = AgentState::Failed;
                self.session.error = Some(e.clone());
                self.emit_session();
                return Err(e);
            }
        };

        self.session.plan = Some(plan);
        self.session.state = AgentState::Executing;
        self.emit_session();

        // Phase 2: Execute each plan step
        println!("\n[PHASE 2] Executing plan steps...");

        let plan_steps: Vec<String> = self.session.plan.as_ref()
            .map(|p| p.steps.iter().map(|s| s.description.clone()).collect())
            .unwrap_or_default();

        for (step_idx, step_desc) in plan_steps.iter().enumerate() {
            if is_cancelled() {
                println!("[AGENT] Cancelled by user");
                self.session.state = AgentState::Failed;
                self.session.error = Some("Cancelled by user".into());
                self.emit_session();
                return Err("Cancelled".into());
            }

            println!("\n[STEP {}/{}] {}", step_idx + 1, plan_steps.len(), step_desc);

            // Get tools for this step
            let tools = match self.plan_step_tools(step_desc).await {
                Ok(t) => t,
                Err(e) => {
                    println!("  Failed to plan tools: {}", e);
                    continue;
                }
            };

            println!("  Tools: {:?}", tools.iter().map(tool_name).collect::<Vec<_>>());

            // Execute each tool in sequence
            for tool in tools {
                if is_cancelled() {
                    break;
                }

                println!("  [EXEC] {}", tool_name(&tool));
                let result = self.execute_tool(&tool).await;

                if !result.success {
                    println!("    Failed: {:?}", result.error);
                }

                self.session.step_count += 1;
                self.emit_session();
                sleep(Duration::from_millis(100)).await;
            }

            // Advance plan step
            if let Some(ref mut plan) = self.session.plan {
                plan.advance();
                self.emit_session();
            }
        }

        println!("\n========================================");
        println!("[AGENT] All plan steps completed!");
        println!("========================================\n");

        self.session.state = AgentState::Done;
        self.emit_session();
        let summary = format!("Completed: {}", self.session.task);
        self.emit("agent_done", &summary);
        Ok(summary)
    }

    /// Plan tools needed for a single step
    async fn plan_step_tools(&self, step_desc: &str) -> Result<Vec<Tool>, String> {
        // First, try pattern matching for common steps
        if let Some(tools) = self.match_step_pattern(step_desc) {
            return Ok(tools);
        }

        // Fall back to LLM
        let prompt = format!(
            r#"What tools are needed for this step? Output a JSON array.

TASK: {}
STEP: {}

Available tools:
- open_app: {{"name": "AppName"}}
- key: {{"key": "l", "modifiers": ["cmd"]}} or {{"key": "return"}}
- type: {{"text": "search query"}}
- wait: {{"ms": 500}}

Examples:
Step: "Open Safari" -> [{{"tool": "open_app", "params": {{"name": "Safari"}}}}, {{"tool": "wait", "params": {{"ms": 500}}}}]
Step: "Focus URL bar" -> [{{"tool": "key", "params": {{"key": "l", "modifiers": ["cmd"]}}}}]
Step: "Type hello and search" -> [{{"tool": "type", "params": {{"text": "hello"}}}}, {{"tool": "key", "params": {{"key": "return"}}}}]

Output ONLY the JSON array:
"#,
            self.session.task,
            step_desc
        );

        let response = call_ollama_with_debug(
            &self.app_handle,
            &prompt,
            LlmCallType::ActionDecision,
        )
        .await?;

        let mut tools = parse_tools_array(&response)?;

        // Post-process: ensure search steps end with Enter
        let step_lower = step_desc.to_lowercase();
        if step_lower.contains("search") || step_lower.contains("submit") || step_lower.contains("enter") {
            let has_type = tools.iter().any(|t| matches!(t, Tool::Type { .. }));
            let has_return = tools.iter().any(|t| matches!(t, Tool::Key { key, .. } if key == "return"));
            if has_type && !has_return {
                tools.push(Tool::Key { key: "return".into(), modifiers: None });
            }
        }

        Ok(tools)
    }

    /// Pattern match common step descriptions
    fn match_step_pattern(&self, step_desc: &str) -> Option<Vec<Tool>> {
        let step_lower = step_desc.to_lowercase();
        let task_lower = self.session.task.to_lowercase();

        // "Open X" pattern
        if step_lower.starts_with("open ") {
            let app_name = step_desc[5..].trim();
            // Capitalize first letter
            let app_name = app_name.chars().next()
                .map(|c| c.to_uppercase().collect::<String>() + &app_name[1..])
                .unwrap_or_else(|| app_name.to_string());
            return Some(vec![
                Tool::OpenApp { name: app_name },
                Tool::Wait { ms: 500 },
            ]);
        }

        // "Focus URL bar" pattern
        if step_lower.contains("url bar") || step_lower.contains("address bar") || step_lower.contains("focus") && step_lower.contains("cmd+l") {
            return Some(vec![
                Tool::Key { key: "l".into(), modifiers: Some(vec!["cmd".into()]) },
            ]);
        }

        // "Type X and search" pattern - extract search query from task
        if step_lower.contains("type") && (step_lower.contains("search") || step_lower.contains("enter")) {
            // Try to extract search query from task
            let query = extract_search_query(&task_lower).unwrap_or_else(|| "search".to_string());
            return Some(vec![
                Tool::Type { text: query },
                Tool::Key { key: "return".into(), modifiers: None },
            ]);
        }

        None
    }

    /// Create a high-level plan for the task
    async fn create_plan(&self) -> Result<Plan, String> {
        let prompt = format!(
            r#"Create a simple plan for this macOS task:

TASK: {}

Rules:
- 2-4 steps maximum
- Each step = one clear action
- For browser: must include "focus URL bar" step before typing

Examples:
Task: open safari and search rust
1. Open Safari
2. Focus URL bar (Cmd+L)
3. Type query and search

Task: open notes
1. Open Notes app

Output ONLY numbered steps:"#,
            self.session.task
        );

        let response = call_ollama_with_debug(
            &self.app_handle,
            &prompt,
            LlmCallType::Decomposition,
        )
        .await?;

        parse_plan(&self.session.task, &response)
    }

    /// Ask LLM to pick next tool based on current state
    async fn decide_next_tool(&self) -> Result<Tool, String> {
        let prompt = self.build_tool_prompt();

        let response = call_ollama_with_debug(
            &self.app_handle,
            &prompt,
            LlmCallType::ActionDecision,
        )
        .await?;

        parse_tool_response(&response)
    }

    /// Build prompt for tool selection
    fn build_tool_prompt(&self) -> String {
        let plan = self.session.plan.as_ref();
        let current_step = plan.and_then(|p| p.current_step_desc()).unwrap_or("Complete the task");
        let step_num = plan.map(|p| p.current_step + 1).unwrap_or(1);
        let total_steps = plan.map(|p| p.steps.len()).unwrap_or(1);

        let mut prompt = format!(
            r#"You are a macOS automation agent. Execute actions step by step.

TASK: {}
CURRENT STEP ({}/{}): {}

"#,
            self.session.task,
            step_num,
            total_steps,
            current_step,
        );

        // Add recent history (last 5 steps)
        if !self.history.is_empty() {
            prompt.push_str("DONE ACTIONS:\n");
            let start = self.history.len().saturating_sub(5);
            for (i, step) in self.history[start..].iter().enumerate() {
                let idx = start + i + 1;
                let status = if step.success { "OK" } else { "FAILED" };
                if let Some(params) = &step.params {
                    prompt.push_str(&format!("{}. {} {} -> {}\n", idx, step.tool, params, status));
                } else {
                    prompt.push_str(&format!("{}. {} -> {}\n", idx, step.tool, status));
                }
                if let Some(err) = &step.error {
                    prompt.push_str(&format!("   Error: {}\n", err));
                }
            }
            prompt.push('\n');
        }

        prompt.push_str(
            r#"TOOLS:
- open_app {"name": "Safari"}: Open an application
- key {"key": "l", "modifiers": ["cmd"]}: Press key combo (for URL bar: cmd+l)
- type {"text": "hello"}: Type text
- key {"key": "return"}: Press enter
- wait {"ms": 500}: Wait
- step_done: Mark current step DONE and move to next

CRITICAL RULES:
1. If action shows "-> OK", it WORKED. Move to NEXT action, never repeat!
2. After completing all actions for current step, use step_done
3. Browser search flow: open_app -> wait -> key cmd+l -> type -> key return -> step_done

What is the NEXT action? Output JSON:
{"tool": "...", "params": {...}}

JSON:"#,
        );

        prompt
    }

    /// Execute a tool and return result
    async fn execute_tool(&self, tool: &Tool) -> ToolResult {
        match tool {
            Tool::Screenshot => {
                println!("  [EXEC] Capturing screenshot...");
                match self.capture_screen().await {
                    Ok((elements, active_app)) => {
                        println!("  [EXEC] Found {} UI elements", elements.len());
                        ToolResult {
                            tool: "screenshot".into(),
                            success: true,
                            output: Some(ToolOutput::Screenshot {
                                elements,
                                active_app,
                            }),
                            error: None,
                        }
                    }
                    Err(e) => {
                        println!("  [EXEC] Screenshot failed: {}", e);
                        ToolResult {
                            tool: "screenshot".into(),
                            success: false,
                            output: None,
                            error: Some(e),
                        }
                    }
                }
            }

            Tool::Click { x, y } => {
                println!("  [EXEC] Click at ({}, {})", x, y);
                let result = computer::mouse_click(*x, *y, MouseButton::Left);
                ToolResult {
                    tool: "click".into(),
                    success: result.is_ok(),
                    output: result.is_ok().then_some(ToolOutput::Ack),
                    error: result.err(),
                }
            }

            Tool::DoubleClick { x, y } => {
                println!("  [EXEC] Double click at ({}, {})", x, y);
                // Two clicks with short delay
                let r1 = computer::mouse_click(*x, *y, MouseButton::Left);
                sleep(Duration::from_millis(50)).await;
                let r2 = computer::mouse_click(*x, *y, MouseButton::Left);
                let success = r1.is_ok() && r2.is_ok();
                ToolResult {
                    tool: "double_click".into(),
                    success,
                    output: success.then_some(ToolOutput::Ack),
                    error: r1.err().or(r2.err()),
                }
            }

            Tool::Type { text } => {
                println!("  [EXEC] Type: \"{}\"", text);
                let result = computer::type_text(text);
                ToolResult {
                    tool: "type".into(),
                    success: result.is_ok(),
                    output: result.is_ok().then_some(ToolOutput::Ack),
                    error: result.err(),
                }
            }

            Tool::Key { key, modifiers } => {
                let mods: Vec<&str> = modifiers
                    .as_ref()
                    .map(|m| m.iter().map(|s| s.as_str()).collect())
                    .unwrap_or_default();
                println!("  [EXEC] Key: {} {:?}", key, mods);
                let result = computer::press_key(key, &mods);
                ToolResult {
                    tool: "key".into(),
                    success: result.is_ok(),
                    output: result.is_ok().then_some(ToolOutput::Ack),
                    error: result.err(),
                }
            }

            Tool::Wait { ms } => {
                println!("  [EXEC] Wait {}ms", ms);
                sleep(Duration::from_millis(*ms)).await;
                ToolResult {
                    tool: "wait".into(),
                    success: true,
                    output: Some(ToolOutput::Ack),
                    error: None,
                }
            }

            Tool::OpenApp { name } => {
                println!("  [EXEC] Open app: {}", name);
                let result = computer::open_app(name);
                ToolResult {
                    tool: "open_app".into(),
                    success: result.is_ok(),
                    output: result.is_ok().then_some(ToolOutput::Ack),
                    error: result.err(),
                }
            }

            Tool::Scroll { direction, amount } => {
                println!("  [EXEC] Scroll {:?} by {}", direction, amount);
                // TODO: implement scroll
                ToolResult {
                    tool: "scroll".into(),
                    success: false,
                    output: None,
                    error: Some("Scroll not implemented yet".into()),
                }
            }

            // Terminal tools handled in run()
            Tool::StepDone | Tool::Done { .. } | Tool::Fail { .. } => {
                unreachable!("Terminal tools handled in main loop")
            }
        }
    }

    /// Capture screen and detect UI elements
    async fn capture_screen(&self) -> Result<(Vec<UIElement>, Option<String>), String> {
        let (bytes, scale_x, scale_y) =
            screenshot::capture_and_resize().map_err(|e| e.to_string())?;

        let detected = vision::detect_ui_elements(&self.app_handle, &bytes, "").await?;

        let elements: Vec<UIElement> = detected
            .into_iter()
            .map(|e| {
                let cx = ((e.x1 + e.x2) / 2) as f64 * scale_x;
                let cy = ((e.y1 + e.y2) / 2) as f64 * scale_y;
                UIElement {
                    label: e.label,
                    element_type: e.element_type,
                    x: cx as i32,
                    y: cy as i32,
                }
            })
            .collect();

        let active_app = get_frontmost_app();

        Ok((elements, active_app))
    }

    fn emit_session(&self) {
        let _ = self.app_handle.emit("agent_session", &self.session);
    }

    fn emit<T: Serialize>(&self, event: &str, data: &T) {
        let _ = self.app_handle.emit(event, data);
    }
}

// ==========================================
// Helper functions
// ==========================================

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

fn tool_name(tool: &Tool) -> String {
    match tool {
        Tool::Screenshot => "screenshot".into(),
        Tool::Click { .. } => "click".into(),
        Tool::DoubleClick { .. } => "double_click".into(),
        Tool::Type { .. } => "type".into(),
        Tool::Key { .. } => "key".into(),
        Tool::Wait { .. } => "wait".into(),
        Tool::OpenApp { .. } => "open_app".into(),
        Tool::Scroll { .. } => "scroll".into(),
        Tool::StepDone => "step_done".into(),
        Tool::Done { .. } => "done".into(),
        Tool::Fail { .. } => "fail".into(),
    }
}

fn tool_params(tool: &Tool) -> Option<serde_json::Value> {
    match tool {
        Tool::Screenshot => None,
        Tool::Click { x, y } => Some(serde_json::json!({"x": x, "y": y})),
        Tool::DoubleClick { x, y } => Some(serde_json::json!({"x": x, "y": y})),
        Tool::Type { text } => Some(serde_json::json!({"text": text})),
        Tool::Key { key, modifiers } => Some(serde_json::json!({"key": key, "modifiers": modifiers})),
        Tool::Wait { ms } => Some(serde_json::json!({"ms": ms})),
        Tool::OpenApp { name } => Some(serde_json::json!({"name": name})),
        Tool::Scroll { direction, amount } => {
            Some(serde_json::json!({"direction": direction, "amount": amount}))
        }
        Tool::StepDone => None,
        Tool::Done { summary } => Some(serde_json::json!({"summary": summary})),
        Tool::Fail { reason } => Some(serde_json::json!({"reason": reason})),
    }
}

fn format_output(output: &ToolOutput) -> String {
    match output {
        ToolOutput::Screenshot {
            elements,
            active_app,
        } => {
            let mut s = String::new();
            if let Some(app) = active_app {
                s.push_str(&format!("Active app: {}\n", app));
            }
            s.push_str(&format!("UI Elements ({}):\n", elements.len()));
            for el in elements.iter().take(20) {
                // Limit to 20 elements
                s.push_str(&format!(
                    "  - {} '{}' at ({}, {})\n",
                    el.element_type, el.label, el.x, el.y
                ));
            }
            if elements.len() > 20 {
                s.push_str(&format!("  ... and {} more\n", elements.len() - 20));
            }
            s
        }
        ToolOutput::Ack => "OK".into(),
    }
}

/// Extract search query from task string
fn extract_search_query(task: &str) -> Option<String> {
    // Pattern: "search X", "search for X", "search X in Y"
    let task = task.to_lowercase();

    // "open X and search Y"
    if let Some(idx) = task.find("search ") {
        let after_search = &task[idx + 7..];
        // Remove "for " if present
        let query = after_search.strip_prefix("for ").unwrap_or(after_search);
        // Remove "in X" suffix
        let query = if let Some(in_idx) = query.find(" in ") {
            &query[..in_idx]
        } else {
            query
        };
        let query = query.trim();
        if !query.is_empty() {
            return Some(query.to_string());
        }
    }

    None
}

/// Parse array of tools from LLM response
fn parse_tools_array(response: &str) -> Result<Vec<Tool>, String> {
    let response = response.trim();

    // Find JSON array in response
    let json_str = if let Some(start) = response.find('[') {
        if let Some(end) = response.rfind(']') {
            &response[start..=end]
        } else {
            return Err("No closing bracket found".into());
        }
    } else {
        return Err("No JSON array found".into());
    };

    // Parse as array of generic objects
    let arr: Vec<serde_json::Value> = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse JSON array: {}", e))?;

    let mut tools = Vec::new();
    for obj in arr {
        if let Some(tool_name) = obj.get("tool").and_then(|v| v.as_str()) {
            let params = obj.get("params").cloned();

            let tool = match tool_name {
                "open_app" => {
                    let name = params.as_ref()
                        .and_then(|p| p.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Safari")
                        .to_string();
                    Tool::OpenApp { name }
                }
                "key" => {
                    let key = params.as_ref()
                        .and_then(|p| p.get("key"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("return")
                        .to_string();
                    let modifiers = params.as_ref()
                        .and_then(|p| p.get("modifiers"))
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect());
                    Tool::Key { key, modifiers }
                }
                "type" => {
                    let text = params.as_ref()
                        .and_then(|p| p.get("text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Tool::Type { text }
                }
                "wait" => {
                    let ms = params.as_ref()
                        .and_then(|p| p.get("ms"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(500);
                    Tool::Wait { ms }
                }
                "click" => {
                    let x = params.as_ref()
                        .and_then(|p| p.get("x"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32;
                    let y = params.as_ref()
                        .and_then(|p| p.get("y"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32;
                    Tool::Click { x, y }
                }
                _ => continue,
            };
            tools.push(tool);
        }
    }

    if tools.is_empty() {
        return Err("No valid tools found in array".into());
    }

    Ok(tools)
}

/// Parse plan from LLM response
fn parse_plan(task: &str, response: &str) -> Result<Plan, String> {
    let mut steps = Vec::new();

    for line in response.lines() {
        let line = line.trim();
        // Match lines starting with a number
        if let Some(idx) = line.find('.') {
            let num_part = &line[..idx].trim();
            if num_part.chars().all(|c| c.is_ascii_digit()) {
                let description = line[idx + 1..].trim().to_string();
                if !description.is_empty() {
                    steps.push(description);
                }
            }
        }
    }

    if steps.is_empty() {
        // Fallback: treat each line as a step
        for line in response.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                steps.push(line.to_string());
            }
        }
    }

    if steps.is_empty() {
        return Err("Could not parse plan from response".into());
    }

    Ok(Plan::new(task.to_string(), steps))
}

/// Parse tool from LLM response
fn parse_tool_response(response: &str) -> Result<Tool, String> {
    let response = response.trim();

    // Try to find JSON in response
    let json_str = if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            &response[start..=end]
        } else {
            response
        }
    } else {
        response
    };

    // Try parsing as Tool directly
    if let Ok(tool) = serde_json::from_str::<Tool>(json_str) {
        return Ok(tool);
    }

    // Try parsing as generic JSON and extract tool
    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(tool_name) = obj.get("tool").and_then(|v| v.as_str()) {
            let params = obj.get("params").cloned();

            return match tool_name {
                "screenshot" => Ok(Tool::Screenshot),
                "click" => {
                    let x = params
                        .as_ref()
                        .and_then(|p| p.get("x"))
                        .and_then(|v| v.as_i64())
                        .ok_or("click requires x coordinate")? as i32;
                    let y = params
                        .as_ref()
                        .and_then(|p| p.get("y"))
                        .and_then(|v| v.as_i64())
                        .ok_or("click requires y coordinate")? as i32;
                    Ok(Tool::Click { x, y })
                }
                "double_click" => {
                    let x = params
                        .as_ref()
                        .and_then(|p| p.get("x"))
                        .and_then(|v| v.as_i64())
                        .ok_or("double_click requires x coordinate")? as i32;
                    let y = params
                        .as_ref()
                        .and_then(|p| p.get("y"))
                        .and_then(|v| v.as_i64())
                        .ok_or("double_click requires y coordinate")? as i32;
                    Ok(Tool::DoubleClick { x, y })
                }
                "type" => {
                    let text = params
                        .as_ref()
                        .and_then(|p| p.get("text"))
                        .and_then(|v| v.as_str())
                        .ok_or("type requires text")?
                        .to_string();
                    Ok(Tool::Type { text })
                }
                "key" => {
                    let key = params
                        .as_ref()
                        .and_then(|p| p.get("key"))
                        .and_then(|v| v.as_str())
                        .ok_or("key requires key name")?
                        .to_string();
                    let modifiers = params
                        .as_ref()
                        .and_then(|p| p.get("modifiers"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        });
                    Ok(Tool::Key { key, modifiers })
                }
                "wait" => {
                    let ms = params
                        .as_ref()
                        .and_then(|p| p.get("ms"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(500);
                    Ok(Tool::Wait { ms })
                }
                "open_app" => {
                    let name = params
                        .as_ref()
                        .and_then(|p| p.get("name"))
                        .and_then(|v| v.as_str())
                        .ok_or("open_app requires name")?
                        .to_string();
                    Ok(Tool::OpenApp { name })
                }
                "scroll" => {
                    let direction = params
                        .as_ref()
                        .and_then(|p| p.get("direction"))
                        .and_then(|v| v.as_str())
                        .map(|s| match s {
                            "up" => ScrollDirection::Up,
                            "down" => ScrollDirection::Down,
                            "left" => ScrollDirection::Left,
                            "right" => ScrollDirection::Right,
                            _ => ScrollDirection::Down,
                        })
                        .unwrap_or(ScrollDirection::Down);
                    let amount = params
                        .as_ref()
                        .and_then(|p| p.get("amount"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(3) as i32;
                    Ok(Tool::Scroll { direction, amount })
                }
                "step_done" => Ok(Tool::StepDone),
                "done" => {
                    let summary = params
                        .as_ref()
                        .and_then(|p| p.get("summary"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Task completed")
                        .to_string();
                    Ok(Tool::Done { summary })
                }
                "fail" => {
                    let reason = params
                        .as_ref()
                        .and_then(|p| p.get("reason"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error")
                        .to_string();
                    Ok(Tool::Fail { reason })
                }
                _ => Err(format!("Unknown tool: {}", tool_name)),
            };
        }
    }

    Err(format!(
        "Failed to parse tool from response: {}",
        response
    ))
}
