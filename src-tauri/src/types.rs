use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================
// Agent State Machine
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Idle,
    Decomposing,  // Breaking command into goals
    Observing,    // Capturing and analyzing screen
    Thinking,     // Deciding next action
    Acting,       // Executing atomic action
    Verifying,    // Checking if action succeeded
    Complete,
    Error,
}

impl Default for AgentState {
    fn default() -> Self {
        AgentState::Idle
    }
}

// ============================================
// Goal System
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// Information about how a command was decomposed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompositionInfo {
    pub method: String,           // "pattern" or "llm"
    pub pattern_name: Option<String>,  // e.g., "search", "open", "click"
    pub original_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub description: String,
    pub success_criteria: String,
    pub status: GoalStatus,
    pub attempts: u32,
    pub max_attempts: u32,
}

impl Goal {
    pub fn new(description: String, success_criteria: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            description,
            success_criteria,
            status: GoalStatus::Pending,
            attempts: 0,
            max_attempts: 5,
        }
    }
}

// ============================================
// Screen Observation
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedElement {
    pub description: String,
    pub location: Option<(i32, i32)>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenState {
    pub timestamp: u64,
    pub description: String,
    pub detected_elements: Vec<DetectedElement>,
    pub active_app: Option<String>,
    pub screenshot_hash: String,
}

impl ScreenState {
    pub fn new(description: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            timestamp,
            description,
            detected_elements: vec![],
            active_app: None,
            screenshot_hash: String::new(),
        }
    }
}

// ============================================
// Atomic Action (smallest executable unit)
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicAction {
    pub id: String,
    pub action_type: ActionType,
    pub params: ActionParams,
    pub rationale: String,
}

impl AtomicAction {
    pub fn new(action_type: ActionType, params: ActionParams, rationale: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            action_type,
            params,
            rationale,
        }
    }
}

// ============================================
// Action Results
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action_id: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub screen_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub goal_id: String,
    pub action_id: String,
    pub goal_achieved: bool,
    pub progress_made: bool,
    pub observation: String,
}

// ============================================
// Agent Session
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub original_command: String,
    pub goals: Vec<Goal>,
    pub current_goal_index: usize,
    pub state: AgentState,
    pub action_history: Vec<ActionResult>,
    pub total_actions: u32,
    pub max_total_actions: u32,
    pub current_action: Option<AtomicAction>,
    pub last_observation: Option<ScreenState>,
    pub error: Option<String>,
}

impl AgentSession {
    pub fn new(command: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            original_command: command,
            goals: vec![],
            current_goal_index: 0,
            state: AgentState::Idle,
            action_history: vec![],
            total_actions: 0,
            max_total_actions: 50,
            current_action: None,
            last_observation: None,
            error: None,
        }
    }

    pub fn current_goal(&self) -> Option<&Goal> {
        self.goals.get(self.current_goal_index)
    }

    pub fn current_goal_mut(&mut self) -> Option<&mut Goal> {
        self.goals.get_mut(self.current_goal_index)
    }
}

// ============================================
// Original Action Types (kept for compatibility)
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    OpenApp,
    TypeText,
    PressKey,
    MouseClick,
    MouseMove,
    Wait,
    FindAndClick,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionParams {
    OpenApp { app_name: String },
    TypeText { text: String },
    PressKey { key: String, modifiers: Option<Vec<String>> },
    MouseClick { x: i32, y: i32, button: Option<String> },
    MouseMove { x: i32, y: i32 },
    Wait { ms: u64 },
    FindAndClick { element: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStep {
    pub id: String,
    #[serde(rename = "type")]
    pub action_type: ActionType,
    pub description: String,
    pub params: ActionParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    pub id: String,
    pub original_command: String,
    pub steps: Vec<ActionStep>,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
}

impl Default for MouseButton {
    fn default() -> Self {
        MouseButton::Left
    }
}

impl From<Option<&str>> for MouseButton {
    fn from(s: Option<&str>) -> Self {
        match s {
            Some("right") => MouseButton::Right,
            _ => MouseButton::Left,
        }
    }
}
