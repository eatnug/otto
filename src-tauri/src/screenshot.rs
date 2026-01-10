use image::GenericImageView;
use std::io::Cursor;
use std::process::Command;
use std::path::PathBuf;

const MAX_WIDTH: u32 = 1280;
const MAX_HEIGHT: u32 = 720;

pub fn capture_screen() -> Result<PathBuf, String> {
    let temp_path = std::env::temp_dir().join("otto_screenshot.png");

    Command::new("screencapture")
        .arg("-x") // No sound
        .arg("-C") // Capture cursor
        .arg(&temp_path)
        .output()
        .map_err(|e| format!("Failed to capture screenshot: {}", e))?;

    Ok(temp_path)
}

pub fn capture_screen_bytes() -> Result<Vec<u8>, String> {
    let path = capture_screen()?;
    std::fs::read(&path).map_err(|e| format!("Failed to read screenshot: {}", e))
}

/// Capture and resize screenshot for faster vision model processing
/// Returns (resized_bytes, scale_factor_x, scale_factor_y)
pub fn capture_and_resize() -> Result<(Vec<u8>, f64, f64), String> {
    let original_bytes = capture_screen_bytes()?;

    let img = image::load_from_memory(&original_bytes)
        .map_err(|e| format!("Failed to load image: {}", e))?;

    let (orig_width, orig_height) = img.dimensions();

    // Calculate scale to fit within MAX dimensions while preserving aspect ratio
    let scale_x = MAX_WIDTH as f64 / orig_width as f64;
    let scale_y = MAX_HEIGHT as f64 / orig_height as f64;
    let scale = scale_x.min(scale_y).min(1.0); // Don't upscale

    let new_width = (orig_width as f64 * scale) as u32;
    let new_height = (orig_height as f64 * scale) as u32;

    // Resize the image
    let resized = img.resize(new_width, new_height, image::imageops::FilterType::Triangle);

    // Encode to PNG
    let mut buffer = Cursor::new(Vec::new());
    resized.write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode resized image: {}", e))?;

    // Return the bytes and scale factors to convert coordinates back to original
    let scale_factor_x = orig_width as f64 / new_width as f64;
    let scale_factor_y = orig_height as f64 / new_height as f64;

    Ok((buffer.into_inner(), scale_factor_x, scale_factor_y))
}
