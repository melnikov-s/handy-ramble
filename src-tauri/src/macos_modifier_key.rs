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
//! - macOS Accessibility permission (already required by Handy for paste functionality)

use log::{debug, error, info, warn};
use rdev::{listen, Event, EventType, Key};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::AppHandle;

/// Binding identifiers for raw modifier shortcuts
pub const RAW_BINDING_RIGHT_OPTION: &str = "right_option";
pub const RAW_BINDING_LEFT_OPTION: &str = "left_option";
pub const RAW_BINDING_SHIFT_RIGHT_OPTION: &str = "shift+right_option";
pub const RAW_BINDING_SHIFT_LEFT_OPTION: &str = "shift+left_option";

/// Check if a binding string is a raw modifier binding (macOS-only)
pub fn is_raw_modifier_binding(binding: &str) -> bool {
    matches!(
        binding,
        RAW_BINDING_RIGHT_OPTION
            | RAW_BINDING_LEFT_OPTION
            | RAW_BINDING_SHIFT_RIGHT_OPTION
            | RAW_BINDING_SHIFT_LEFT_OPTION
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
    /// App handle for triggering actions
    app_handle: Option<AppHandle>,
    /// Track if Shift is currently held (to allow Shift+Option shortcuts to work)
    shift_pressed: bool,
}

impl ModifierListenerState {
    fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            suspended: std::collections::HashSet::new(),
            pressed_state: HashMap::new(),
            app_handle: None,
            shift_pressed: false,
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

/// rdev callback for handling keyboard events
fn rdev_callback(event: Event) {
    match event.event_type {
        // Track Shift key state
        EventType::KeyPress(Key::ShiftLeft) | EventType::KeyPress(Key::ShiftRight) => {
            if let Ok(mut guard) = get_listener_state().lock() {
                guard.shift_pressed = true;
            }
        }
        EventType::KeyRelease(Key::ShiftLeft) | EventType::KeyRelease(Key::ShiftRight) => {
            if let Ok(mut guard) = get_listener_state().lock() {
                guard.shift_pressed = false;
            }
        }
        // Left Alt/Option key (rdev uses Key::Alt for left)
        EventType::KeyPress(Key::Alt) => {
            let shift_held = get_listener_state()
                .lock()
                .map(|g| g.shift_pressed)
                .unwrap_or(false);
            if shift_held {
                // Shift+Left Option combination
                handle_modifier_event(RAW_BINDING_SHIFT_LEFT_OPTION, ModifierKeyState::Pressed);
            } else {
                // Left Option only
                handle_modifier_event(RAW_BINDING_LEFT_OPTION, ModifierKeyState::Pressed);
            }
        }
        EventType::KeyRelease(Key::Alt) => {
            let shift_held = get_listener_state()
                .lock()
                .map(|g| g.shift_pressed)
                .unwrap_or(false);
            if shift_held {
                handle_modifier_event(RAW_BINDING_SHIFT_LEFT_OPTION, ModifierKeyState::Released);
            } else {
                handle_modifier_event(RAW_BINDING_LEFT_OPTION, ModifierKeyState::Released);
            }
        }
        // Right Alt/Option key (rdev reports as Key::AltGr on macOS)
        EventType::KeyPress(Key::AltGr) => {
            let shift_held = get_listener_state()
                .lock()
                .map(|g| g.shift_pressed)
                .unwrap_or(false);
            if shift_held {
                // Shift+Right Option combination
                handle_modifier_event(RAW_BINDING_SHIFT_RIGHT_OPTION, ModifierKeyState::Pressed);
            } else {
                // Right Option only
                handle_modifier_event(RAW_BINDING_RIGHT_OPTION, ModifierKeyState::Pressed);
            }
        }
        EventType::KeyRelease(Key::AltGr) => {
            let shift_held = get_listener_state()
                .lock()
                .map(|g| g.shift_pressed)
                .unwrap_or(false);
            if shift_held {
                handle_modifier_event(RAW_BINDING_SHIFT_RIGHT_OPTION, ModifierKeyState::Released);
            } else {
                handle_modifier_event(RAW_BINDING_RIGHT_OPTION, ModifierKeyState::Released);
            }
        }
        _ => {}
    }
}

/// Handle a modifier key event
fn handle_modifier_event(binding_string: &str, key_state: ModifierKeyState) {
    let state = get_listener_state();
    let (app_handle, binding_id, should_process) = {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(e) => {
                error!("Failed to lock listener state: {}", e);
                return;
            }
        };

        let binding = match guard.bindings.get(binding_string) {
            Some(b) => b.clone(),
            None => return, // Not registered
        };

        // Check if suspended
        if guard.suspended.contains(&binding.binding_id) {
            debug!(
                "Ignoring {} event for suspended binding {}",
                binding_string, binding.binding_id
            );
            return;
        }

        // Track pressed state to avoid duplicate events
        let was_pressed = *guard.pressed_state.get(binding_string).unwrap_or(&false);
        let is_now_pressed = key_state == ModifierKeyState::Pressed;

        if was_pressed == is_now_pressed {
            return; // No state change
        }

        guard
            .pressed_state
            .insert(binding_string.to_string(), is_now_pressed);

        (guard.app_handle.clone(), binding.binding_id.clone(), true)
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

    // Trigger the action using the same mechanism as regular shortcuts
    use crate::actions::ACTION_MAP;
    use crate::settings::get_settings;
    use crate::ManagedToggleState;
    use tauri::Manager;

    let settings = get_settings(&app);

    if let Some(action) = ACTION_MAP.get(&binding_id) {
        if settings.push_to_talk {
            // Push-to-talk mode: start on press, stop on release
            match key_state {
                ModifierKeyState::Pressed => {
                    debug!(
                        "Raw binding {} pressed (push-to-talk start)",
                        binding_string
                    );
                    action.start(&app, &binding_id, binding_string);
                }
                ModifierKeyState::Released => {
                    debug!(
                        "Raw binding {} released (push-to-talk stop)",
                        binding_string
                    );
                    action.stop(&app, &binding_id, binding_string);
                }
            }
        } else {
            // Toggle mode: toggle on press only
            if key_state == ModifierKeyState::Pressed {
                let toggle_state_manager = app.state::<ManagedToggleState>();
                let should_start = {
                    let mut states = toggle_state_manager
                        .lock()
                        .expect("Failed to lock toggle state manager");
                    let is_active = states
                        .active_toggles
                        .entry(binding_id.clone())
                        .or_insert(false);
                    let start = !*is_active;
                    *is_active = start;
                    start
                };

                if should_start {
                    debug!("Raw binding {} toggle start", binding_string);
                    action.start(&app, &binding_id, binding_string);
                } else {
                    debug!("Raw binding {} toggle stop", binding_string);
                    action.stop(&app, &binding_id, binding_string);
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
