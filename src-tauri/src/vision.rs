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

/// Captures a specific region of the screen and returns a Base64-encoded PNG string.
/// Automatically detects which monitor the region belongs to.
pub fn capture_region(x: i32, y: i32, width: u32, height: u32) -> Result<String, String> {
    log::info!(
        "Starting regional capture: {}x{} at global coordinates ({}, {})",
        width,
        height,
        x,
        y
    );

    let monitors = Monitor::all().map_err(|e| format!("Failed to get monitors: {}", e))?;

    // Find the monitor that contains the top-left corner of the capture area
    // Coordinates (x,y) from the frontend are absolute screen coordinates on macOS
    let monitor = monitors
        .into_iter()
        .find(|m| {
            let mx = m.x().unwrap_or(0);
            let my = m.y().unwrap_or(0);
            let mw = m.width().unwrap_or(0) as i32;
            let mh = m.height().unwrap_or(0) as i32;
            x >= mx && x < mx + mw && y >= my && y < my + mh
        })
        .or_else(|| {
            // Fallback to primary monitor if not found (e.g. edge case)
            log::warn!(
                "No monitor returned strict match for ({},{}), falling back to first monitor",
                x,
                y
            );
            Monitor::all().ok()?.into_iter().next()
        })
        .ok_or("No suitable monitor found for capture region")?;

    let monitor_width = monitor.width().unwrap_or(1);
    let monitor_height = monitor.height().unwrap_or(1);

    log::info!(
        "Selected monitor for capture: logical {}x{} at ({}, {})",
        monitor_width,
        monitor_height,
        monitor.x().unwrap_or(0),
        monitor.y().unwrap_or(0)
    );

    let image = monitor
        .capture_image()
        .map_err(|e| format!("Failed to capture image: {}", e))?;

    // Create a dynamic image to crop it
    let dynamic_image = image::DynamicImage::ImageRgba8(image);

    let phys_width = dynamic_image.width();
    let phys_height = dynamic_image.height();

    log::info!("Captured physical image: {}x{}", phys_width, phys_height);

    // Calculate DPI scale factor (Physical / Logical)
    // Avoid division by zero
    let scale_x = if monitor_width > 0 {
        phys_width as f64 / monitor_width as f64
    } else {
        1.0
    };
    let scale_y = if monitor_height > 0 {
        phys_height as f64 / monitor_height as f64
    } else {
        1.0
    };

    log::info!("Calculated scale factors: x={}, y={}", scale_x, scale_y);

    // Convert absolute screen coordinates to monitor-relative coordinates (logical)
    let mx = monitor.x().unwrap_or(0);
    let my = monitor.y().unwrap_or(0);

    // Relative coordinates in logical pixels
    let rel_x_logical = (x - mx).max(0) as f64;
    let rel_y_logical = (y - my).max(0) as f64;

    // Convert to physical pixels
    let rel_x_phys = (rel_x_logical * scale_x).round() as u32;
    let rel_y_phys = (rel_y_logical * scale_y).round() as u32;
    let req_width_phys = (width as f64 * scale_x).round() as u32;
    let req_height_phys = (height as f64 * scale_y).round() as u32;

    log::info!(
        "Cropping at relative physical coords: ({}, {}) size {}x{} (Logical was ({}, {}) size {}x{})",
        rel_x_phys, rel_y_phys, req_width_phys, req_height_phys,
        rel_x_logical, rel_y_logical, width, height
    );

    // Bounds checking
    if rel_x_phys >= phys_width || rel_y_phys >= phys_height {
        return Err(format!(
            "Crop start ({}, {}) is outside image bounds {}x{}",
            rel_x_phys, rel_y_phys, phys_width, phys_height
        ));
    }

    // Clamp width/height to available space
    let available_width = phys_width - rel_x_phys;
    let available_height = phys_height - rel_y_phys;

    let crop_width = req_width_phys.min(available_width);
    let crop_height = req_height_phys.min(available_height);

    if crop_width == 0 || crop_height == 0 {
        return Err("Capture region resulted in 0 width or height".to_string());
    }

    // Wrap in catch_unwind to be absolutely paranoid about crashing the app
    let crop_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dynamic_image.crop_imm(rel_x_phys, rel_y_phys, crop_width, crop_height)
    }));

    let cropped = match crop_result {
        Ok(c) => c,
        Err(_) => return Err("Image cropping panicked".to_string()),
    };

    let mut buffer = Cursor::new(Vec::new());
    cropped
        .write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode image to PNG: {}", e))?;

    let base64_image = general_purpose::STANDARD.encode(buffer.into_inner());

    log::info!(
        "Region capture successful, encoded length: {}",
        base64_image.len()
    );

    Ok(base64_image)
}
