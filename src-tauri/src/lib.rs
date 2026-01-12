mod agent;
mod computer;
mod executor;
mod hotkey;
mod llm;
mod screenshot;
mod types;
mod vision;
mod window;

use agent::{Agent, AgentOrchestrator};
use tauri::{AppHandle, Emitter};
use types::ActionPlan;

#[tauri::command]
async fn plan_command(app: AppHandle, command: String) -> Result<(), String> {
    match llm::generate_plan(&command).await {
        Ok(plan) => {
            app.emit("plan_ready", &plan).map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            app.emit("error", serde_json::json!({ "message": e }))
                .map_err(|e| e.to_string())?;
            Err(e)
        }
    }
}

#[tauri::command]
async fn execute_plan(app: AppHandle, plan: ActionPlan) -> Result<(), String> {
    executor::execute_plan(&app, &plan).await
}

#[tauri::command]
fn cancel_execution() {
    executor::cancel_execution();
}

#[tauri::command]
fn hide_window(app: AppHandle) -> Result<(), String> {
    window::hide_overlay(&app)
}

// === New Agent Commands ===

#[tauri::command]
async fn start_agent(app: AppHandle, command: String) -> Result<(), String> {
    let mut orchestrator = AgentOrchestrator::new(app, command);
    orchestrator.run().await
}

#[tauri::command]
fn cancel_agent() {
    agent::orchestrator::cancel_agent();
    agent::runner::cancel();
}

// === New Tool-based Agent ===

#[tauri::command]
async fn start_agent_v2(app: AppHandle, command: String) -> Result<(), String> {
    let mut agent = Agent::new(app, command);
    agent.run().await.map(|_| ())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            hotkey::register_global_hotkey(app.handle())?;
            // Set up window to float above other apps (like Spotlight)
            if let Err(e) = window::setup_floating_window(app.handle()) {
                eprintln!("[SETUP] Warning: Failed to setup floating window: {}", e);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            plan_command,
            execute_plan,
            cancel_execution,
            hide_window,
            start_agent,
            cancel_agent,
            start_agent_v2
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
