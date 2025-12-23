//! macOS frontmost application detection via Swift bridge.
//!
//! This module provides functionality to detect the currently focused application
//! on macOS, used for application-aware prompt selection.

use log::debug;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::ffi::CStr;

/// Information about a detected application
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AppInfo {
    pub bundle_identifier: String,
    pub display_name: String,
}

/// Information about an installed application (from JSON)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstalledApp {
    pub bundle_id: String,
    pub name: String,
}

#[cfg(target_os = "macos")]
use std::ffi::c_char;

#[cfg(target_os = "macos")]
extern "C" {
    fn get_frontmost_app_bundle_id() -> *mut c_char;
    fn get_frontmost_app_name() -> *mut c_char;
    fn free_string(ptr: *mut c_char);
    fn get_installed_applications_json() -> *mut c_char;
}

/// Get information about the currently focused application.
/// Returns None if the frontmost app cannot be determined.
#[cfg(target_os = "macos")]
pub fn get_frontmost_application() -> Option<AppInfo> {
    unsafe {
        let bundle_id_ptr = get_frontmost_app_bundle_id();
        let name_ptr = get_frontmost_app_name();

        let bundle_id = if !bundle_id_ptr.is_null() {
            let s = CStr::from_ptr(bundle_id_ptr).to_string_lossy().into_owned();
            free_string(bundle_id_ptr);
            s
        } else {
            String::new()
        };

        let name = if !name_ptr.is_null() {
            let s = CStr::from_ptr(name_ptr).to_string_lossy().into_owned();
            free_string(name_ptr);
            s
        } else {
            String::new()
        };

        if bundle_id.is_empty() && name.is_empty() {
            debug!("Frontmost app detection returned empty result");
            None
        } else {
            debug!("Detected frontmost app: {} ({})", name, bundle_id);
            Some(AppInfo {
                bundle_identifier: bundle_id,
                display_name: name,
            })
        }
    }
}

/// Get a list of installed applications on the system.
#[cfg(target_os = "macos")]
pub fn get_installed_applications() -> Vec<InstalledApp> {
    unsafe {
        let json_ptr = get_installed_applications_json();
        if json_ptr.is_null() {
            return Vec::new();
        }

        let json_str = CStr::from_ptr(json_ptr).to_string_lossy().into_owned();
        free_string(json_ptr);

        match serde_json::from_str::<Vec<InstalledApp>>(&json_str) {
            Ok(apps) => {
                debug!("Found {} installed applications", apps.len());
                apps
            }
            Err(e) => {
                debug!("Failed to parse installed apps JSON: {}", e);
                Vec::new()
            }
        }
    }
}

// Stub implementations for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_application() -> Option<AppInfo> {
    debug!("Frontmost app detection not available on this platform");
    None
}

#[cfg(not(target_os = "macos"))]
pub fn get_installed_applications() -> Vec<InstalledApp> {
    debug!("Installed apps detection not available on this platform");
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_info_creation() {
        let info = AppInfo {
            bundle_identifier: "com.example.test".to_string(),
            display_name: "Test App".to_string(),
        };
        assert_eq!(info.bundle_identifier, "com.example.test");
        assert_eq!(info.display_name, "Test App");
    }
}
