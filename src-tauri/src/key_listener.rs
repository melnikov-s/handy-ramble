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

// Note: Raw bindings are now handled generically in is_raw_modifier_binding.
// Individual constants are kept for reference if needed elsewhere.

/// Check if a binding string is a raw modifier binding (composed only of modifiers)
pub fn is_raw_modifier_binding(binding: &str) -> bool {
    let modifiers = [
        "ctrl",
        "control",
        "shift",
        "alt",
        "option",
        "meta",
        "command",
        "cmd",
        "right_option",
        "left_option",
        "right_command",
        "left_command",
        "right_shift",
        "left_shift",
    ];

    binding
        .split('+')
        .all(|part| modifiers.contains(&part.trim().to_lowercase().as_str()))
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

    // Modifier Tracking
    shift_left_pressed: bool,
    shift_right_pressed: bool,
    ctrl_pressed: bool,
    alt_pressed: bool,        // Left Option on Mac
    alt_gr_pressed: bool,     // Right Option on Mac
    meta_left_pressed: bool,  // Left Command on Mac
    meta_right_pressed: bool, // Right Command on Mac
}

impl KeyListenerState {
    fn new() -> Self {
        Self {
            state: ListenerState::Idle,
            bindings: HashMap::new(),
            suspended: std::collections::HashSet::new(),
            app_handle: None,
            shift_left_pressed: false,
            shift_right_pressed: false,
            ctrl_pressed: false,
            alt_pressed: false,
            alt_gr_pressed: false,
            meta_left_pressed: false,
            meta_right_pressed: false,
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
    let mut current_modifiers = Vec::new();

    // 1. Update Modifier State & Build Modifier String
    if let Ok(mut guard) = get_state().lock() {
        match key {
            Key::ShiftLeft => guard.shift_left_pressed = true,
            Key::ShiftRight => guard.shift_right_pressed = true,
            Key::ControlLeft | Key::ControlRight => guard.ctrl_pressed = true,
            Key::Alt => guard.alt_pressed = true,
            Key::AltGr => guard.alt_gr_pressed = true,
            Key::MetaLeft => guard.meta_left_pressed = true,
            Key::MetaRight => guard.meta_right_pressed = true,
            _ => {}
        }

        // Build current chord string for identification
        if guard.shift_left_pressed {
            current_modifiers.push("left_shift");
        }
        if guard.shift_right_pressed {
            current_modifiers.push("right_shift");
        }
        if guard.ctrl_pressed {
            current_modifiers.push("ctrl");
        }
        if guard.alt_pressed {
            current_modifiers.push("left_option");
        }
        if guard.alt_gr_pressed {
            current_modifiers.push("right_option");
        }
        if guard.meta_left_pressed {
            current_modifiers.push("left_command");
        }
        if guard.meta_right_pressed {
            current_modifiers.push("right_command");
        }
    }

    // 2. Identify the Binding
    // We check for exact matches based on the physical key pressed + current modifiers.
    // e.g. If Shift is held and RightCmd is pressed, binding_string = "shift+right_command"
    let binding_string = key_to_binding_string_chord(key, &current_modifiers);

    if let Some(bs) = binding_string {
        if handle_behavior_press(&bs) {
            return; // Action handled
        }
    }

    // 3. Handle Passive Keys during Recording (S, P, Esc)
    // Only if not already handled by a binding
    match key {
        Key::Escape => handle_cancel(),
        Key::KeyS => handle_vision(),
        Key::KeyP => handle_pause(),
        _ => {}
    }
}

fn handle_key_release(key: Key) {
    let mut current_modifiers = Vec::new();

    // 1. Get snapshot of modifiers BEFORE we update them for release
    if let Ok(guard) = get_state().lock() {
        if guard.shift_left_pressed {
            current_modifiers.push("left_shift");
        }
        if guard.shift_right_pressed {
            current_modifiers.push("right_shift");
        }
        if guard.ctrl_pressed {
            current_modifiers.push("ctrl");
        }
        if guard.alt_pressed {
            current_modifiers.push("left_option");
        }
        if guard.alt_gr_pressed {
            current_modifiers.push("right_option");
        }
        if guard.meta_left_pressed {
            current_modifiers.push("left_command");
        }
        if guard.meta_right_pressed {
            current_modifiers.push("right_command");
        }
    }

    // 2. Resolve the binding that is being released
    let binding_string = key_to_binding_string_chord(key, &current_modifiers);

    // 3. Trigger behavior release
    if let Some(bs) = binding_string {
        handle_behavior_release(&bs);
    }

    // 4. Update Global Modifier State
    if let Ok(mut guard) = get_state().lock() {
        match key {
            Key::ShiftLeft => guard.shift_left_pressed = false,
            Key::ShiftRight => guard.shift_right_pressed = false,
            Key::ControlLeft | Key::ControlRight => guard.ctrl_pressed = false,
            Key::Alt => guard.alt_pressed = false,
            Key::AltGr => guard.alt_gr_pressed = false,
            Key::MetaLeft => guard.meta_left_pressed = false,
            Key::MetaRight => guard.meta_right_pressed = false,
            _ => {}
        }
    }
}

/// Helper to build a binding string for the current key event
fn key_to_binding_string_chord(key: Key, modifiers: &[&str]) -> Option<String> {
    let key_name = match key {
        Key::Alt => "left_option",
        Key::AltGr => "right_option",
        Key::MetaLeft => "left_command",
        Key::MetaRight => "right_command",
        Key::ShiftLeft => "left_shift",
        Key::ShiftRight => "right_shift",
        Key::ControlLeft | Key::ControlRight => "ctrl",
        Key::Space => "space",
        Key::KeyQ => "q",
        Key::KeyA => "a",
        Key::KeyS => "s",
        Key::KeyZ => "z",
        _ => return None, // Add more as needed
    };

    // If the key itself is a modifier, we want the chord WITHOUT it in the prefix
    // e.g. Chord = ["shift", "right_command"], Key = "right_command" -> "shift+right_command"
    let modifier_prefix: Vec<String> = modifiers
        .iter()
        .filter(|&&m| m != key_name)
        .map(|&m| m.to_string())
        .collect();

    if modifier_prefix.is_empty() {
        Some(key_name.to_string())
    } else {
        Some(format!("{}+{}", modifier_prefix.join("+"), key_name))
    }
}

// Replaced by key_to_binding_string_chord and behavior handlers

// ============================================================================
// State Machine Transitions
// ============================================================================

// behavior handlers
fn handle_behavior_press(binding_string: &str) -> bool {
    use crate::actions::InteractionBehavior;

    let state = get_state();
    let (app, binding_id, behavior) = {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };

        // Exact match lookup
        let binding = match guard.bindings.get(binding_string) {
            Some(b) => b.clone(),
            None => return false,
        };

        if guard.suspended.contains(&binding.binding_id) {
            return false;
        }

        let app = match &guard.app_handle {
            Some(a) => a.clone(),
            None => return false,
        };

        let action = match ACTION_MAP.get(&binding.binding_id) {
            Some(a) => a,
            None => return false,
        };

        let behavior = action.interaction_behavior();

        // State machine transition
        match guard.state.clone() {
            ListenerState::Idle => {
                // Determine next state based on behavior
                match behavior {
                    InteractionBehavior::Instant => {
                        // Fires once, stays Idle
                    }
                    InteractionBehavior::Hybrid | InteractionBehavior::Momentary => {
                        guard.state = ListenerState::Recording {
                            binding_id: binding.binding_id.clone(),
                            press_time: Instant::now(),
                            key_released: false,
                        };
                    }
                }
                (app, binding.binding_id.clone(), behavior)
            }
            ListenerState::Recording {
                key_released: true,
                binding_id,
                ..
            } => {
                if binding_id == binding.binding_id {
                    // Toggle off
                    guard.state = ListenerState::Idle;
                    (app, binding.binding_id.clone(), behavior)
                } else {
                    return false;
                }
            }
            ListenerState::Paused { binding_id } => {
                if binding_id == binding.binding_id {
                    // Resume
                    guard.state = ListenerState::Recording {
                        binding_id: binding.binding_id.clone(),
                        press_time: Instant::now(),
                        key_released: false,
                    };
                    (app, binding.binding_id.clone(), behavior)
                } else {
                    return false;
                }
            }
            _ => return false,
        }
    };

    // Execute outside of lock
    match behavior {
        InteractionBehavior::Instant => {
            if let Some(action) = ACTION_MAP.get(&binding_id) {
                action.start(&app, &binding_id, binding_string);
            }
            true
        }
        InteractionBehavior::Hybrid | InteractionBehavior::Momentary => {
            // Check if we are actually starting or resuming
            let is_stop = {
                let guard = state.lock().unwrap();
                matches!(guard.state, ListenerState::Idle)
            };

            if is_stop {
                // Stop action (Toggle off)
                if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
                    states.active_toggles.insert(binding_id.clone(), false);
                }
                if let Some(action) = ACTION_MAP.get(&binding_id) {
                    action.stop(&app, &binding_id, binding_string);
                }
            } else {
                // Start/Resume action
                if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
                    states.active_toggles.insert(binding_id.clone(), true);
                }
                if let Some(action) = ACTION_MAP.get(&binding_id) {
                    let started = action.start(&app, &binding_id, binding_string);
                    if started {
                        if behavior == InteractionBehavior::Hybrid {
                            spawn_hold_timer(app.clone(), binding_id.clone());
                        }
                    } else {
                        // Reset if failed
                        let mut guard = state.lock().unwrap();
                        guard.state = ListenerState::Idle;
                        if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
                            states.active_toggles.insert(binding_id.clone(), false);
                        }
                    }
                }
            }
            true
        }
    }
}

fn handle_behavior_release(binding_string: &str) {
    use crate::actions::InteractionBehavior;

    let state = get_state();
    let (app, binding_id, behavior, is_long_hold) = {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        match guard.state.clone() {
            ListenerState::Recording {
                binding_id,
                press_time,
                key_released: false,
            } => {
                let app = guard.app_handle.as_ref().cloned().unwrap();
                let action = match ACTION_MAP.get(&binding_id) {
                    Some(a) => a,
                    None => return,
                };
                let behavior = action.interaction_behavior();
                let threshold = get_hold_threshold(&app);
                let held_ms = press_time.elapsed().as_millis() as u64;
                let is_long_hold = held_ms >= threshold;

                match behavior {
                    InteractionBehavior::Instant => {
                        // Should not even be in Recording state for Instant
                        return;
                    }
                    InteractionBehavior::Momentary => {
                        // Transitions to Idle
                        let bid = binding_id.clone();
                        guard.state = ListenerState::Idle;
                        (app, bid, behavior, true)
                    }
                    InteractionBehavior::Hybrid => {
                        if is_long_hold {
                            // PTT release -> Idle
                            let bid = binding_id.clone();
                            guard.state = ListenerState::Idle;
                            (app, bid, behavior, true)
                        } else {
                            // Tap -> remain in Recording, mark released
                            guard.state = ListenerState::Recording {
                                binding_id: binding_id.clone(),
                                press_time: press_time,
                                key_released: true,
                            };
                            (app, binding_id.clone(), behavior, false)
                        }
                    }
                }
            }
            _ => return,
        }
    };

    // Execute outside of lock
    match behavior {
        InteractionBehavior::Hybrid => {
            if is_long_hold {
                // PTT stop
                if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
                    states.active_toggles.insert(binding_id.clone(), false);
                }
                if let Some(action) = ACTION_MAP.get(&binding_id) {
                    action.stop(&app, &binding_id, binding_string);
                }
                crate::overlay::emit_mode_determined(&app, "hold");
            } else {
                // Toggle ON - Tap
                // Check which action this is - voice commands should NOT switch to refining mode
                let is_voice_command = binding_id == "voice_command";
                let is_context_chat = binding_id == "context_chat";

                if is_voice_command {
                    // Voice commands stay in voice command mode, no refining
                    // The overlay was already set when start() was called
                } else if is_context_chat {
                    // Context chat needs selection but no refining mode
                    let app_clone = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let Ok(Some(text)) = crate::clipboard::get_selected_text(&app_clone) {
                            if let Some(mgr) = app_clone.try_state::<Arc<AudioRecordingManager>>() {
                                debug!(
                                    "[CONTEXT_CHAT] Captured selection context: {} chars",
                                    text.len()
                                );
                                mgr.set_selection_context(text);
                            }
                        }
                    });
                } else {
                    // Regular transcribe action - switch to refining mode
                    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
                    audio_manager.set_coherent_mode(true);
                    crate::utils::show_ramble_recording_overlay(&app);
                    crate::overlay::emit_mode_determined(&app, "refining");

                    let app_clone = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let Ok(Some(text)) = crate::clipboard::get_selected_text(&app_clone) {
                            if let Some(mgr) = app_clone.try_state::<Arc<AudioRecordingManager>>() {
                                mgr.set_selection_context(text);
                            }
                        }
                    });
                }
            }
        }
        InteractionBehavior::Momentary => {
            if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
                states.active_toggles.insert(binding_id.clone(), false);
            }
            if let Some(action) = ACTION_MAP.get(&binding_id) {
                action.stop(&app, &binding_id, binding_string);
            }
        }
        _ => {}
    }
}

fn handle_cancel() {
    debug!("handle_cancel() invoked - Escape key detected");
    let state = get_state();
    let (app, current_state) = {
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        (guard.app_handle.clone(), format!("{:?}", guard.state))
    };
    debug!("handle_cancel: current state = {}", current_state);

    if let Some(app) = app {
        // Priority 1: Close focused Quick Chat window
        let windows = app.webview_windows();
        for (label, window) in windows {
            if label.starts_with("chat_") {
                if let Ok(true) = window.is_focused() {
                    info!("Closing focused chat window '{}' via Escape", label);
                    let _ = window.close();
                    return; // Exit - window closed, don't cancel recording
                }
            }
        }

        // Priority 2: Cancel active recording
        let should_cancel = {
            let guard = state.lock().unwrap();
            matches!(
                &guard.state,
                ListenerState::Recording { .. } | ListenerState::Paused { .. }
            )
        };

        debug!("handle_cancel: should_cancel = {}", should_cancel);
        if should_cancel {
            info!("Cancel recording triggered via Escape");
            crate::utils::cancel_current_operation(&app);
            force_reset_state();
        } else {
            // Even if state is Idle, stop any active TTS playback
            debug!("handle_cancel: state is Idle, stopping TTS if playing");
            crate::utils::stop_tts_and_hide_overlay(&app);
        }
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
        let has_modifier = guard.shift_left_pressed || guard.shift_right_pressed; // Could add more modifiers

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
        let has_modifier = guard.shift_left_pressed || guard.shift_right_pressed;

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
