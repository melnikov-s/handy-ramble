//! Vision OCR module for extracting text from screenshots
//!
//! Uses Apple's Vision Framework to perform local OCR on screenshots,
//! providing text context to the LLM without sending image tokens.

use log::debug;
use std::ffi::{c_char, CStr};

#[cfg(target_os = "macos")]
extern "C" {
    fn extract_text_from_image(image_data: *const u8, image_length: i32) -> *mut c_char;
    fn free_ocr_string(ptr: *mut c_char);
}

/// Extract text from an image using Vision OCR
///
/// Returns the extracted text, or an empty string if OCR fails or no text is found.
#[cfg(target_os = "macos")]
pub fn ocr_screenshot(image_data: &[u8]) -> String {
    let start = std::time::Instant::now();

    let text = unsafe {
        let result = extract_text_from_image(image_data.as_ptr(), image_data.len() as i32);
        if result.is_null() {
            return String::new();
        }
        let text = CStr::from_ptr(result).to_string_lossy().into_owned();
        free_ocr_string(result);
        text
    };

    debug!(
        "OCR completed in {:?}, extracted {} chars",
        start.elapsed(),
        text.len()
    );
    text
}

/// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub fn ocr_screenshot(_image_data: &[u8]) -> String {
    String::new()
}
