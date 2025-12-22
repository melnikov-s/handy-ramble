//! macOS-only raw key listener for standalone modifier key bindings.
//!
//! This module provides support for binding left/right Option keys as standalone
//! transcription triggers on macOS. The standard `tauri-plugin-global-shortcut`
//! cannot represent modifier-only shortcuts or distinguish left/right modifiers,
//! so we use `rdev` to capture these at a low level.
//!
//! ## Supported bindings
//! - `"right_option"` - Right Option key as standalone trigger
//! - `"left_option"` - Left Option key as standalone trigger
//!
//! ## Requirements
//! - macOS Accessibility permission (already required by Ramble for paste functionality)

use log::{debug, error, info, warn};
use rdev::{listen, Event, EventType, Key};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

use crate::managers::audio::AudioRecordingManager;

/// Binding identifiers for raw modifier shortcuts
pub const RAW_BINDING_RIGHT_OPTION: &str = "right_option";
pub const RAW_BINDING_LEFT_OPTION: &str = "left_option";
pub const RAW_BINDING_SHIFT_RIGHT_OPTION: &str = "shift+right_option";
pub const RAW_BINDING_SHIFT_LEFT_OPTION: &str = "shift+left_option";
pub const RAW_BINDING_RIGHT_COMMAND: &str = "right_command";
pub const RAW_BINDING_LEFT_COMMAND: &str = "left_command";
pub const RAW_BINDING_SHIFT_RIGHT_COMMAND: &str = "shift+right_command";
pub const RAW_BINDING_SHIFT_LEFT_COMMAND: &str = "shift+left_command";

/// Check if a binding string is a raw modifier binding (macOS-only)
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

/// Represents the state of a modifier key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKeyState {
    Pressed,
    Released,
}

/// A registered raw modifier binding
#[derive(Debug, Clone)]
struct RawBinding {
    binding_id: String,
    #[allow(dead_code)]
    binding_string: String,
}

/// Thread-safe state for the modifier key listener
struct ModifierListenerState {
    /// Registered raw bindings: binding_string -> RawBinding
    bindings: HashMap<String, RawBinding>,
    /// Suspended binding IDs (don't fire while user is editing)
    suspended: std::collections::HashSet<String>,
    /// Track current pressed state for each binding
    pressed_state: HashMap<String, bool>,
    /// Track when each binding was pressed (for tap vs hold detection)
    press_timestamps: HashMap<String, std::time::Instant>,
    /// App handle for triggering actions
    app_handle: Option<AppHandle>,
    /// Track if Shift is currently held (to allow Shift+Option shortcuts to work)
    shift_pressed: bool,
    /// Track if Alt/Option is currently held
    alt_pressed: bool,
    /// Track if Control is currently held
    ctrl_pressed: bool,
    /// Track if Command/Meta is currently held
    meta_pressed: bool,
}

impl ModifierListenerState {
    fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            suspended: std::collections::HashSet::new(),
            pressed_state: HashMap::new(),
            press_timestamps: HashMap::new(),
            app_handle: None,
            shift_pressed: false,
            alt_pressed: false,
            ctrl_pressed: false,
            meta_pressed: false,
        }
    }
}

/// Global state for the modifier listener
static LISTENER_STATE: OnceLock<Arc<Mutex<ModifierListenerState>>> = OnceLock::new();
static LISTENER_RUNNING: AtomicBool = AtomicBool::new(false);

fn get_listener_state() -> &'static Arc<Mutex<ModifierListenerState>> {
    LISTENER_STATE.get_or_init(|| Arc::new(Mutex::new(ModifierListenerState::new())))
}

/// Initialize the macOS modifier key listener.
/// This must be called once during app startup.
pub fn init_modifier_listener(app: &AppHandle) {
    let state = get_listener_state();
    {
        let mut guard = state.lock().expect("Failed to lock listener state");
        guard.app_handle = Some(app.clone());
    }

    // Start the event listener in a background thread if not already running
    if !LISTENER_RUNNING.swap(true, Ordering::SeqCst) {
        std::thread::spawn(|| {
            info!("Starting macOS modifier key listener (rdev)");
            if let Err(e) = listen(rdev_callback) {
                error!("Failed to start rdev listener: {:?}", e);
                LISTENER_RUNNING.store(false, Ordering::SeqCst);
            }
        });
    }
}

/// Register a raw modifier binding.
pub fn register_raw_binding(binding_id: &str, binding_string: &str) -> Result<(), String> {
    if !is_raw_modifier_binding(binding_string) {
        return Err(format!("Not a raw modifier binding: {}", binding_string));
    }

    let state = get_listener_state();
    let mut guard = state.lock().map_err(|e| format!("Lock error: {}", e))?;

    // Check for duplicates
    if guard.bindings.contains_key(binding_string) {
        return Err(format!(
            "Raw binding '{}' is already registered",
            binding_string
        ));
    }

    guard.bindings.insert(
        binding_string.to_string(),
        RawBinding {
            binding_id: binding_id.to_string(),
            binding_string: binding_string.to_string(),
        },
    );
    guard
        .pressed_state
        .insert(binding_string.to_string(), false);

    info!(
        "Registered raw modifier binding: {} -> {}",
        binding_id, binding_string
    );
    Ok(())
}

/// Unregister a raw modifier binding.
pub fn unregister_raw_binding(binding_string: &str) -> Result<(), String> {
    let state = get_listener_state();
    let mut guard = state.lock().map_err(|e| format!("Lock error: {}", e))?;

    if guard.bindings.remove(binding_string).is_some() {
        guard.pressed_state.remove(binding_string);
        info!("Unregistered raw modifier binding: {}", binding_string);
        Ok(())
    } else {
        Err(format!(
            "Raw binding '{}' was not registered",
            binding_string
        ))
    }
}

/// Suspend a raw binding (don't fire while user is editing).
pub fn suspend_raw_binding(binding_id: &str) {
    let state = get_listener_state();
    if let Ok(mut guard) = state.lock() {
        guard.suspended.insert(binding_id.to_string());
        debug!("Suspended raw binding: {}", binding_id);
    }
}

/// Resume a raw binding after editing.
pub fn resume_raw_binding(binding_id: &str) {
    let state = get_listener_state();
    if let Ok(mut guard) = state.lock() {
        guard.suspended.remove(binding_id);
        debug!("Resumed raw binding: {}", binding_id);
    }
}

/// Force reset the 'pressed' state of all raw bindings.
/// This is used when an operation is cancelled (e.g. via Escape) to ensure
/// that subsequent presses are correctly detected as new presses, even if
/// the physical key release was missed or already processed.
pub fn force_reset_pressed_state() {
    let state = get_listener_state();
    if let Ok(mut guard) = state.lock() {
        for val in guard.pressed_state.values_mut() {
            *val = false;
        }
        // Also clear invalid timestamps to avoid stuck PTT logic
        guard.press_timestamps.clear();
        debug!("[RESET] Forced reset of all raw binding pressed states");
    }
}

/// rdev callback for handling keyboard events
fn rdev_callback(event: Event) {
    match event.event_type {
        // Track Shift key state
        EventType::KeyPress(k) => {
            debug!("[KEY-RAW] Press: {:?}", k);
            match k {
                Key::ShiftLeft | Key::ShiftRight => {
                    debug!("[KEY] Shift pressed");
                    if let Ok(mut guard) = get_listener_state().lock() {
                        guard.shift_pressed = true;
                    }
                }
                Key::Alt | Key::AltGr => {
                    debug!("[KEY] Alt/Option pressed");
                    if let Ok(mut guard) = get_listener_state().lock() {
                        guard.alt_pressed = true;
                    }
                    if k == Key::Alt {
                        let shift_held = get_listener_state()
                            .lock()
                            .map(|g| g.shift_pressed)
                            .unwrap_or(false);
                        debug!("[KEY] Left Option PRESSED (shift_held={})", shift_held);
                        if shift_held {
                            handle_modifier_event(
                                RAW_BINDING_SHIFT_LEFT_OPTION,
                                ModifierKeyState::Pressed,
                            );
                        } else {
                            handle_modifier_event(
                                RAW_BINDING_LEFT_OPTION,
                                ModifierKeyState::Pressed,
                            );
                        }
                    } else {
                        let shift_held = get_listener_state()
                            .lock()
                            .map(|g| g.shift_pressed)
                            .unwrap_or(false);
                        debug!("[KEY] Right Option PRESSED (shift_held={})", shift_held);
                        if shift_held {
                            handle_modifier_event(
                                RAW_BINDING_SHIFT_RIGHT_OPTION,
                                ModifierKeyState::Pressed,
                            );
                        } else {
                            handle_modifier_event(
                                RAW_BINDING_RIGHT_OPTION,
                                ModifierKeyState::Pressed,
                            );
                        }
                    }
                }
                Key::ControlLeft | Key::ControlRight => {
                    debug!("[KEY] Control pressed");
                    if let Ok(mut guard) = get_listener_state().lock() {
                        guard.ctrl_pressed = true;
                    }
                }
                Key::MetaLeft | Key::MetaRight => {
                    debug!("[KEY] Command/Meta pressed");
                    if let Ok(mut guard) = get_listener_state().lock() {
                        guard.meta_pressed = true;
                    }
                    if k == Key::MetaLeft {
                        let shift_held = get_listener_state()
                            .lock()
                            .map(|g| g.shift_pressed)
                            .unwrap_or(false);
                        debug!("[KEY] Left Command PRESSED (shift_held={})", shift_held);
                        if shift_held {
                            handle_modifier_event(
                                RAW_BINDING_SHIFT_LEFT_COMMAND,
                                ModifierKeyState::Pressed,
                            );
                        } else {
                            handle_modifier_event(
                                RAW_BINDING_LEFT_COMMAND,
                                ModifierKeyState::Pressed,
                            );
                        }
                    } else {
                        let shift_held = get_listener_state()
                            .lock()
                            .map(|g| g.shift_pressed)
                            .unwrap_or(false);
                        debug!("[KEY] Right Command PRESSED (shift_held={})", shift_held);
                        if shift_held {
                            handle_modifier_event(
                                RAW_BINDING_SHIFT_RIGHT_COMMAND,
                                ModifierKeyState::Pressed,
                            );
                        } else {
                            handle_modifier_event(
                                RAW_BINDING_RIGHT_COMMAND,
                                ModifierKeyState::Pressed,
                            );
                        }
                    }
                }
                // Handle passive hotkeys (S for Screenshot, P for Pause, Escape for Cancel)
                Key::Escape | Key::KeyS | Key::KeyP => {
                    handle_passive_key(k);
                }
                _ => {}
            }
        }
        EventType::KeyRelease(k) => {
            debug!("[KEY-RAW] Release: {:?}", k);
            match k {
                Key::ShiftLeft | Key::ShiftRight => {
                    debug!("[KEY] Shift released");
                    if let Ok(mut guard) = get_listener_state().lock() {
                        guard.shift_pressed = false;
                    }
                }
                Key::Alt | Key::AltGr => {
                    debug!("[KEY] Alt/Option released");
                    if let Ok(mut guard) = get_listener_state().lock() {
                        guard.alt_pressed = false;
                    }
                    if k == Key::Alt {
                        let shift_held = get_listener_state()
                            .lock()
                            .map(|g| g.shift_pressed)
                            .unwrap_or(false);
                        if shift_held {
                            handle_modifier_event(
                                RAW_BINDING_SHIFT_LEFT_OPTION,
                                ModifierKeyState::Released,
                            );
                        } else {
                            handle_modifier_event(
                                RAW_BINDING_LEFT_OPTION,
                                ModifierKeyState::Released,
                            );
                        }
                    } else {
                        let shift_held = get_listener_state()
                            .lock()
                            .map(|g| g.shift_pressed)
                            .unwrap_or(false);
                        if shift_held {
                            handle_modifier_event(
                                RAW_BINDING_SHIFT_RIGHT_OPTION,
                                ModifierKeyState::Released,
                            );
                        } else {
                            handle_modifier_event(
                                RAW_BINDING_RIGHT_OPTION,
                                ModifierKeyState::Released,
                            );
                        }
                    }
                }
                Key::ControlLeft | Key::ControlRight => {
                    debug!("[KEY] Control released");
                    if let Ok(mut guard) = get_listener_state().lock() {
                        guard.ctrl_pressed = false;
                    }
                }
                Key::MetaLeft | Key::MetaRight => {
                    debug!("[KEY] Command/Meta released");
                    if let Ok(mut guard) = get_listener_state().lock() {
                        guard.meta_pressed = false;
                    }
                    if k == Key::MetaLeft {
                        let shift_held = get_listener_state()
                            .lock()
                            .map(|g| g.shift_pressed)
                            .unwrap_or(false);
                        if shift_held {
                            handle_modifier_event(
                                RAW_BINDING_SHIFT_LEFT_COMMAND,
                                ModifierKeyState::Released,
                            );
                        } else {
                            handle_modifier_event(
                                RAW_BINDING_LEFT_COMMAND,
                                ModifierKeyState::Released,
                            );
                        }
                    } else {
                        let shift_held = get_listener_state()
                            .lock()
                            .map(|g| g.shift_pressed)
                            .unwrap_or(false);
                        if shift_held {
                            handle_modifier_event(
                                RAW_BINDING_SHIFT_RIGHT_COMMAND,
                                ModifierKeyState::Released,
                            );
                        } else {
                            handle_modifier_event(
                                RAW_BINDING_RIGHT_COMMAND,
                                ModifierKeyState::Released,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

/// Handle Escape, S and P keys while an operation is active
fn handle_passive_key(key: Key) {
    let app_handle = {
        let guard = match get_listener_state().lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.app_handle.clone()
    };

    if let Some(app) = app_handle {
        let audio_manager = app.state::<Arc<AudioRecordingManager>>();
        // Check if recording or paused
        let is_active =
            audio_manager.is_recording() || audio_manager.get_paused_binding_id().is_some();

        if is_active {
            match key {
                Key::Escape => {
                    info!("[RAW] Cancel triggered via Escape");
                    crate::utils::cancel_current_operation(&app);
                }
                Key::KeyS => {
                    // Vision capture - only if a modifier is held to avoid accidental triggers while typing
                    if let Ok(guard) = get_listener_state().lock() {
                        if guard.shift_pressed
                            || guard.alt_pressed
                            || guard.ctrl_pressed
                            || guard.meta_pressed
                        {
                            info!("[RAW] Vision capture triggered via S + Modifier");
                            let app_clone = app.clone();
                            tauri::async_runtime::spawn(async move {
                                match crate::vision::capture_screen() {
                                    Ok(base64) => {
                                        let audio_manager =
                                            app_clone.state::<Arc<AudioRecordingManager>>();
                                        audio_manager.add_vision_context(base64);
                                        let _ = app_clone.emit("vision-captured", ());
                                    }
                                    Err(e) => error!("Vision capture failed: {}", e),
                                }
                            });
                        }
                    }
                }
                Key::KeyP => {
                    // Pause toggle - only if a modifier is held
                    if let Ok(guard) = get_listener_state().lock() {
                        if guard.shift_pressed
                            || guard.alt_pressed
                            || guard.ctrl_pressed
                            || guard.meta_pressed
                        {
                            info!("[RAW] Pause toggle triggered via P + Modifier");
                            let app_clone = app.clone();
                            tauri::async_runtime::spawn(async move {
                                crate::utils::toggle_pause_operation(&app_clone);
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Handle a modifier key event with smart tap/hold detection
fn handle_modifier_event(binding_string: &str, key_state: ModifierKeyState) {
    debug!(
        "[HANDLER] handle_modifier_event called: binding='{}' key_state={:?}",
        binding_string, key_state
    );

    let state = get_listener_state();
    let (app_handle, binding_id, should_process, press_time) = {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(e) => {
                error!("Failed to lock listener state: {}", e);
                return;
            }
        };

        let binding = match guard.bindings.get(binding_string) {
            Some(b) => b.clone(),
            None => {
                // FALLBACK: if "shift+modifier" is not registered, try the base "modifier"
                if binding_string.starts_with("shift+") {
                    let base_binding = &binding_string[6..];
                    match guard.bindings.get(base_binding) {
                        Some(b) => b.clone(),
                        None => {
                            debug!(
                                "[HANDLER] Binding '{}' (and fallback '{}') not registered, skipping",
                                binding_string, base_binding
                            );
                            return;
                        }
                    }
                } else {
                    debug!(
                        "[HANDLER] Binding '{}' not registered, skipping",
                        binding_string
                    );
                    return; // Not registered
                }
            }
        };

        // Check if suspended
        if guard.suspended.contains(&binding.binding_id) {
            debug!(
                "[HANDLER] Ignoring {} event for suspended binding {}",
                binding_string, binding.binding_id
            );
            return;
        }

        // Track pressed state to avoid duplicate events
        let was_pressed = *guard.pressed_state.get(binding_string).unwrap_or(&false);
        let is_now_pressed = key_state == ModifierKeyState::Pressed;

        debug!(
            "[HANDLER] pressed_state check: binding='{}' was_pressed={} is_now_pressed={}",
            binding_string, was_pressed, is_now_pressed
        );

        if was_pressed == is_now_pressed {
            // This is normally a duplicate event to filter out.
            // HOWEVER: if this is a Release and the toggle is still active,
            // it means we missed the Press that should have triggered stop.
            // In this case, we should stop anyway as a fallback.
            if !is_now_pressed {
                // This is a Release event being filtered - check for active toggle
                // Clone what we need first, then drop the guard before checking toggle
                if let Some(app) = guard.app_handle.clone() {
                    let binding_id = binding.binding_id.clone();
                    let binding_str = binding_string.to_string();
                    drop(guard);

                    use crate::actions::ACTION_MAP;
                    use crate::ManagedToggleState;
                    use tauri::Manager;

                    let toggle_state_manager = app.state::<ManagedToggleState>();
                    let is_active = toggle_state_manager
                        .lock()
                        .ok()
                        .and_then(|s| s.active_toggles.get(&binding_id).copied())
                        .unwrap_or(false);

                    if is_active {
                        warn!(
                            "[HANDLER] FALLBACK STOP: Release filtered but toggle is active for '{}' - stopping anyway",
                            binding_id
                        );
                        if let Some(action) = ACTION_MAP.get(&binding_id) {
                            action.stop(&app, &binding_id, &binding_str);
                        }
                    }
                    return;
                }
            }

            debug!(
                "[HANDLER] FILTERING as duplicate: binding='{}' was_pressed={} is_now_pressed={}",
                binding_string, was_pressed, is_now_pressed
            );
            return; // No state change
        }

        guard
            .pressed_state
            .insert(binding_string.to_string(), is_now_pressed);
        debug!(
            "[HANDLER] Updated pressed_state['{}'] = {}",
            binding_string, is_now_pressed
        );

        // Track press timestamp for tap vs hold detection
        let press_time = if is_now_pressed {
            // Starting press - record timestamp
            let now = std::time::Instant::now();
            guard
                .press_timestamps
                .insert(binding_string.to_string(), now);
            None
        } else {
            // Releasing - get the press timestamp
            guard.press_timestamps.remove(binding_string)
        };

        (
            guard.app_handle.clone(),
            binding.binding_id.clone(),
            true,
            press_time,
        )
    };

    if !should_process {
        return;
    }

    let app = match app_handle {
        Some(a) => a,
        None => {
            warn!("No app handle available for modifier key event");
            return;
        }
    };

    // Trigger the action using smart tap/hold detection
    use crate::actions::ACTION_MAP;
    use crate::ManagedToggleState;
    use tauri::Manager;

    if let Some(action) = ACTION_MAP.get(&binding_id) {
        match key_state {
            ModifierKeyState::Pressed => {
                debug!(
                    "[TOGGLE] Processing PRESSED event for binding_id='{}'",
                    binding_id
                );
                // Always start on press
                let toggle_state_manager = app.state::<ManagedToggleState>();
                {
                    let mut states = toggle_state_manager
                        .lock()
                        .expect("Failed to lock toggle state manager");
                    let is_active = states
                        .active_toggles
                        .entry(binding_id.clone())
                        .or_insert(false);

                    debug!(
                        "[TOGGLE] Current active_toggles['{}'] = {}",
                        binding_id, *is_active
                    );

                    if *is_active {
                        // Already recording - this is a toggle-off tap
                        debug!(
                            "[TOGGLE] Raw binding {} toggle stop (tap while active)",
                            binding_string
                        );
                        drop(states); // Release lock before action
                        action.stop(&app, &binding_id, binding_string);
                        return;
                    }

                    // Start recording
                    *is_active = true;
                    debug!(
                        "[TOGGLE] Setting active_toggles['{}'] = true (starting recording)",
                        binding_id
                    );
                }
                debug!(
                    "[TOGGLE] Raw binding {} start recording - calling action.start()",
                    binding_string
                );
                let started = action.start(&app, &binding_id, binding_string);
                debug!("[TOGGLE] action.start() returned: {}", started);

                // If start failed, reset the toggle state
                if !started {
                    debug!(
                        "[TOGGLE] action.start() returned false, resetting active_toggles['{}'] = false",
                        binding_id
                    );
                    let toggle_state_manager = app.state::<ManagedToggleState>();
                    if let Ok(mut states) = toggle_state_manager.lock() {
                        states.active_toggles.insert(binding_id.clone(), false);
                    };
                } else {
                    // Successfully started recording - spawn a timer to emit "hold" mode after threshold
                    // This allows the "Raw" label to appear while user is still holding
                    use crate::settings::get_settings;
                    let settings = get_settings(&app);
                    let threshold = settings.hold_threshold_ms as u64;
                    let app_clone = app.clone();
                    let binding_id_clone = binding_id.clone();
                    let binding_string_clone = binding_string.to_string();

                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(threshold));

                        // Check if still physically pressed AND recording is still active
                        let is_still_physically_pressed = get_listener_state()
                            .lock()
                            .ok()
                            .map(|s| s.press_timestamps.contains_key(&binding_string_clone))
                            .unwrap_or(false);

                        let toggle_state_manager = app_clone.state::<ManagedToggleState>();
                        let is_still_active = toggle_state_manager
                            .lock()
                            .ok()
                            .and_then(|s| s.active_toggles.get(&binding_id_clone).copied())
                            .unwrap_or(false);

                        if is_still_physically_pressed && is_still_active {
                            // User has been holding for threshold ms - this is "hold" mode
                            use crate::overlay;
                            debug!("[TOGGLE] Threshold passed while still holding - emitting hold mode");
                            overlay::emit_mode_determined(&app_clone, "hold");
                        }
                    });
                }
            }
            ModifierKeyState::Released => {
                debug!(
                    "[TOGGLE] Processing RELEASED event for binding_id='{}'",
                    binding_id
                );
                // Check how long the key was held
                let hold_duration_ms = press_time.map(|t| t.elapsed().as_millis()).unwrap_or(0);

                // Get threshold from settings
                use crate::settings::get_settings;
                let settings = get_settings(&app);
                let threshold = settings.hold_threshold_ms as u128;

                debug!(
                    "[TOGGLE] hold_duration={}ms threshold={}ms",
                    hold_duration_ms, threshold
                );

                if hold_duration_ms >= threshold {
                    // Long hold - PTT behavior, stop immediately
                    let toggle_state_manager = app.state::<ManagedToggleState>();
                    {
                        let mut states = toggle_state_manager
                            .lock()
                            .expect("Failed to lock toggle state manager");
                        debug!(
                            "[TOGGLE] PTT mode: setting active_toggles['{}'] = false",
                            binding_id
                        );
                        states.active_toggles.insert(binding_id.clone(), false);
                    }
                    debug!(
                        "[TOGGLE] Raw binding {} released after {}ms (PTT stop) - calling action.stop()",
                        binding_string, hold_duration_ms
                    );

                    // Emit hold mode so UI can show "Raw" briefly before transitioning
                    use crate::overlay;
                    overlay::emit_mode_determined(&app, "hold");

                    action.stop(&app, &binding_id, binding_string);
                } else {
                    // Quick tap - toggle mode.
                    // CRITICAL: Only emit if we are still active (i.e. this was the START tap).
                    // If we just stopped on Pressed, active_toggles will be false now.
                    let is_still_active = {
                        let toggle_state_manager = app.state::<ManagedToggleState>();
                        let states = toggle_state_manager
                            .lock()
                            .expect("Failed to lock toggle state manager");
                        *states.active_toggles.get(&binding_id).unwrap_or(&false)
                    };

                    debug!(
                        "[TOGGLE] Raw binding {} quick released (duration={}ms). is_still_active={}",
                        binding_string, hold_duration_ms, is_still_active
                    );

                    if is_still_active {
                        // Quick press = coherent mode (unified hotkey UX)
                        let audio_manager = app.state::<Arc<AudioRecordingManager>>();
                        audio_manager.set_coherent_mode(true);

                        // Emit refining mode and update overlay SYNCHRONOUSLY (so UI updates immediately)
                        use crate::overlay;
                        crate::utils::show_ramble_recording_overlay(&app);
                        overlay::emit_mode_determined(&app, "refining");

                        // Spawn async ONLY for clipboard copy (blocks rdev if done synchronously)
                        let app_clone = app.clone();
                        let audio_manager_clone = Arc::clone(&audio_manager);
                        // Run on main thread to prevent crash on macOS (TSM/Enigo requirements)
                        let _ = app.run_on_main_thread(move || {
                            // Capture selection context for coherent processing
                            if let Ok(Some(text)) = crate::clipboard::get_selected_text(&app_clone)
                            {
                                debug!("Captured selection context: {} chars", text.len());
                                audio_manager_clone.set_selection_context(text);
                            }
                        });
                    }
                }
            }
        }
    } else {
        warn!(
            "No action defined in ACTION_MAP for raw binding ID '{}'",
            binding_id
        );
    }
}
