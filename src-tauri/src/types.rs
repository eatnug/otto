use serde::{Deserialize, Serialize};

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
