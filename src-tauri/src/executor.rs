use crate::computer;
use crate::types::{ActionParams, ActionPlan, MouseButton};
use crate::vision;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{sleep, Duration};

static CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn cancel_execution() {
    CANCELLED.store(true, Ordering::SeqCst);
}

fn reset_cancellation() {
    CANCELLED.store(false, Ordering::SeqCst);
}

fn is_cancelled() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

pub async fn execute_plan(app: &AppHandle, plan: &ActionPlan) -> Result<(), String> {
    reset_cancellation();

    for (index, step) in plan.steps.iter().enumerate() {
        if is_cancelled() {
            app.emit("execution_done", serde_json::json!({
                "success": false,
                "message": "Execution cancelled"
            }))
            .map_err(|e| e.to_string())?;
            return Ok(());
        }

        // Get debug info about what will be executed
        let debug_info = get_debug_info(&step.params);

        // Emit step started with debug info
        app.emit("step_started", serde_json::json!({
            "stepIndex": index,
            "debug": debug_info
        }))
            .map_err(|e| e.to_string())?;

        // Execute the step
        let result = execute_step(&step.params).await;

        // Emit step completed
        let success = result.is_ok();
        app.emit(
            "step_completed",
            serde_json::json!({
                "stepIndex": index,
                "success": success
            }),
        )
        .map_err(|e| e.to_string())?;

        if let Err(e) = result {
            app.emit(
                "execution_done",
                serde_json::json!({
                    "success": false,
                    "message": e
                }),
            )
            .map_err(|e| e.to_string())?;
            return Err(e);
        }

        // Small delay between steps
        sleep(Duration::from_millis(100)).await;
    }

    app.emit(
        "execution_done",
        serde_json::json!({
            "success": true
        }),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn get_debug_info(params: &ActionParams) -> String {
    match params {
        ActionParams::OpenApp { app_name } => format!("open -a {}", app_name),
        ActionParams::TypeText { text } => format!("type: '{}'", text),
        ActionParams::PressKey { key, modifiers } => {
            let mods = modifiers.as_ref().map(|m| m.join("+")).unwrap_or_default();
            if mods.is_empty() {
                format!("key: {}", key)
            } else {
                format!("key: {}+{}", mods, key)
            }
        }
        ActionParams::MouseClick { x, y, button } => format!("click: ({}, {}) {:?}", x, y, button),
        ActionParams::MouseMove { x, y } => format!("move: ({}, {})", x, y),
        ActionParams::Wait { ms } => format!("wait: {}ms", ms),
        ActionParams::FindAndClick { element } => format!("find+click: {}", element),
    }
}

async fn execute_step(params: &ActionParams) -> Result<(), String> {
    match params {
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
        ActionParams::FindAndClick { element } => {
            let screen_element = vision::find_element(element).await?;
            computer::mouse_click(screen_element.x, screen_element.y, MouseButton::Left)
        }
    }
}
