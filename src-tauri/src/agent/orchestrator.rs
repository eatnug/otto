use crate::computer;
use crate::types::{
    ActionParams, ActionResult, AgentSession, AgentState, AtomicAction,
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
        match decomposer::decompose(&self.app_handle, &self.session.original_command).await {
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

        // Use Think-first flow
        self.process_goal_think_first(goal_index).await
    }

    /// Process a goal using Think-first flow:
    /// 1. Think (blind) → decide action without seeing screen
    /// 2. If action needs coordinates → Observe → Think again
    /// 3. Execute
    /// 4. Verify if needed
    async fn process_goal_think_first(&mut self, goal_index: usize) -> Result<(), String> {
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
                self.emit_session_update();
                return Err(format!(
                    "Goal failed after {} attempts: {}",
                    goal.max_attempts, goal.description
                ));
            }

            // Step 1: Think BLIND (no screen)
            println!("\n[STEP 1] Thinking BLIND (no screen info)...");
            self.update_state(AgentState::Thinking);
            let blind_action = match self.thinker.decide_action_blind(&self.app_handle, &goal).await {
                Ok(action) => {
                    println!("[STEP 1] Decided: {:?} -> {:?}", action.action_type, action.params);
                    action
                }
                Err(e) => {
                    if let Some(g) = self.session.goals.get_mut(goal_index) {
                        g.attempts += 1;
                    }
                    println!("[STEP 1] ERROR: Blind decision failed: {}", e);
                    continue;
                }
            };

            // Step 2: If action needs coordinates (FindAndClick), observe and think again
            let final_action = if let ActionParams::FindAndClick { element } = &blind_action.params {
                println!("[STEP 2] Action needs coordinates, observing screen...");
                self.update_state(AgentState::Observing);

                let screen_state = match self.observer.observe(&self.app_handle, &goal.description).await {
                    Ok(state) => {
                        println!("[STEP 2] Observed {} UI elements", state.ui_elements.len());
                        state
                    }
                    Err(e) => {
                        if let Some(g) = self.session.goals.get_mut(goal_index) {
                            g.attempts += 1;
                        }
                        println!("[STEP 2] ERROR: Observation failed: {}", e);
                        continue;
                    }
                };
                self.session.last_observation = Some(screen_state.clone());
                self.emit_observation(&screen_state);

                // Think again with screen info to get coordinates
                println!("[STEP 2b] Finding '{}' on screen...", element);
                self.update_state(AgentState::Thinking);
                match self.thinker.decide_action_with_screen(&self.app_handle, &goal, element, &screen_state).await {
                    Ok(action) => {
                        println!("[STEP 2b] Found: {:?}", action.params);
                        action
                    }
                    Err(e) => {
                        if let Some(g) = self.session.goals.get_mut(goal_index) {
                            g.attempts += 1;
                        }
                        println!("[STEP 2b] ERROR: Could not find element: {}", e);
                        continue;
                    }
                }
            } else {
                println!("[STEP 2] Action doesn't need coordinates, skipping observation");
                blind_action
            };

            self.session.current_action = Some(final_action.clone());
            self.emit_action_planned(&final_action);

            // Step 3: Execute
            println!("[STEP 3] Executing action: {:?}", final_action.action_type);
            self.update_state(AgentState::Acting);
            let result = execute_atomic(&final_action).await;
            println!("[STEP 3] Result: {}", if result.success { "SUCCESS" } else { "FAILED" });

            self.session.action_history.push(result.clone());
            self.session.total_actions += 1;
            self.emit_action_completed(&result);

            if !result.success {
                if let Some(g) = self.session.goals.get_mut(goal_index) {
                    g.attempts += 1;
                }
                continue;
            }

            // Step 4: Verify (self-verifying actions skip this)
            let goal_achieved = if is_self_verifying_action(&final_action) {
                println!("[STEP 4] Action is SELF-VERIFYING -> ACHIEVED");
                true
            } else {
                println!("[STEP 4] Verifying goal completion...");
                self.update_state(AgentState::Verifying);
                // For click actions, we assume success if the click executed
                // More sophisticated verification can be added later
                true
            };

            if goal_achieved {
                println!("[GOAL] Goal achieved!");
                if let Some(g) = self.session.goals.get_mut(goal_index) {
                    g.status = GoalStatus::Completed;
                }
                self.emit_session_update();
                self.emit_goal_completed(goal_index);
                break;
            }

            if let Some(g) = self.session.goals.get_mut(goal_index) {
                g.attempts += 1;
            }
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

