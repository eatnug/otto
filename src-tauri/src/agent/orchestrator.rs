use crate::computer;
use crate::types::{
    ActionParams, ActionResult, ActionType, AgentSession, AgentState, AtomicAction,
    DecompositionInfo, GoalStatus, MouseButton, ScreenState,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Duration};

use super::decomposer;
use super::observer::Observer;
use super::thinker::Thinker;
use super::verifier::Verifier;

static CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn cancel_agent() {
    CANCELLED.store(true, Ordering::SeqCst);
}

fn reset_cancellation() {
    CANCELLED.store(false, Ordering::SeqCst);
}

fn is_cancelled() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

pub struct AgentOrchestrator {
    session: AgentSession,
    observer: Observer,
    thinker: Thinker,
    verifier: Verifier,
    app_handle: AppHandle,
}

impl AgentOrchestrator {
    pub fn new(app_handle: AppHandle, command: String) -> Self {
        Self {
            session: AgentSession::new(command),
            observer: Observer::new(),
            thinker: Thinker::new(),
            verifier: Verifier::new(),
            app_handle,
        }
    }

    /// Run the agent loop
    pub async fn run(&mut self) -> Result<(), String> {
        reset_cancellation();

        println!("\n========================================");
        println!("[AGENT] Starting agent session");
        println!("[AGENT] Command: \"{}\"", self.session.original_command);
        println!("========================================\n");

        // Emit session started
        self.emit_session_update();

        // Phase 1: Decompose command into goals
        println!("[PHASE 1] Decomposing command into goals...");
        self.update_state(AgentState::Decomposing);
        match decomposer::decompose(&self.session.original_command).await {
            Ok(result) => {
                println!("[PHASE 1] Decomposition complete: {} goals (method: {})",
                    result.goals.len(), result.info.method);
                for (i, goal) in result.goals.iter().enumerate() {
                    println!("  Goal {}: \"{}\"", i + 1, goal.description);
                    println!("         Success: \"{}\"", goal.success_criteria);
                }
                self.session.goals = result.goals;
                self.emit_session_update(); // Send goals to frontend
                self.emit_decomposition(&result.info);
                self.emit_goals_ready();
            }
            Err(e) => {
                println!("[PHASE 1] ERROR: Decomposition failed: {}", e);
                self.set_error(&e);
                return Err(e);
            }
        }

        // Phase 2: Process each goal
        println!("\n[PHASE 2] Processing goals...");
        while self.session.current_goal_index < self.session.goals.len() {
            if is_cancelled() {
                println!("[PHASE 2] Cancelled by user");
                self.set_error("Cancelled by user");
                return Err("Cancelled".to_string());
            }

            if self.session.total_actions >= self.session.max_total_actions {
                println!("[PHASE 2] Maximum actions exceeded");
                self.set_error("Maximum actions exceeded");
                return Err("Maximum actions exceeded".to_string());
            }

            let goal_num = self.session.current_goal_index + 1;
            let total_goals = self.session.goals.len();
            println!("\n----------------------------------------");
            println!("[GOAL {}/{}] Starting", goal_num, total_goals);

            // Process current goal
            let result = self.process_current_goal().await;

            match result {
                Ok(()) => {
                    println!("[GOAL {}/{}] Completed successfully", goal_num, total_goals);
                    // Move to next goal
                    self.session.current_goal_index += 1;
                }
                Err(e) => {
                    println!("[GOAL {}/{}] Failed: {}", goal_num, total_goals, e);
                    self.set_error(&e);
                    return Err(e);
                }
            }
        }

        // All goals completed
        println!("\n========================================");
        println!("[AGENT] All goals completed successfully!");
        println!("========================================\n");
        self.update_state(AgentState::Complete);
        self.emit_session_complete();

        Ok(())
    }

    /// Process a single goal using observe → think → act → verify loop
    async fn process_current_goal(&mut self) -> Result<(), String> {
        let goal_index = self.session.current_goal_index;
        let goal = self.session.goals.get(goal_index).cloned();
        let goal = match goal {
            Some(g) => g,
            None => return Err("Goal not found".to_string()),
        };

        println!("[GOAL] Description: \"{}\"", goal.description);
        println!("[GOAL] Success criteria: \"{}\"", goal.success_criteria);

        // Mark goal as in progress
        if let Some(g) = self.session.goals.get_mut(goal_index) {
            g.status = GoalStatus::InProgress;
        }
        self.emit_session_update(); // Send updated goal status to frontend
        self.emit_goal_started(goal_index);

        // All goals use observe → think → act → verify loop
        self.process_goal_with_llm(goal_index).await
    }

    /// Process a goal using LLM-based decision making
    async fn process_goal_with_llm(&mut self, goal_index: usize) -> Result<(), String> {
        // Goal processing loop
        loop {
            if is_cancelled() {
                return Err("Cancelled".to_string());
            }

            let goal = self.session.goals.get(goal_index).cloned();
            let goal = match goal {
                Some(g) => g,
                None => return Err("Goal not found".to_string()),
            };

            if goal.attempts >= goal.max_attempts {
                if let Some(g) = self.session.goals.get_mut(goal_index) {
                    g.status = GoalStatus::Failed;
                }
                self.emit_session_update(); // Send failed status to frontend
                return Err(format!(
                    "Goal failed after {} attempts: {}",
                    goal.max_attempts, goal.description
                ));
            }

            println!("\n[STEP A] Observing screen... (vision model, non-deterministic)");
            // Step A: Observe current screen
            self.update_state(AgentState::Observing);
            let screen_state = match self.observer.observe(&goal.description).await {
                Ok(state) => {
                    println!("[STEP A] Observation: \"{}\"", state.description.chars().take(80).collect::<String>());
                    state
                },
                Err(e) => {
                    // Increment attempts and retry
                    if let Some(g) = self.session.goals.get_mut(goal_index) {
                        g.attempts += 1;
                    }
                    println!("[STEP A] ERROR: Observation failed: {}", e);
                    continue;
                }
            };
            self.session.last_observation = Some(screen_state.clone());
            self.emit_observation(&screen_state);

            println!("[STEP B] Thinking about action... (LLM, non-deterministic)");
            // Step B: Think about next action
            self.update_state(AgentState::Thinking);
            let action = match self
                .thinker
                .decide_action(&goal, &screen_state, &self.session.action_history)
                .await
            {
                Ok(action) => {
                    println!("[STEP B] Decided: {:?}", action.action_type);
                    action
                },
                Err(e) => {
                    if let Some(g) = self.session.goals.get_mut(goal_index) {
                        g.attempts += 1;
                    }
                    println!("[STEP B] ERROR: Action decision failed: {}", e);
                    continue;
                }
            };
            self.session.current_action = Some(action.clone());
            self.emit_action_planned(&action);

            println!("[STEP C] Executing action... (deterministic)");
            // Step C: Execute the atomic action
            self.update_state(AgentState::Acting);
            let result = execute_atomic(&action).await;
            println!("[STEP C] Result: {}", if result.success { "SUCCESS" } else { "FAILED" });
            self.session.action_history.push(result.clone());
            self.session.total_actions += 1;
            self.emit_action_completed(&result);

            if !result.success {
                if let Some(g) = self.session.goals.get_mut(goal_index) {
                    g.attempts += 1;
                }
                continue; // Retry with new observation
            }

            // Step D: Verify goal completion
            println!("[STEP D] Verifying goal completion...");
            // For simple actions that are self-verifying, skip complex verification
            let goal_achieved = if is_self_verifying_action(&action) {
                println!("[STEP D] Action is SELF-VERIFYING (deterministic) -> ACHIEVED");
                true
            } else {
                println!("[STEP D] Using vision verification (non-deterministic)");
                self.update_state(AgentState::Verifying);
                let verification = match self.verifier.verify(&goal, &action, &screen_state).await {
                    Ok(v) => {
                        println!("[STEP D] Verification result: {}", if v.goal_achieved { "ACHIEVED" } else { "NOT_ACHIEVED" });
                        v
                    },
                    Err(e) => {
                        println!("[STEP D] ERROR: Verification failed: {}", e);
                        // Continue anyway - we'll observe again
                        continue;
                    }
                };
                self.emit_verification(&verification);
                verification.goal_achieved
            };

            if goal_achieved {
                println!("[GOAL] Goal achieved, moving to next goal");
                if let Some(g) = self.session.goals.get_mut(goal_index) {
                    g.status = GoalStatus::Completed;
                }
                self.emit_session_update(); // Send completed status to frontend
                self.emit_goal_completed(goal_index);
                break; // Move to next goal
            }

            // If we got here, goal not achieved - increment attempts
            println!("[GOAL] Goal not achieved, incrementing attempts");
            if let Some(g) = self.session.goals.get_mut(goal_index) {
                g.attempts += 1;
            }

            // Small delay before next iteration
            sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }

    // === State Management ===

    fn update_state(&mut self, state: AgentState) {
        self.session.state = state;
        self.emit_session_update();
    }

    fn set_error(&mut self, message: &str) {
        self.session.state = AgentState::Error;
        self.session.error = Some(message.to_string());
        self.emit_error(message);
    }

    // === Event Emission ===

    fn emit_session_update(&self) {
        let _ = self.app_handle.emit("agent_session", &self.session);
    }

    fn emit_decomposition(&self, info: &DecompositionInfo) {
        let _ = self.app_handle.emit("decomposition", info);
    }

    fn emit_goals_ready(&self) {
        let _ = self.app_handle.emit("goals_ready", &self.session.goals);
    }

    fn emit_goal_started(&self, index: usize) {
        let _ = self
            .app_handle
            .emit("goal_started", serde_json::json!({ "goalIndex": index }));
    }

    fn emit_goal_completed(&self, index: usize) {
        let _ = self.app_handle.emit(
            "goal_completed",
            serde_json::json!({ "goalIndex": index }),
        );
    }

    fn emit_observation(&self, screen: &ScreenState) {
        let _ = self.app_handle.emit("observation", screen);
    }

    fn emit_action_planned(&self, action: &AtomicAction) {
        let _ = self.app_handle.emit("action_planned", action);
    }

    fn emit_action_completed(&self, result: &ActionResult) {
        let _ = self.app_handle.emit("action_completed", result);
    }

    fn emit_verification(&self, result: &crate::types::VerificationResult) {
        let _ = self.app_handle.emit("verification", result);
    }

    fn emit_session_complete(&self) {
        let _ = self
            .app_handle
            .emit("session_complete", serde_json::json!({}));
    }

    fn emit_error(&self, message: &str) {
        let _ = self
            .app_handle
            .emit("agent_error", serde_json::json!({ "message": message }));
    }
}

/// Execute an atomic action
async fn execute_atomic(action: &AtomicAction) -> ActionResult {
    let success = match &action.params {
        ActionParams::OpenApp { app_name } => computer::open_app(app_name),
        ActionParams::TypeText { text } => computer::type_text(text),
        ActionParams::PressKey { key, modifiers } => {
            let mods: Vec<&str> = modifiers
                .as_ref()
                .map(|m| m.iter().map(|s| s.as_str()).collect())
                .unwrap_or_default();
            computer::press_key(key, &mods)
        }
        ActionParams::MouseClick { x, y, button } => {
            let btn = MouseButton::from(button.as_deref());
            computer::mouse_click(*x, *y, btn)
        }
        ActionParams::MouseMove { x, y } => computer::mouse_move(*x, *y),
        ActionParams::Wait { ms } => {
            sleep(Duration::from_millis(*ms)).await;
            Ok(())
        }
        ActionParams::FindAndClick { element: _ } => {
            // This shouldn't happen in the new architecture
            // The thinker should output click X Y instead
            Err("FindAndClick is not supported in agent mode".to_string())
        }
    };

    ActionResult {
        action_id: action.id.clone(),
        success: success.is_ok(),
        error_message: success.err(),
        screen_changed: true, // We assume screen changed; will verify later
    }
}

/// Check if an action is self-verifying (success means goal achieved)
/// For these actions, we skip complex vision-based verification
fn is_self_verifying_action(action: &AtomicAction) -> bool {
    matches!(
        &action.params,
        ActionParams::OpenApp { .. }
            | ActionParams::TypeText { .. }
            | ActionParams::PressKey { .. }
            | ActionParams::Wait { .. }
    )
}
