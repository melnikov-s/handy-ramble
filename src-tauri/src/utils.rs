use crate::managers::audio::AudioRecordingManager;
use crate::shortcut;
use crate::ManagedToggleState;
use log::{info, warn};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

// Re-export all utility modules for easy access
// pub use crate::audio_feedback::*;
pub use crate::clipboard::*;
pub use crate::overlay::*;
pub use crate::tray::*;

/// Centralized cancellation function that can be called from anywhere in the app.
/// Handles cancelling both recording and transcription operations and updates UI state.
pub fn cancel_current_operation(app: &AppHandle) {
    info!("Initiating operation cancellation...");

    // Unregister the cancel shortcut asynchronously
    shortcut::unregister_cancel_shortcut(app);

    // First, reset all shortcut toggle states.
    // This is critical for non-push-to-talk mode where shortcuts toggle on/off
    let toggle_state_manager = app.state::<ManagedToggleState>();
    if let Ok(mut states) = toggle_state_manager.lock() {
        states.active_toggles.values_mut().for_each(|v| *v = false);
    } else {
        warn!("Failed to lock toggle state manager during cancellation");
    }

    // Cancel any ongoing recording
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    audio_manager.cancel_recording();

    // Update tray icon and hide overlay
    change_tray_icon(app, crate::tray::TrayIconState::Idle);
    hide_recording_overlay(app);

    info!("Operation cancellation completed - returned to idle state");
}

/// Pause the current recording operation without discarding audio.
/// Returns the binding_id if pausing was successful.
pub fn pause_current_operation(app: &AppHandle) -> Option<String> {
    info!("Initiating operation pause...");

    let audio_manager = app.state::<Arc<AudioRecordingManager>>();

    if let Some(binding_id) = audio_manager.pause_recording() {
        // Determine if this is a ramble binding by checking the binding_id
        let is_ramble = binding_id == "ramble_to_coherent";

        // Show the paused overlay
        if is_ramble {
            show_paused_overlay(app, true);
        } else {
            show_paused_overlay(app, false);
        }

        info!("Operation paused for binding {}", binding_id);
        Some(binding_id)
    } else {
        warn!("No active recording to pause");
        None
    }
}

/// Resume a paused recording operation.
/// Returns the binding_id if resuming was successful.
pub fn resume_current_operation(app: &AppHandle) -> Option<String> {
    info!("Initiating operation resume...");

    let audio_manager = app.state::<Arc<AudioRecordingManager>>();

    if let Some(binding_id) = audio_manager.resume_recording() {
        // Determine if this is a ramble binding
        let is_ramble = binding_id == "ramble_to_coherent";

        // Show the appropriate recording overlay
        if is_ramble {
            show_ramble_recording_overlay(app);
        } else {
            show_recording_overlay(app);
        }

        info!("Operation resumed for binding {}", binding_id);
        Some(binding_id)
    } else {
        warn!("No paused recording to resume");
        None
    }
}

/// Check if there is a paused recording for the given binding_id
pub fn is_operation_paused(app: &AppHandle, binding_id: &str) -> bool {
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    if let Some(paused_binding) = audio_manager.get_paused_binding_id() {
        paused_binding == binding_id
    } else {
        false
    }
}

/// Check if using the Wayland display server protocol
#[cfg(target_os = "linux")]
pub fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.to_lowercase() == "wayland")
            .unwrap_or(false)
}
