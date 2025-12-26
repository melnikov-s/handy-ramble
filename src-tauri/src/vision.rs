use base64::{engine::general_purpose, Engine as _};
use log::debug;
use std::io::Cursor;
use xcap::Monitor;

/// Captures the main screen and returns a Base64-encoded PNG string.
pub fn capture_screen() -> Result<String, String> {
    debug!("Starting screen capture...");

    // Get all monitors
    let monitors = Monitor::all().map_err(|e| format!("Failed to get monitors: {}", e))?;

    // Pick the primary or first one
    let monitor = monitors.into_iter().next().ok_or("No monitors found")?;

    // Capture the monitor
    let image = monitor
        .capture_image()
        .map_err(|e| format!("Failed to capture image: {}", e))?;

    // Encode to PNG
    let mut buffer = Cursor::new(Vec::new());
    image
        .write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode image to PNG: {}", e))?;

    let base64_image = general_purpose::STANDARD.encode(buffer.into_inner());

    debug!(
        "Screen capture successful ({} bytes Base64)",
        base64_image.len()
    );
    Ok(base64_image)
}

/// Captures the main screen and returns raw PNG bytes.
pub fn capture_screen_raw() -> Result<Vec<u8>, String> {
    debug!("Starting raw screen capture for OCR...");

    let monitors = Monitor::all().map_err(|e| format!("Failed to get monitors: {}", e))?;
    let monitor = monitors.into_iter().next().ok_or("No monitors found")?;

    let image = monitor
        .capture_image()
        .map_err(|e| format!("Failed to capture image: {}", e))?;

    let mut buffer = Cursor::new(Vec::new());
    image
        .write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode image to PNG: {}", e))?;

    Ok(buffer.into_inner())
}

/// Captures the screen and extracts text using Vision OCR.
/// Returns the extracted text or an empty string on failure.
#[cfg(target_os = "macos")]
pub fn capture_and_ocr_screen() -> String {
    match capture_screen_raw() {
        Ok(image_data) => {
            let text = crate::vision_ocr::ocr_screenshot(&image_data);
            debug!("OCR extracted {} characters from screen", text.len());
            text
        }
        Err(e) => {
            debug!("Failed to capture screen for OCR: {}", e);
            String::new()
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn capture_and_ocr_screen() -> String {
    String::new()
}
