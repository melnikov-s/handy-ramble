use super::audio::AudioRecordingManager;
use super::transcription::TranscriptionManager;
use rdev::EventType;
use rig::agent::Agent;
use rig::providers::anthropic;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

type KeySet = HashSet<rdev::Key>;

#[derive(Clone)]
pub struct BindingContext {
    pub recording_manager: Arc<AudioRecordingManager>,
    pub transcription_manager: Arc<TranscriptionManager>,
    pub sonnet: Arc<Agent<anthropic::completion::CompletionModel>>,
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
    currently_pressed: Arc<Mutex<KeySet>>,
}

impl KeyBinding {
    fn new<F, G>(id: String, keys: Vec<rdev::Key>, on_press: F, on_release: G) -> Self
    where
        F: Fn(&BindingContext) -> Option<tauri::async_runtime::JoinHandle<()>> + Send + 'static,
        G: Fn(&BindingContext) -> Option<tauri::async_runtime::JoinHandle<()>> + Send + 'static,
    {
        Self {
            id,
            keys: keys.into_iter().collect(),
            on_press: Box::new(on_press),
            on_release: Box::new(on_release),
            currently_pressed: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn handle_event(&self, key: rdev::Key, is_press: bool, context: &BindingContext) -> bool {
        let mut pressed = self.currently_pressed.lock().unwrap();

        if is_press {
            if !self.keys.contains(&key) {
                return false; // Ignore keys that aren't part of this binding
            }

            if pressed.contains(&key) {
                return false;
            }

            pressed.insert(key);
            if pressed.len() == self.keys.len() && pressed.is_subset(&self.keys) {
                let _ = (self.on_press)(context);
                return true;
            }
        } else {
            // Release event
            if !self.keys.contains(&key) {
                return false;
            }

            if pressed.remove(&key) {
                if pressed.len() == self.keys.len() - 1 {
                    let _ = (self.on_release)(context);

                    // Clear the currently_pressed set if all keys are released
                    if pressed.is_empty() {
                        pressed.clear();
                    }

                    return true;
                }
            }
        }
        false
    }
}

pub struct KeyBindingManager {
    bindings: Vec<KeyBinding>,
    context: BindingContext,
}

impl KeyBindingManager {
    pub fn new(
        recording_manager: Arc<AudioRecordingManager>,
        transcription_manager: Arc<TranscriptionManager>,
        sonnet: Arc<Agent<anthropic::completion::CompletionModel>>,
    ) -> Self {
        Self {
            bindings: Vec::new(),
            context: BindingContext {
                recording_manager,
                transcription_manager,
                sonnet,
            },
        }
    }

    pub fn register<F, G>(&mut self, id: String, keys: Vec<rdev::Key>, on_press: F, on_release: G)
    where
        F: Fn(&BindingContext) -> Option<tauri::async_runtime::JoinHandle<()>> + Send + 'static,
        G: Fn(&BindingContext) -> Option<tauri::async_runtime::JoinHandle<()>> + Send + 'static,
    {
        self.bindings
            .push(KeyBinding::new(id, keys, on_press, on_release));
    }

    pub fn handle_event(&self, event: &rdev::Event) {
        if let EventType::KeyPress(key) | EventType::KeyRelease(key) = event.event_type {
            let is_press = matches!(event.event_type, EventType::KeyPress(_));

            // Only process the first binding that successfully handles the event
            for binding in &self.bindings {
                if binding.handle_event(key, is_press, &self.context) {
                    break; // Exit after first successful handling
                }
            }
        }
    }
}
