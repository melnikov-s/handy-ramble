use std::sync::Arc;
use std::thread;
use std::time;

use rdev::{simulate, EventType, Key, SimulateError};
use serde::Deserialize;
use serde::Serialize;
use tauri::App;
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};
use tauri_plugin_store::{JsonValue, StoreExt};

use crate::managers::audio::AudioRecordingManager;
use crate::managers::transcription::TranscriptionManager;

#[derive(Serialize, Deserialize, Debug, Clone)] // Clone is useful
pub struct ShortcutBinding {
    id: String,
    name: String,
    description: String,
    default_binding: String,
    current_binding: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppSettings {
    bindings: Vec<ShortcutBinding>,
}

fn get_default_settings() -> AppSettings {
    AppSettings {
        bindings: vec![
            ShortcutBinding {
                id: "transcribe".to_string(),
                name: "Transcribe".to_string(),
                description: "Converts your speech into text.".to_string(),
                default_binding: "alt+space".to_string(),
                current_binding: "alt+space".to_string(),
            },
            ShortcutBinding {
                id: "test".to_string(),
                name: "Test".to_string(),
                description: "This is a test binding.".to_string(),
                default_binding: "ctrl+d".to_string(),
                current_binding: "ctrl+d".to_string(),
            },
        ],
    }
}

fn try_send_event(event: &EventType) {
    if let Err(SimulateError) = simulate(event) {
        println!("We could not send {:?}", event);
    }
}

fn send(event: EventType) {
    try_send_event(&event);
    thread::sleep(time::Duration::from_millis(60));
}

fn send_paste() {
    // Determine the modifier key based on the OS
    #[cfg(target_os = "macos")]
    let modifier_key = Key::MetaLeft; // Command key on macOS
    #[cfg(not(target_os = "macos"))]
    let modifier_key = Key::ControlLeft; // Control key on other systems

    // Press both keys
    send(EventType::KeyPress(modifier_key));
    send(EventType::KeyPress(Key::KeyV));

    // Release both keys
    send(EventType::KeyRelease(Key::KeyV));
    send(EventType::KeyRelease(modifier_key));
}

fn paste(text: String, app_handle: AppHandle) {
    let clipboard = app_handle.clipboard();

    // get the current clipboard content
    let clipboard_content = clipboard.read_text().unwrap_or_default();

    clipboard.write_text(&text).unwrap();
    send_paste();

    // restore the clipboard
    clipboard.write_text(&clipboard_content).unwrap();
}

// const HANDY_TAURI_STORE: &str = "handy_tauri_store";

// let mut shortcut_map: HashMap<String, fn()> = HashMap::new();

fn transcribe_pressed(app: &AppHandle) {
    let rm = app.state::<Arc<AudioRecordingManager>>();
    rm.try_start_recording("transcribe");
}

fn transcribe_released(app: &AppHandle) {
    let ah = app.clone();
    let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
    let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());

    tauri::async_runtime::spawn(async move {
        if let Some(samples) = rm.stop_recording("transcribe") {
            match tm.transcribe(samples) {
                // Not .await, as transcribe is synchronous
                Ok(transcription) => {
                    println!("Global Shortcut Transcription: {}", transcription);
                    paste(transcription, ah);
                }
                Err(err) => println!("Global Shortcut Transcription error: {}", err),
            }
        }
    });
}

pub fn init_shortcuts(app: &App) {
    // init store
    let kb_store = app
        .store("settings_store.json")
        .expect("Failed to initialize store");

    if let Some(bindings) = kb_store.get("settings") {
        // print the bindings that exist
        println!("Bindings: {:?}", bindings);
    } else {
        kb_store.set(
            "settings",
            serde_json::to_value(&get_default_settings()).unwrap(),
        );
        // create the default bindings
    }

    // load state from store

    _register_shortcut_upon_start(
        app,
        "alt+space"
            .parse::<Shortcut>()
            .expect("Failed to parse shortcut"),
    );
}

fn _register_shortcut_upon_start(app: &App, shortcut: Shortcut) {
    // Initialize global shortcut and set its handler
    app.handle()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |handler_app, scut, event| {
                    if scut == &shortcut {
                        println!("Global Shortcut pressed! {}", scut.into_string());
                        if event.state == ShortcutState::Pressed {
                            transcribe_pressed(handler_app);
                        } else if event.state == ShortcutState::Released {
                            transcribe_released(handler_app);
                        }
                    }
                })
                .build(),
        )
        .unwrap();
    app.global_shortcut().register(shortcut).unwrap(); // Register global shortcut
}
