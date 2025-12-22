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
