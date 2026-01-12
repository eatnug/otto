use serde::{Deserialize, Serialize};

// ==========================================
// Tools - All available tools for the agent
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "params", rename_all = "snake_case")]
pub enum Tool {
    // === Perception ===
    /// Capture screenshot and detect UI elements
    Screenshot,

    // === Basic Actions ===
    /// Click at screen coordinates
    Click { x: i32, y: i32 },

    /// Double click at screen coordinates
    DoubleClick { x: i32, y: i32 },

    /// Type text
    Type { text: String },

    /// Press key with optional modifiers
    Key {
        key: String,
        modifiers: Option<Vec<String>>,
    },

    /// Wait for UI to settle
    Wait { ms: u64 },

    // === High-level Actions ===
    /// Open an application by name
    OpenApp { name: String },

    /// Scroll in a direction
    Scroll { direction: ScrollDirection, amount: i32 },

    // === Control Flow ===
    /// Current plan step completed, move to next
    StepDone,

    /// Task completed successfully
    Done { summary: String },

    /// Cannot complete task
    Fail { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

// ==========================================
// Tool Results
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: String,
    pub success: bool,
    pub output: Option<ToolOutput>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolOutput {
    /// Screenshot result with detected UI elements
    Screenshot {
        elements: Vec<UIElement>,
        active_app: Option<String>,
    },
    /// Simple acknowledgment
    Ack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIElement {
    pub label: String,
    pub element_type: String,
    pub x: i32, // center x
    pub y: i32, // center y
}

// ==========================================
// Plan
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub task: String,
    pub steps: Vec<PlanStep>,
    pub current_step: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: usize,
    pub description: String,
    pub status: StepStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    InProgress,
    Done,
    Failed,
}

impl Plan {
    pub fn new(task: String, step_descriptions: Vec<String>) -> Self {
        let steps = step_descriptions
            .into_iter()
            .enumerate()
            .map(|(i, desc)| PlanStep {
                id: i,
                description: desc,
                status: if i == 0 {
                    StepStatus::InProgress
                } else {
                    StepStatus::Pending
                },
            })
            .collect();

        Self {
            task,
            steps,
            current_step: 0,
        }
    }

    pub fn current_step_desc(&self) -> Option<&str> {
        self.steps.get(self.current_step).map(|s| s.description.as_str())
    }

    pub fn advance(&mut self) -> bool {
        if let Some(step) = self.steps.get_mut(self.current_step) {
            step.status = StepStatus::Done;
        }

        self.current_step += 1;

        if self.current_step < self.steps.len() {
            if let Some(step) = self.steps.get_mut(self.current_step) {
                step.status = StepStatus::InProgress;
            }
            true
        } else {
            false
        }
    }

    pub fn is_complete(&self) -> bool {
        self.current_step >= self.steps.len()
    }

    pub fn format(&self) -> String {
        self.steps
            .iter()
            .map(|s| {
                let marker = match s.status {
                    StepStatus::Done => "[x]",
                    StepStatus::InProgress => "[>]",
                    StepStatus::Failed => "[!]",
                    StepStatus::Pending => "[ ]",
                };
                format!("{} {}. {}", marker, s.id + 1, s.description)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ==========================================
// Agent Session (for frontend)
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub task: String,
    pub state: AgentState,
    pub plan: Option<Plan>,
    pub step_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Idle,
    Planning,
    Executing,
    Done,
    Failed,
}

impl AgentSession {
    pub fn new(task: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            task,
            state: AgentState::Idle,
            plan: None,
            step_count: 0,
            error: None,
        }
    }
}
