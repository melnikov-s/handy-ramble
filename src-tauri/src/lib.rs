mod managers;

use log::{info, Level};
use managers::keybinding::KeyBindingManager;
use managers::transcription::TranscriptionManager;
use managers::{audio::AudioRecordingManager, transcription};
use rdev::Key;
use rig::{completion::Prompt, providers::anthropic};
use std::sync::{Arc, Mutex};
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let recording_manager =
        Arc::new(AudioRecordingManager::new().expect("Failed to initialize recording manager"));
    let transcription_manager =
        Arc::new(TranscriptionManager::new().expect("Failed to initialize transcription manager"));
    // let transcription_manager = Arc::new(TranscriptionManager::new());
    let claude_client = anthropic::Client::from_env();
    let sonnet: Arc<rig::agent::Agent<anthropic::completion::CompletionModel>> = Arc::new(
        claude_client
            .agent(anthropic::CLAUDE_3_5_SONNET)
            .preamble("Be precise and concise.")
            .temperature(0.5)
            .build(),
    );

    let manager = Arc::new(Mutex::new(KeyBindingManager::new(
        recording_manager.clone(),
        transcription_manager.clone(),
        sonnet.clone(),
    )));

    // Register your key bindings
    {
        let mut manager = manager.lock().unwrap();

        // Register Basic Transcription
        manager.register(
            "ctrl-meta".to_string(),
            vec![Key::ControlRight, Key::MetaRight],
            |ctx| {
                info!("Ctrl+Meta pressed!");
                ctx.recording_manager.try_start_recording("ctrl-meta");
                None
            },
            |ctx| {
                info!("release being called from ctrl-meta");
                let ctx = ctx.clone();
                Some(tauri::async_runtime::spawn(async move {
                    if let Some(samples) = ctx.recording_manager.stop_recording("ctrl-meta") {
                        match ctx.transcription_manager.transcribe(samples) {
                            Ok(transcription) => println!("Transcription: {}", transcription),
                            Err(err) => println!("Transcription error: {}", err),
                        }
                    }
                }))
            },
        );

        // Register LLM Call after Transcription
        manager.register(
            "shift-alt".to_string(),
            vec![Key::ShiftLeft, Key::Alt],
            |ctx| {
                info!("Shift+Alt pressed!");
                ctx.recording_manager.try_start_recording("shift-alt");
                None
            },
            |ctx| {
                info!("release being called from shift-alt");
                let ctx = ctx.clone();
                Some(tauri::async_runtime::spawn(async move {
                    if let Some(samples) = ctx.recording_manager.stop_recording("shift-alt") {
                        if let Ok(transcription) = ctx.transcription_manager.transcribe(samples) {
                            println!("Transcription: {}", transcription);
                            match ctx.sonnet.prompt(transcription).await {
                                Ok(response) => println!("Sonnet response: {}", response),
                                Err(err) => println!("Sonnet error: {}", err),
                            }
                        }
                    }
                }))
            },
        );

        manager.register(
            "ctrl-alt-meta".to_string(),
            vec![Key::ControlLeft, Key::Alt, Key::MetaLeft],
            |ctx| {
                info!("Ctrl+Alt+Meta pressed!");
                ctx.recording_manager.try_start_recording("ctrl-alt-meta");
                None
            },
            |ctx| {
                info!("release being called from ctrl-alt-meta");
                let ctx = ctx.clone();
                Some(tauri::async_runtime::spawn(async move {
                    if let Some(samples) = ctx.recording_manager.stop_recording("ctrl-alt-meta") {
                        let samples: Vec<f32> = samples; // explicit type annotation
                        match ctx.transcription_manager.transcribe(samples) {
                            Ok(transcription) => {
                                println!("Transcription: {}", transcription);
                                // Call LLM for code
                            }
                            Err(err) => println!("Transcription error: {}", err),
                        }
                    } else {
                        println!("No samples recorded");
                    }
                }))
            },
        );
    }

    tauri::async_runtime::spawn({
        let manager = manager.clone();
        async move {
            rdev::listen(move |event| {
                if let Ok(manager) = manager.lock() {
                    manager.handle_event(&event);
                }
            })
            .unwrap();
        }
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--auto-launch"]),
        ))
        .plugin(tauri_plugin_macos_permissions::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
