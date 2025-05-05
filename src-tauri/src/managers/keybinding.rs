use super::audio::AudioRecordingManager;
use super::transcription::TranscriptionManager;
use rdev::{EventType, Key};
use rig::providers::anthropic;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

type KeySet = HashSet<Key>;

#[derive(Clone)]
pub struct BindingContext {
    pub recording_manager: Arc<AudioRecordingManager>,
    pub transcription_manager: Arc<TranscriptionManager>,
    // pub anthropic: Arc<anthropic::Client>,
    pub app_handle: tauri::AppHandle,
}

pub struct KeyBinding {
    id: String,
    keys: KeySet,
    on_press: Box<
        dyn Fn(&BindingContext) -> Option<tauri::async_runtime::JoinHandle<()>> + Send + 'static,
    >,
    on_release: Box<
        dyn Fn(&BindingContext) -> Option<tauri::async_runtime::JoinHandle<()>> + Send + 'static,
    >,
}

impl KeyBinding {
    fn new<F, G>(id: String, keys: Vec<Key>, on_press: F, on_release: G) -> Self
    where
        F: Fn(&BindingContext) -> Option<tauri::async_runtime::JoinHandle<()>> + Send + 'static,
        G: Fn(&BindingContext) -> Option<tauri::async_runtime::JoinHandle<()>> + Send + 'static,
    {
        Self {
            id,
            keys: keys.into_iter().collect(),
            on_press: Box::new(on_press),
            on_release: Box::new(on_release),
        }
    }
}

pub struct KeyBindingManager {
    bindings: Vec<KeyBinding>,
    global_pressed: Arc<Mutex<HashSet<Key>>>,
    active_bindings: Arc<Mutex<HashSet<String>>>,
    context: BindingContext,
}

impl KeyBindingManager {
    pub fn new(
        recording_manager: Arc<AudioRecordingManager>,
        transcription_manager: Arc<TranscriptionManager>,
        // anthropic: Arc<anthropic::Client>,
        app_handle: tauri::AppHandle,
    ) -> Self {
        Self {
            bindings: Vec::new(),
            global_pressed: Arc::new(Mutex::new(HashSet::new())),
            active_bindings: Arc::new(Mutex::new(HashSet::new())),
            context: BindingContext {
                recording_manager,
                transcription_manager,
                // anthropic,
                app_handle,
            },
        }
    }

    pub fn register<F, G>(&mut self, id: String, keys: Vec<Key>, on_press: F, on_release: G)
    where
        F: Fn(&BindingContext) -> Option<tauri::async_runtime::JoinHandle<()>> + Send + 'static,
        G: Fn(&BindingContext) -> Option<tauri::async_runtime::JoinHandle<()>> + Send + 'static,
    {
        self.bindings
            .push(KeyBinding::new(id, keys, on_press, on_release));
    }

    pub fn handle_event(&self, event: &rdev::Event) {
        if let EventType::KeyPress(key) | EventType::KeyRelease(key) = event.event_type {
            let mut is_press = matches!(event.event_type, EventType::KeyPress(_));

            // Update global pressed keys
            let mut pressed = self.global_pressed.lock().unwrap();

            // If we get a release event for a key that wasn't pressed,
            // treat it as a press event instead
            if !is_press && !pressed.contains(&key) {
                is_press = true;
            }

            if is_press {
                pressed.insert(key);
            } else {
                pressed.remove(&key);
            }
            let pressed_snapshot = pressed.clone();
            drop(pressed);

            // Check binding states
            let mut active = self.active_bindings.lock().unwrap();
            for binding in &self.bindings {
                let is_active = active.contains(&binding.id);
                let all_required_pressed = binding.keys.is_subset(&pressed_snapshot);
                let any_required_key = binding.keys.contains(&key);

                // Only process if this event is for a key we care about
                if any_required_key {
                    match (all_required_pressed, is_active) {
                        (true, false) => {
                            (binding.on_press)(&self.context);
                            active.insert(binding.id.clone());
                        }
                        (false, true) => {
                            // Only trigger release if none of the required keys are pressed
                            let any_binding_key_pressed =
                                binding.keys.iter().any(|k| pressed_snapshot.contains(k));

                            if !any_binding_key_pressed {
                                (binding.on_release)(&self.context);
                                active.remove(&binding.id);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
