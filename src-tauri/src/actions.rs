use crate::managers::audio::AudioRecordingManager;
use crate::managers::transcription::TranscriptionManager;
use crate::utils;
use crate::utils::change_tray_icon;
use crate::utils::TrayIconState;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tauri::Manager;

// Action Handler Types
pub type PressAction = fn(app: &AppHandle, shortcut_str: &str);
pub type ReleaseAction = fn(app: &AppHandle, shortcut_str: &str);

// TODO refactor to start and stop
pub struct ActionSet {
    pub press: PressAction,
    pub release: ReleaseAction,
}

// Handler Functions
fn transcribe_pressed_action(app: &AppHandle, _shortcut_str: &str) {
    change_tray_icon(app, TrayIconState::Recording);

    let rm = app.state::<Arc<AudioRecordingManager>>();
    rm.try_start_recording("transcribe");
}

fn transcribe_released_action(app: &AppHandle, _shortcut_str: &str) {
    let ah = app.clone();
    let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
    let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());

    change_tray_icon(app, TrayIconState::Idle);

    tauri::async_runtime::spawn(async move {
        if let Some(samples) = rm.stop_recording("transcribe") {
            match tm.transcribe(samples) {
                Ok(transcription) => {
                    println!("Global Shortcut Transcription: {}", transcription);
                    utils::paste(transcription, ah);
                }
                Err(err) => println!("Global Shortcut Transcription error: {}", err),
            }
        }
    });
}

fn test_binding_pressed_action(app: &AppHandle, shortcut_str: &str) {
    println!(
        "Shortcut ID 'test': Pressed - {} (App: {})",
        shortcut_str,
        app.package_info().name
    );
}

fn test_binding_released_action(app: &AppHandle, shortcut_str: &str) {
    println!(
        "Shortcut ID 'test': Released - {} (App: {})",
        shortcut_str,
        app.package_info().name
    );
}

// Static Action Map
pub static ACTION_MAP: Lazy<HashMap<String, ActionSet>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "transcribe".to_string(),
        ActionSet {
            press: transcribe_pressed_action,
            release: transcribe_released_action,
        },
    );
    map.insert(
        "test".to_string(),
        ActionSet {
            press: test_binding_pressed_action,
            release: test_binding_released_action,
        },
    );
    map
});
