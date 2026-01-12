use crate::types::{AtomicAction, Goal, ScreenState, VerificationResult};
use crate::vision;
use regex::Regex;
use tauri::AppHandle;
use tokio::time::{sleep, Duration};

use super::observer::Observer;

pub struct Verifier {
    observer: Observer,
}

impl Verifier {
    pub fn new() -> Self {
        Self {
            observer: Observer::new(),
        }
    }

    /// Verify if the goal was achieved after executing an action
    pub async fn verify(
        &self,
        app_handle: &AppHandle,
        goal: &Goal,
        action: &AtomicAction,
        before_screen: &ScreenState,
    ) -> Result<VerificationResult, String> {
        // Wait briefly for UI to update
        sleep(Duration::from_millis(300)).await;

        // Observe new screen state
        let after_screen = self.observer.observe(app_handle, &goal.description).await?;

        // Use vision model to verify
        let verification_response = vision::verify_goal_with_debug(
            app_handle,
            &goal.description,
            &goal.success_criteria,
            &before_screen.description,
            &after_screen.description,
        )
        .await?;

        // Parse verification response
        parse_verification_response(&verification_response, goal, action)
    }

    /// Quick verification based on screen change
    pub fn quick_verify(&self, before_hash: &str, after_hash: &str) -> bool {
        before_hash != after_hash
    }
}

/// Parse the verification response from vision model
fn parse_verification_response(
    response: &str,
    goal: &Goal,
    action: &AtomicAction,
) -> Result<VerificationResult, String> {
    let response_lower = response.to_lowercase();

    // Check for ACHIEVED/NOT_ACHIEVED
    let goal_achieved = response_lower.contains("achieved")
        && !response_lower.contains("not_achieved")
        && !response_lower.contains("not achieved");

    // Check for PROGRESS/NO_PROGRESS
    let progress_made = response_lower.contains("progress")
        && !response_lower.contains("no_progress")
        && !response_lower.contains("no progress");

    // Extract observation (last line or after "observation:")
    let observation = extract_observation(response);

    Ok(VerificationResult {
        goal_id: goal.id.clone(),
        action_id: action.id.clone(),
        goal_achieved,
        progress_made,
        observation,
    })
}

/// Extract the observation text from response
fn extract_observation(response: &str) -> String {
    // Try to find observation after common prefixes
    let observation_pattern = Regex::new(r"(?i)(?:observation[:\s]+)?(.{1,100})$").ok();

    if let Some(re) = observation_pattern {
        if let Some(caps) = re.captures(response.trim()) {
            if let Some(m) = caps.get(1) {
                return m.as_str().trim().to_string();
            }
        }
    }

    // Fallback: return last non-empty line
    response
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("No observation")
        .trim()
        .to_string()
}

impl Default for Verifier {
    fn default() -> Self {
        Self::new()
    }
}
