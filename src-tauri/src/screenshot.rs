use core_foundation::base::{CFRelease, TCFType};
use core_foundation::string::CFString;
use core_graphics::display::{
    kCGNullWindowID, kCGWindowImageDefault, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly, CGDisplay, CGWindowListCopyWindowInfo,
};
use core_graphics::window::{
    kCGWindowBounds, kCGWindowLayer, kCGWindowName, kCGWindowNumber, kCGWindowOwnerName,
};
use image::GenericImageView;
use std::io::Cursor;
use std::os::raw::c_void;

const MAX_WIDTH: u32 = 1280;
const MAX_HEIGHT: u32 = 720;

// FFI bindings for CGImage functions
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGImageGetWidth(image: *const c_void) -> usize;
    fn CGImageGetHeight(image: *const c_void) -> usize;
    fn CGImageGetBytesPerRow(image: *const c_void) -> usize;
    fn CGImageGetDataProvider(image: *const c_void) -> *const c_void;
    fn CGDataProviderCopyData(provider: *const c_void) -> *const c_void;
    fn CGImageRelease(image: *const c_void);
    fn CGWindowListCreateImage(
        screen_bounds: CGRect,
        list_option: u32,
        window_id: u32,
        image_option: u32,
    ) -> *const c_void;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

/// Information about a window on screen
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u32,
    pub name: String,
    pub owner: String,
    pub bounds: (f64, f64, f64, f64), // x, y, width, height
    pub layer: i32,
}

/// Get list of all visible windows on screen
pub fn get_visible_windows() -> Vec<WindowInfo> {
    let mut windows = Vec::new();

    unsafe {
        let window_list = CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        );

        if window_list.is_null() {
            return windows;
        }

        let count = core_foundation::array::CFArrayGetCount(window_list as _);

        for i in 0..count {
            let window_dict = core_foundation::array::CFArrayGetValueAtIndex(window_list as _, i)
                as core_foundation::dictionary::CFDictionaryRef;

            if window_dict.is_null() {
                continue;
            }

            // Get window ID
            let window_id = get_dict_number(window_dict, kCGWindowNumber) as u32;

            // Get window layer
            let layer = get_dict_number(window_dict, kCGWindowLayer) as i32;

            // Get window name
            let name = get_dict_string(window_dict, kCGWindowName);

            // Get owner name (app name)
            let owner = get_dict_string(window_dict, kCGWindowOwnerName);

            // Get bounds
            let bounds = get_dict_bounds(window_dict);

            // Skip very small windows or windows with no size
            if bounds.2 < 10.0 || bounds.3 < 10.0 {
                continue;
            }

            windows.push(WindowInfo {
                id: window_id,
                name,
                owner,
                bounds,
                layer,
            });
        }

        CFRelease(window_list as _);
    }

    windows
}

/// Find Otto window ID
pub fn find_otto_window_id() -> Option<u32> {
    let windows = get_visible_windows();
    windows
        .iter()
        .find(|w| w.owner == "Otto" || w.name == "Otto")
        .map(|w| w.id)
}

/// Capture screen excluding Otto window using CGWindowListCreateImage
pub fn capture_screen_excluding_otto() -> Result<Vec<u8>, String> {
    // Find Otto window to exclude
    let otto_window_id = find_otto_window_id().unwrap_or(0);

    unsafe {
        let display_bounds = CGDisplay::main().bounds();
        let rect = CGRect {
            origin: CGPoint {
                x: display_bounds.origin.x,
                y: display_bounds.origin.y,
            },
            size: CGSize {
                width: display_bounds.size.width,
                height: display_bounds.size.height,
            },
        };

        // Create image excluding the Otto window
        let image = if otto_window_id > 0 {
            println!("[SCREENSHOT] Excluding Otto window ID: {}", otto_window_id);
            // kCGWindowListOptionOnScreenBelowWindow = 1 << 1 = 2
            CGWindowListCreateImage(rect, 2 | 16, otto_window_id, kCGWindowImageDefault)
        } else {
            println!("[SCREENSHOT] No Otto window found, capturing full screen");
            CGWindowListCreateImage(
                rect,
                kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
                kCGNullWindowID,
                kCGWindowImageDefault,
            )
        };

        if image.is_null() {
            return Err("Failed to capture screen".to_string());
        }

        let result = cgimage_to_png(image);
        CGImageRelease(image);
        result
    }
}

/// Capture a specific region of the screen
pub fn capture_region(x1: i32, y1: i32, x2: i32, y2: i32) -> Result<Vec<u8>, String> {
    let x = x1.min(x2) as f64;
    let y = y1.min(y2) as f64;
    let width = (x1 - x2).abs() as f64;
    let height = (y1 - y2).abs() as f64;

    unsafe {
        let rect = CGRect {
            origin: CGPoint { x, y },
            size: CGSize { width, height },
        };

        let image = CGWindowListCreateImage(
            rect,
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
            kCGWindowImageDefault,
        );

        if image.is_null() {
            return Err("Failed to capture region".to_string());
        }

        let result = cgimage_to_png(image);
        CGImageRelease(image);
        result
    }
}

/// Convert CGImage to PNG bytes
unsafe fn cgimage_to_png(image: *const c_void) -> Result<Vec<u8>, String> {
    let width = CGImageGetWidth(image);
    let height = CGImageGetHeight(image);
    let bytes_per_row = CGImageGetBytesPerRow(image);
    let data_provider = CGImageGetDataProvider(image);

    if data_provider.is_null() {
        return Err("Failed to get image data provider".to_string());
    }

    let data = CGDataProviderCopyData(data_provider);
    if data.is_null() {
        return Err("Failed to copy image data".to_string());
    }

    let ptr = core_foundation::data::CFDataGetBytePtr(data as _);
    let len = core_foundation::data::CFDataGetLength(data as _) as usize;
    let raw_data = std::slice::from_raw_parts(ptr, len);

    // Convert BGRA to RGBA
    let mut rgba_data = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let offset = y * bytes_per_row + x * 4;
            if offset + 3 < len {
                rgba_data.push(raw_data[offset + 2]); // R
                rgba_data.push(raw_data[offset + 1]); // G
                rgba_data.push(raw_data[offset + 0]); // B
                rgba_data.push(raw_data[offset + 3]); // A
            }
        }
    }

    CFRelease(data as _);

    // Create image and encode to PNG
    let img = image::RgbaImage::from_raw(width as u32, height as u32, rgba_data)
        .ok_or("Failed to create image from raw data")?;

    let mut buffer = Cursor::new(Vec::new());
    img.write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(buffer.into_inner())
}

/// Capture and resize screenshot for faster vision model processing
/// Returns (resized_bytes, scale_factor_x, scale_factor_y)
pub fn capture_and_resize() -> Result<(Vec<u8>, f64, f64), String> {
    let original_bytes = capture_screen_excluding_otto()?;

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
    resized
        .write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode resized image: {}", e))?;

    // Return the bytes and scale factors to convert coordinates back to original
    let scale_factor_x = orig_width as f64 / new_width as f64;
    let scale_factor_y = orig_height as f64 / new_height as f64;

    Ok((buffer.into_inner(), scale_factor_x, scale_factor_y))
}

// Helper functions for dictionary access
unsafe fn get_dict_number(
    dict: core_foundation::dictionary::CFDictionaryRef,
    key: core_foundation::string::CFStringRef,
) -> i64 {
    let mut value: *const c_void = std::ptr::null();
    if core_foundation::dictionary::CFDictionaryGetValueIfPresent(dict, key as _, &mut value) != 0 {
        let num = value as core_foundation::number::CFNumberRef;
        let mut result: i64 = 0;
        core_foundation::number::CFNumberGetValue(
            num,
            core_foundation::number::kCFNumberSInt64Type,
            &mut result as *mut _ as *mut c_void,
        );
        result
    } else {
        0
    }
}

unsafe fn get_dict_string(
    dict: core_foundation::dictionary::CFDictionaryRef,
    key: core_foundation::string::CFStringRef,
) -> String {
    let mut value: *const c_void = std::ptr::null();
    if core_foundation::dictionary::CFDictionaryGetValueIfPresent(dict, key as _, &mut value) != 0 {
        let cf_str = value as core_foundation::string::CFStringRef;
        let c_str = core_foundation::string::CFStringGetCStringPtr(
            cf_str,
            core_foundation::string::kCFStringEncodingUTF8,
        );
        if !c_str.is_null() {
            return std::ffi::CStr::from_ptr(c_str)
                .to_string_lossy()
                .to_string();
        }

        // Fallback: copy string
        let len = core_foundation::string::CFStringGetLength(cf_str);
        let mut buf = vec![0u8; (len * 4 + 1) as usize];
        if core_foundation::string::CFStringGetCString(
            cf_str,
            buf.as_mut_ptr() as *mut i8,
            buf.len() as isize,
            core_foundation::string::kCFStringEncodingUTF8,
        ) != 0 {
            return std::ffi::CStr::from_ptr(buf.as_ptr() as *const i8)
                .to_string_lossy()
                .to_string();
        }
    }
    String::new()
}

unsafe fn get_dict_bounds(dict: core_foundation::dictionary::CFDictionaryRef) -> (f64, f64, f64, f64) {
    let mut value: *const c_void = std::ptr::null();
    if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
        dict,
        kCGWindowBounds as _,
        &mut value,
    ) != 0 {
        let bounds_dict = value as core_foundation::dictionary::CFDictionaryRef;

        let x_key = CFString::new("X");
        let y_key = CFString::new("Y");
        let w_key = CFString::new("Width");
        let h_key = CFString::new("Height");

        let x = get_dict_float(bounds_dict, x_key.as_concrete_TypeRef());
        let y = get_dict_float(bounds_dict, y_key.as_concrete_TypeRef());
        let w = get_dict_float(bounds_dict, w_key.as_concrete_TypeRef());
        let h = get_dict_float(bounds_dict, h_key.as_concrete_TypeRef());

        return (x, y, w, h);
    }
    (0.0, 0.0, 0.0, 0.0)
}

unsafe fn get_dict_float(
    dict: core_foundation::dictionary::CFDictionaryRef,
    key: core_foundation::string::CFStringRef,
) -> f64 {
    let mut value: *const c_void = std::ptr::null();
    if core_foundation::dictionary::CFDictionaryGetValueIfPresent(dict, key as _, &mut value) != 0 {
        let num = value as core_foundation::number::CFNumberRef;
        let mut result: f64 = 0.0;
        core_foundation::number::CFNumberGetValue(
            num,
            core_foundation::number::kCFNumberFloat64Type,
            &mut result as *mut _ as *mut c_void,
        );
        result
    } else {
        0.0
    }
}
