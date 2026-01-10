use crate::types::MouseButton;
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn open_app(app_name: &str) -> Result<(), String> {
    println!("[DEBUG] open_app: open -a {}", app_name);

    Command::new("open")
        .arg("-a")
        .arg(app_name)
        .output()
        .map_err(|e| format!("Failed to open app: {}", e))?;

    // Wait for app to launch
    thread::sleep(Duration::from_millis(500));

    // Activate the app to ensure it has focus
    activate_app(app_name)?;

    Ok(())
}

pub fn activate_app(app_name: &str) -> Result<(), String> {
    let script = format!(
        r#"tell application "{}" to activate"#,
        app_name
    );
    println!("[DEBUG] activate_app: osascript -e '{}'", script);

    Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to activate app: {}", e))?;

    thread::sleep(Duration::from_millis(200));
    Ok(())
}

pub fn focus_browser_url_bar(app_name: &str) -> Result<(), String> {
    // Use AppleScript to send Cmd+L to the specific app
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                keystroke "l" using command down
            end tell
        end tell"#,
        app_name
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to focus URL bar: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("AppleScript error: {}", stderr));
    }

    thread::sleep(Duration::from_millis(100));
    Ok(())
}

pub fn type_text(text: &str) -> Result<(), String> {
    // Use AppleScript for text input - sends to frontmost app
    let escaped = text.replace("\\", "\\\\").replace("\"", "\\\"");
    let script = format!(
        r#"tell application "System Events" to keystroke "{}""#,
        escaped
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to type text: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Type text failed: {}", stderr));
    }

    Ok(())
}

fn char_to_keycode(c: char) -> Option<CGKeyCode> {
    match c.to_ascii_lowercase() {
        'a' => Some(0x00), 'b' => Some(0x0B), 'c' => Some(0x08), 'd' => Some(0x02),
        'e' => Some(0x0E), 'f' => Some(0x03), 'g' => Some(0x05), 'h' => Some(0x04),
        'i' => Some(0x22), 'j' => Some(0x26), 'k' => Some(0x28), 'l' => Some(0x25),
        'm' => Some(0x2E), 'n' => Some(0x2D), 'o' => Some(0x1F), 'p' => Some(0x23),
        'q' => Some(0x0C), 'r' => Some(0x0F), 's' => Some(0x01), 't' => Some(0x11),
        'u' => Some(0x20), 'v' => Some(0x09), 'w' => Some(0x0D), 'x' => Some(0x07),
        'y' => Some(0x10), 'z' => Some(0x06),
        '0' | ')' => Some(0x1D), '1' | '!' => Some(0x12), '2' | '@' => Some(0x13),
        '3' | '#' => Some(0x14), '4' | '$' => Some(0x15), '5' | '%' => Some(0x17),
        '6' | '^' => Some(0x16), '7' | '&' => Some(0x1A), '8' | '*' => Some(0x1C),
        '9' | '(' => Some(0x19),
        ' ' => Some(0x31),
        '-' | '_' => Some(0x1B), '=' | '+' => Some(0x18),
        '[' | '{' => Some(0x21), ']' | '}' => Some(0x1E),
        '\\' | '|' => Some(0x2A), ';' | ':' => Some(0x29),
        '\'' | '"' => Some(0x27), ',' | '<' => Some(0x2B),
        '.' | '>' => Some(0x2F), '/' | '?' => Some(0x2C),
        '`' | '~' => Some(0x32),
        _ => None,
    }
}

fn key_to_keycode(key: &str) -> Option<CGKeyCode> {
    match key.to_lowercase().as_str() {
        "a" => Some(0x00),
        "s" => Some(0x01),
        "d" => Some(0x02),
        "f" => Some(0x03),
        "h" => Some(0x04),
        "g" => Some(0x05),
        "z" => Some(0x06),
        "x" => Some(0x07),
        "c" => Some(0x08),
        "v" => Some(0x09),
        "b" => Some(0x0B),
        "q" => Some(0x0C),
        "w" => Some(0x0D),
        "e" => Some(0x0E),
        "r" => Some(0x0F),
        "y" => Some(0x10),
        "t" => Some(0x11),
        "1" => Some(0x12),
        "2" => Some(0x13),
        "3" => Some(0x14),
        "4" => Some(0x15),
        "6" => Some(0x16),
        "5" => Some(0x17),
        "=" => Some(0x18),
        "9" => Some(0x19),
        "7" => Some(0x1A),
        "-" => Some(0x1B),
        "8" => Some(0x1C),
        "0" => Some(0x1D),
        "]" => Some(0x1E),
        "o" => Some(0x1F),
        "u" => Some(0x20),
        "[" => Some(0x21),
        "i" => Some(0x22),
        "p" => Some(0x23),
        "return" | "enter" => Some(0x24),
        "l" => Some(0x25),
        "j" => Some(0x26),
        "'" => Some(0x27),
        "k" => Some(0x28),
        ";" => Some(0x29),
        "\\" => Some(0x2A),
        "," => Some(0x2B),
        "/" => Some(0x2C),
        "n" => Some(0x2D),
        "m" => Some(0x2E),
        "." => Some(0x2F),
        "tab" => Some(0x30),
        "space" => Some(0x31),
        "`" => Some(0x32),
        "delete" | "backspace" => Some(0x33),
        "escape" | "esc" => Some(0x35),
        "f17" => Some(0x40),
        "f18" => Some(0x4F),
        "f19" => Some(0x50),
        "f20" => Some(0x5A),
        "f5" => Some(0x60),
        "f6" => Some(0x61),
        "f7" => Some(0x62),
        "f3" => Some(0x63),
        "f8" => Some(0x64),
        "f9" => Some(0x65),
        "f11" => Some(0x67),
        "f13" => Some(0x69),
        "f16" => Some(0x6A),
        "f14" => Some(0x6B),
        "f10" => Some(0x6D),
        "f12" => Some(0x6F),
        "f15" => Some(0x71),
        "home" => Some(0x73),
        "pageup" => Some(0x74),
        "forwarddelete" => Some(0x75),
        "f4" => Some(0x76),
        "end" => Some(0x77),
        "f2" => Some(0x78),
        "pagedown" => Some(0x79),
        "f1" => Some(0x7A),
        "left" | "leftarrow" => Some(0x7B),
        "right" | "rightarrow" => Some(0x7C),
        "down" | "downarrow" => Some(0x7D),
        "up" | "uparrow" => Some(0x7E),
        _ => None,
    }
}

pub fn press_key(key: &str, modifiers: &[&str]) -> Result<(), String> {
    // Use AppleScript to send key to frontmost application
    let modifier_str = modifiers
        .iter()
        .map(|m| match m.to_lowercase().as_str() {
            "cmd" | "command" => "command down",
            "shift" => "shift down",
            "alt" | "option" => "option down",
            "ctrl" | "control" => "control down",
            _ => "",
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");

    let script = if let Some(keycode) = special_key_code(key) {
        if modifier_str.is_empty() {
            format!(
                r#"tell application "System Events" to key code {}"#,
                keycode
            )
        } else {
            format!(
                r#"tell application "System Events" to key code {} using {{{}}}"#,
                keycode, modifier_str
            )
        }
    } else {
        // Single character key
        if modifier_str.is_empty() {
            format!(
                r#"tell application "System Events" to keystroke "{}""#,
                key
            )
        } else {
            format!(
                r#"tell application "System Events" to keystroke "{}" using {{{}}}"#,
                key, modifier_str
            )
        }
    };

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to press key: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Key press failed: {}", stderr));
    }

    Ok(())
}

fn special_key_code(key: &str) -> Option<u8> {
    match key.to_lowercase().as_str() {
        "return" | "enter" => Some(36),
        "tab" => Some(48),
        "space" => Some(49),
        "delete" | "backspace" => Some(51),
        "escape" | "esc" => Some(53),
        "left" | "leftarrow" => Some(123),
        "right" | "rightarrow" => Some(124),
        "down" | "downarrow" => Some(125),
        "up" | "uparrow" => Some(126),
        _ => None,
    }
}

pub fn mouse_click(x: i32, y: i32, button: MouseButton) -> Result<(), String> {
    let point = CGPoint::new(x as f64, y as f64);

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Failed to create event source")?;

    let (button_type, down_event_type, up_event_type) = match button {
        MouseButton::Left => (
            CGMouseButton::Left,
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
        ),
        MouseButton::Right => (
            CGMouseButton::Right,
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
        ),
    };

    let event_down = CGEvent::new_mouse_event(source.clone(), down_event_type, point, button_type)
        .map_err(|_| "Failed to create mouse down event")?;
    event_down.post(CGEventTapLocation::HID);

    thread::sleep(Duration::from_millis(50));

    let event_up = CGEvent::new_mouse_event(source, up_event_type, point, button_type)
        .map_err(|_| "Failed to create mouse up event")?;
    event_up.post(CGEventTapLocation::HID);

    Ok(())
}

pub fn mouse_move(x: i32, y: i32) -> Result<(), String> {
    let point = CGPoint::new(x as f64, y as f64);

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Failed to create event source")?;

    let event = CGEvent::new_mouse_event(source, CGEventType::MouseMoved, point, CGMouseButton::Left)
        .map_err(|_| "Failed to create mouse move event")?;
    event.post(CGEventTapLocation::HID);

    Ok(())
}
