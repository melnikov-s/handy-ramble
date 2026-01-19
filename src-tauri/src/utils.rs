use crate::managers::audio::AudioRecordingManager;
use crate::managers::tts::TTSManager;
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
    // Capture backtrace to identify caller
    let bt = std::backtrace::Backtrace::force_capture();
    info!("Initiating operation cancellation... Backtrace:\n{}", bt);

    // First, reset all shortcut toggle states.
    // This is critical for non-push-to-talk mode where shortcuts toggle on/off
    let toggle_state_manager = app.state::<ManagedToggleState>();
    if let Ok(mut states) = toggle_state_manager.lock() {
        states.active_toggles.values_mut().for_each(|v| *v = false);
    } else {
        warn!("Failed to lock toggle state manager during cancellation");
    }

    // Force reset state machine to avoid stuck states
    #[cfg(target_os = "macos")]
    crate::key_listener::force_reset_state();

    // Cancel any ongoing recording
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    audio_manager.cancel_recording();

    // Stop any ongoing TTS
    let tts_manager = app.state::<Arc<TTSManager>>();
    let tts_manager_cloned = tts_manager.inner().clone();
    tauri::async_runtime::spawn(async move {
        let _ = tts_manager_cloned.stop().await;
    });

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
        // Correctly determine if session is in coherent mode
        let is_coherent = audio_manager.get_coherent_mode();

        // Show the paused overlay
        show_paused_overlay(app, is_coherent);

        info!(
            "Operation paused for binding {} (coherent={})",
            binding_id, is_coherent
        );
        Some(binding_id)
    } else {
        warn!("No active recording to pause");
        None
    }
}

pub fn resume_current_operation(app: &AppHandle) -> Option<String> {
    info!("Initiating operation resume...");

    let audio_manager = app.state::<Arc<AudioRecordingManager>>();

    if let Some(binding_id) = audio_manager.resume_recording() {
        // Correctly determine if session is in coherent mode
        let is_coherent = audio_manager.get_coherent_mode();

        // Show the appropriate recording overlay
        if is_coherent {
            show_ramble_recording_overlay(app);
            // Re-emit mode so buttons reappear
            crate::overlay::emit_mode_determined(app, "refining");
        } else {
            show_recording_overlay(app);
            // Re-emit mode if it was already known (otherwise it stays optimistic)
            crate::overlay::emit_mode_determined(app, "hold");
        }

        info!(
            "Operation resumed for binding {} (coherent={})",
            binding_id, is_coherent
        );
        Some(binding_id)
    } else {
        warn!("No paused recording to resume");
        None
    }
}

/// Toggle pause/resume of the current recording operation.
pub fn toggle_pause_operation(app: &AppHandle) {
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    if audio_manager.get_paused_binding_id().is_some() {
        resume_current_operation(app);
    } else {
        pause_current_operation(app);
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

/// Stop any active TTS playback and hide the overlay.
/// This is used when Escape is pressed in Idle state to stop speaking.
pub fn stop_tts_and_hide_overlay(app: &AppHandle) {
    let tts_manager = app.state::<Arc<TTSManager>>();
    let tts_manager_cloned = tts_manager.inner().clone();
    tauri::async_runtime::spawn(async move {
        let _ = tts_manager_cloned.stop().await;
    });

    hide_recording_overlay(app);
    info!("TTS stopped and overlay hidden via Escape key");
}
