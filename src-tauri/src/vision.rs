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

/// Captures screen for Computer Use - compressed for reduced size.
/// Uses grayscale and lower resolution (1280px) with PNG format (required by API).
/// Typically ~150-300KB instead of ~13MB original (98%+ reduction).
pub fn capture_screen_for_computer_use() -> Result<String, String> {
    use image::imageops::FilterType;

    debug!("Starting compressed screen capture for Computer Use...");

    let monitors = Monitor::all().map_err(|e| format!("Failed to get monitors: {}", e))?;
    let monitor = monitors.into_iter().next().ok_or("No monitors found")?;

    let image = monitor
        .capture_image()
        .map_err(|e| format!("Failed to capture image: {}", e))?;

    let (orig_width, orig_height) = (image.width(), image.height());

    // Resize to max 1280 width - sufficient for UI element detection and OCR
    let max_width = 1280u32;
    let dynamic_image = image::DynamicImage::ImageRgba8(image);

    let resized = if orig_width > max_width {
        let scale = max_width as f32 / orig_width as f32;
        let new_height = (orig_height as f32 * scale) as u32;
        debug!(
            "Resizing from {}x{} to {}x{}",
            orig_width, orig_height, max_width, new_height
        );
        dynamic_image.resize(max_width, new_height, FilterType::Triangle)
    } else {
        dynamic_image
    };

    // Convert to grayscale - colors aren't needed for UI navigation
    let grayscale = resized.grayscale();

    // Encode to PNG (required by Gemini API)
    let mut buffer = Cursor::new(Vec::new());
    grayscale
        .write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode image to PNG: {}", e))?;

    let base64_image = general_purpose::STANDARD.encode(buffer.into_inner());
    let size_kb = base64_image.len() / 1024;

    debug!(
        "Compressed screen capture: {} KB (was ~{} KB raw, {:.1}% reduction)",
        size_kb,
        (orig_width * orig_height * 4) / 1024,
        100.0 - (base64_image.len() as f64 / (orig_width * orig_height * 4) as f64) * 100.0
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
