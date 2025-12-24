//! Unified key listener for all keyboard shortcuts.
//!
//! This module provides a clean, state machine-based approach to handling
//! keyboard shortcuts. It supports both raw modifier bindings (e.g., `right_option`)
//! and standard shortcuts (e.g., `Cmd+Space`).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐    ┌─────────────┐    ┌─────────────────┐
//! │   rdev      │───▶│   State     │───▶│   Action        │
//! │  callback   │    │   Machine   │    │   Dispatcher    │
//! └─────────────┘    └─────────────┘    └─────────────────┘
//! ```
//!
//! The state machine has clear transitions:
//! - Idle → Recording (on transcribe key press)
//! - Recording → Transcribing (on transcribe key release)
//! - Recording → Paused (on pause key)
//! - Recording → Idle (on cancel key)

use log::{debug, error, info};
use rdev::{listen, Event, EventType, Key};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

use crate::actions::ACTION_MAP;
use crate::managers::audio::AudioRecordingManager;
use crate::ManagedToggleState;

// ============================================================================
// Constants: Raw modifier binding identifiers
// ============================================================================

pub const RAW_BINDING_RIGHT_OPTION: &str = "right_option";
pub const RAW_BINDING_LEFT_OPTION: &str = "left_option";
pub const RAW_BINDING_SHIFT_RIGHT_OPTION: &str = "shift+right_option";
pub const RAW_BINDING_SHIFT_LEFT_OPTION: &str = "shift+left_option";
pub const RAW_BINDING_RIGHT_COMMAND: &str = "right_command";
pub const RAW_BINDING_LEFT_COMMAND: &str = "left_command";
pub const RAW_BINDING_SHIFT_RIGHT_COMMAND: &str = "shift+right_command";
pub const RAW_BINDING_SHIFT_LEFT_COMMAND: &str = "shift+left_command";

/// Check if a binding string is a raw modifier binding
pub fn is_raw_modifier_binding(binding: &str) -> bool {
    matches!(
        binding,
        RAW_BINDING_RIGHT_OPTION
            | RAW_BINDING_LEFT_OPTION
            | RAW_BINDING_SHIFT_RIGHT_OPTION
            | RAW_BINDING_SHIFT_LEFT_OPTION
            | RAW_BINDING_RIGHT_COMMAND
            | RAW_BINDING_LEFT_COMMAND
            | RAW_BINDING_SHIFT_RIGHT_COMMAND
            | RAW_BINDING_SHIFT_LEFT_COMMAND
    )
}

// ============================================================================
// State Machine Types
// ============================================================================

/// The current state of the key listener
#[derive(Debug, Clone)]
enum ListenerState {
    /// No recording in progress
    Idle,
    /// Recording audio, tracking when it started for tap/hold detection
    Recording {
        binding_id: String,
        press_time: Instant,
        /// Whether the key has been released (toggle mode)
        key_released: bool,
    },
    /// Recording is paused
    Paused { binding_id: String },
}

/// A registered binding maps a key string to an action ID
#[derive(Debug, Clone)]
struct RegisteredBinding {
    binding_id: String,
}

/// Thread-safe state container
struct KeyListenerState {
    /// Current state machine state
    state: ListenerState,
    /// Registered bindings: binding_string -> RegisteredBinding
    bindings: HashMap<String, RegisteredBinding>,
    /// Suspended binding IDs (for UI editing)
    suspended: std::collections::HashSet<String>,
    /// App handle for triggering actions
    app_handle: Option<AppHandle>,
    /// Modifier key tracking
    shift_pressed: bool,
}

impl KeyListenerState {
    fn new() -> Self {
        Self {
            state: ListenerState::Idle,
            bindings: HashMap::new(),
            suspended: std::collections::HashSet::new(),
            app_handle: None,
            shift_pressed: false,
        }
    }
}

// ============================================================================
// Global State
// ============================================================================

static LISTENER_STATE: OnceLock<Arc<Mutex<KeyListenerState>>> = OnceLock::new();
static LISTENER_RUNNING: AtomicBool = AtomicBool::new(false);

fn get_state() -> &'static Arc<Mutex<KeyListenerState>> {
    LISTENER_STATE.get_or_init(|| Arc::new(Mutex::new(KeyListenerState::new())))
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize the key listener. Call once at app startup.
pub fn init(app: &AppHandle) {
    let state = get_state();
    {
        let mut guard = state.lock().expect("Failed to lock listener state");
        guard.app_handle = Some(app.clone());
    }

    // Start the rdev listener in a background thread
    if !LISTENER_RUNNING.swap(true, Ordering::SeqCst) {
        std::thread::spawn(|| {
            info!("Starting unified key listener");
            if let Err(e) = listen(handle_rdev_event) {
                error!("Key listener failed: {:?}", e);
                LISTENER_RUNNING.store(false, Ordering::SeqCst);
            }
        });
    }
}

/// Register a raw modifier binding
pub fn register_raw_binding(binding_id: &str, binding_string: &str) -> Result<(), String> {
    if !is_raw_modifier_binding(binding_string) {
        return Err(format!("Not a raw modifier binding: {}", binding_string));
    }

    let state = get_state();
    let mut guard = state.lock().map_err(|e| format!("Lock error: {}", e))?;

    if guard.bindings.contains_key(binding_string) {
        // Already registered - just update the binding_id
        if let Some(binding) = guard.bindings.get_mut(binding_string) {
            binding.binding_id = binding_id.to_string();
        }
        return Ok(());
    }

    guard.bindings.insert(
        binding_string.to_string(),
        RegisteredBinding {
            binding_id: binding_id.to_string(),
        },
    );

    info!(
        "Registered raw binding: {} -> {}",
        binding_id, binding_string
    );
    Ok(())
}

/// Unregister a raw modifier binding
pub fn unregister_raw_binding(binding_string: &str) -> Result<(), String> {
    let state = get_state();
    let mut guard = state.lock().map_err(|e| format!("Lock error: {}", e))?;

    if guard.bindings.remove(binding_string).is_some() {
        info!("Unregistered raw binding: {}", binding_string);
        Ok(())
    } else {
        // Not an error - might have never been registered
        Ok(())
    }
}

/// Suspend a binding (for UI editing)
pub fn suspend_raw_binding(binding_id: &str) {
    if let Ok(mut guard) = get_state().lock() {
        guard.suspended.insert(binding_id.to_string());
        debug!("Suspended binding: {}", binding_id);
    }
}

/// Resume a binding after editing
pub fn resume_raw_binding(binding_id: &str) {
    if let Ok(mut guard) = get_state().lock() {
        guard.suspended.remove(binding_id);
        debug!("Resumed binding: {}", binding_id);
    }
}

/// Force reset to idle state (e.g., after error or cancel)
pub fn force_reset_state() {
    if let Ok(mut guard) = get_state().lock() {
        guard.state = ListenerState::Idle;
        debug!("Force reset to Idle state");
    }
}

// ============================================================================
// Event Handling
// ============================================================================

/// Main rdev callback - routes keyboard events to the state machine
fn handle_rdev_event(event: Event) {
    match event.event_type {
        EventType::KeyPress(key) => handle_key_press(key),
        EventType::KeyRelease(key) => handle_key_release(key),
        _ => {}
    }
}

fn handle_key_press(key: Key) {
    // Track shift state
    if matches!(key, Key::ShiftLeft | Key::ShiftRight) {
        if let Ok(mut guard) = get_state().lock() {
            guard.shift_pressed = true;
        }
        return;
    }

    // Handle modifier keys (Option/Command)
    if let Some(binding_string) = key_to_binding_string(key, true) {
        handle_transcribe_press(&binding_string);
        return;
    }

    // Handle passive keys during recording (Escape, S, P)
    match key {
        Key::Escape => handle_cancel(),
        Key::KeyS => handle_vision(),
        Key::KeyP => handle_pause(),
        _ => {}
    }
}

fn handle_key_release(key: Key) {
    // Track shift state
    if matches!(key, Key::ShiftLeft | Key::ShiftRight) {
        if let Ok(mut guard) = get_state().lock() {
            guard.shift_pressed = false;
        }
        return;
    }

    // Handle modifier key release
    if let Some(binding_string) = key_to_binding_string(key, false) {
        handle_transcribe_release(&binding_string);
    }
}

/// Convert an rdev Key to a binding string (e.g., "right_option")
fn key_to_binding_string(key: Key, check_shift: bool) -> Option<String> {
    let shift_pressed = if check_shift {
        get_state()
            .lock()
            .ok()
            .map(|g| g.shift_pressed)
            .unwrap_or(false)
    } else {
        // On release, we need to check if this was a shift+modifier combo
        // but shift might already be released, so we check the current state
        get_state()
            .lock()
            .ok()
            .map(|g| g.shift_pressed)
            .unwrap_or(false)
    };

    match key {
        Key::Alt => Some(if shift_pressed {
            RAW_BINDING_SHIFT_LEFT_OPTION.to_string()
        } else {
            RAW_BINDING_LEFT_OPTION.to_string()
        }),
        Key::AltGr => Some(if shift_pressed {
            RAW_BINDING_SHIFT_RIGHT_OPTION.to_string()
        } else {
            RAW_BINDING_RIGHT_OPTION.to_string()
        }),
        Key::MetaLeft => Some(if shift_pressed {
            RAW_BINDING_SHIFT_LEFT_COMMAND.to_string()
        } else {
            RAW_BINDING_LEFT_COMMAND.to_string()
        }),
        Key::MetaRight => Some(if shift_pressed {
            RAW_BINDING_SHIFT_RIGHT_COMMAND.to_string()
        } else {
            RAW_BINDING_RIGHT_COMMAND.to_string()
        }),
        _ => None,
    }
}

// ============================================================================
// State Machine Transitions
// ============================================================================

fn handle_transcribe_press(binding_string: &str) {
    let state = get_state();
    let (app, _binding_id, action) = {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // Look up the binding
        let binding = match guard.bindings.get(binding_string) {
            Some(b) => b.clone(),
            None => {
                // Try base binding if shift variant not found
                let base = binding_string
                    .strip_prefix("shift+")
                    .unwrap_or(binding_string);
                match guard.bindings.get(base) {
                    Some(b) => b.clone(),
                    None => return, // Not registered
                }
            }
        };

        // Check if suspended
        if guard.suspended.contains(&binding.binding_id) {
            return;
        }

        let app = match &guard.app_handle {
            Some(a) => a.clone(),
            None => return,
        };

        // State machine transition
        let action = match &guard.state {
            ListenerState::Idle => {
                // Idle → Recording
                guard.state = ListenerState::Recording {
                    binding_id: binding.binding_id.clone(),
                    press_time: Instant::now(),
                    key_released: false,
                };
                debug!("[STATE] Idle -> Recording ({})", binding.binding_id);
                Some(("start", binding.binding_id.clone()))
            }
            ListenerState::Recording {
                key_released: true,
                binding_id,
                ..
            } => {
                // Toggle off - Recording → Idle (stop)
                let bid = binding_id.clone();
                guard.state = ListenerState::Idle;
                debug!("[STATE] Recording -> Idle (toggle stop)");
                Some(("stop", bid))
            }
            ListenerState::Recording {
                key_released: false,
                ..
            } => {
                // Key pressed while still held - ignore
                None
            }
            ListenerState::Paused { binding_id } => {
                // Resume from pause
                let bid = binding_id.clone();
                guard.state = ListenerState::Recording {
                    binding_id: bid.clone(),
                    press_time: Instant::now(),
                    key_released: false,
                };
                debug!("[STATE] Paused -> Recording (resume)");
                Some(("start", bid)) // start resumes
            }
        };

        (app, binding.binding_id, action)
    };

    // Execute action outside of lock
    if let Some((action_type, bid)) = action {
        if action_type == "start" {
            // Update toggle state for actions
            let toggle_state = app.state::<ManagedToggleState>();
            if let Ok(mut states) = toggle_state.lock() {
                states.active_toggles.insert(bid.clone(), true);
            }

            if let Some(action) = ACTION_MAP.get(&bid) {
                let started = action.start(&app, &bid, binding_string);
                if !started {
                    // Reset state if start failed
                    if let Ok(mut guard) = state.lock() {
                        guard.state = ListenerState::Idle;
                    }
                    if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
                        states.active_toggles.insert(bid, false);
                    }
                } else {
                    // Start hold timer for mode detection
                    spawn_hold_timer(app.clone(), bid.clone());
                }
            }
        } else if action_type == "stop" {
            // Stop action
            if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
                states.active_toggles.insert(bid.clone(), false);
            }
            if let Some(action) = ACTION_MAP.get(&bid) {
                action.stop(&app, &bid, binding_string);
            }
        }
    }
}

fn handle_transcribe_release(binding_string: &str) {
    let state = get_state();
    let (app, action) = {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let app = match &guard.app_handle {
            Some(a) => a.clone(),
            None => return,
        };

        // State machine transition on release
        let action = match &guard.state {
            ListenerState::Recording {
                binding_id,
                press_time,
                key_released: false,
            } => {
                let held_ms = press_time.elapsed().as_millis() as u64;
                let threshold = get_hold_threshold(&app);
                let bid = binding_id.clone();

                if held_ms >= threshold {
                    // Long hold (PTT mode) - stop immediately
                    guard.state = ListenerState::Idle;
                    debug!(
                        "[STATE] Recording -> Idle (PTT release after {}ms)",
                        held_ms
                    );

                    // Emit hold mode
                    crate::overlay::emit_mode_determined(&app, "hold");

                    Some(("stop", bid, false))
                } else {
                    // Quick tap (toggle mode) - keep recording, mark key as released
                    guard.state = ListenerState::Recording {
                        binding_id: bid.clone(),
                        press_time: *press_time,
                        key_released: true,
                    };
                    debug!(
                        "[STATE] Recording: key released (toggle mode, {}ms)",
                        held_ms
                    );

                    // Set coherent mode and emit refining
                    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
                    audio_manager.set_coherent_mode(true);
                    crate::utils::show_ramble_recording_overlay(&app);
                    crate::overlay::emit_mode_determined(&app, "refining");

                    // Capture selection context on main thread
                    let app_clone = app.clone();
                    let audio_manager_clone = Arc::clone(&audio_manager);
                    let _ = app.run_on_main_thread(move || {
                        if let Ok(Some(text)) = crate::clipboard::get_selected_text(&app_clone) {
                            debug!("Captured selection context: {} chars", text.len());
                            audio_manager_clone.set_selection_context(text);
                        }
                    });

                    None // Don't stop yet
                }
            }
            _ => None,
        };

        (app, action)
    };

    // Execute stop action outside of lock
    if let Some(("stop", bid, _)) = action {
        if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
            states.active_toggles.insert(bid.clone(), false);
        }
        if let Some(action) = ACTION_MAP.get(&bid) {
            action.stop(&app, &bid, binding_string);
        }
    }
}

fn handle_cancel() {
    let state = get_state();
    let app = {
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // Only cancel if we're recording or paused
        match &guard.state {
            ListenerState::Recording { .. } | ListenerState::Paused { .. } => {
                guard.app_handle.clone()
            }
            _ => None,
        }
    };

    if let Some(app) = app {
        info!("Cancel triggered via Escape");
        crate::utils::cancel_current_operation(&app);
        force_reset_state();
    }
}

fn handle_vision() {
    let state = get_state();
    let (app, is_active, has_modifier) = {
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let is_active = matches!(
            guard.state,
            ListenerState::Recording { .. } | ListenerState::Paused { .. }
        );
        // Only trigger if a modifier is held (to avoid accidental triggers while typing)
        let has_modifier = guard.shift_pressed; // Could add more modifiers

        (guard.app_handle.clone(), is_active, has_modifier)
    };

    if let (Some(app), true, true) = (app, is_active, has_modifier) {
        info!("Vision capture triggered via S + modifier");
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            match crate::vision::capture_screen() {
                Ok(base64) => {
                    let audio_manager = app_clone.state::<Arc<AudioRecordingManager>>();
                    audio_manager.add_vision_context(base64);
                    let _ = app_clone.emit("vision-captured", ());
                }
                Err(e) => error!("Vision capture failed: {}", e),
            }
        });
    }
}

fn handle_pause() {
    let state = get_state();
    let (app, is_active, has_modifier) = {
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let is_active = matches!(
            guard.state,
            ListenerState::Recording { .. } | ListenerState::Paused { .. }
        );
        let has_modifier = guard.shift_pressed;

        (guard.app_handle.clone(), is_active, has_modifier)
    };

    if let (Some(app), true, true) = (app, is_active, has_modifier) {
        info!("Pause toggle triggered via P + modifier");
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::utils::toggle_pause_operation(&app_clone);
        });
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn get_hold_threshold(app: &AppHandle) -> u64 {
    crate::settings::get_settings(app).hold_threshold_ms
}

fn spawn_hold_timer(app: AppHandle, binding_id: String) {
    let threshold = get_hold_threshold(&app);

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(threshold));

        // Check if still recording and key not released
        let should_emit = get_state()
            .lock()
            .ok()
            .map(|g| {
                matches!(
                    &g.state,
                    ListenerState::Recording { key_released: false, binding_id: bid, .. }
                    if bid == &binding_id
                )
            })
            .unwrap_or(false);

        if should_emit {
            debug!("Hold threshold reached - emitting hold mode");
            crate::overlay::emit_mode_determined(&app, "hold");
        }
    });
}
