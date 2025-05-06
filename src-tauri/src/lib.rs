mod managers;

use log::info;
use managers::audio::AudioRecordingManager;
use managers::transcription::TranscriptionManager;
use rdev::{simulate, EventType, Key, SimulateError};
use std::sync::{Arc, Mutex};
use std::{thread, time};
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_clipboard_manager::ClipboardExt;

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

fn paste(text: String, app_handle: tauri::AppHandle) {
    let clipboard = app_handle.clipboard();

    // get the current clipboard content
    let clipboard_content = clipboard.read_text().unwrap_or_default();

    clipboard.write_text(&text).unwrap();
    send_paste();

    // restore the clipboard
    clipboard.write_text(&clipboard_content).unwrap();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_stronghold::Builder::new(|_pass| todo!()).build())
        .plugin(tauri_plugin_upload::init())
        // .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--auto-launch"]),
        ))
        .plugin(tauri_plugin_macos_permissions::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let vad_path = app.path().resolve(
                "resources/silero_vad_v4.onnx",
                tauri::path::BaseDirectory::Resource,
            )?;

            let whisper_path = app.path().resolve(
                "resources/ggml-small.bin",
                tauri::path::BaseDirectory::Resource,
            )?;

            let recording_manager = Arc::new(
                AudioRecordingManager::new(&vad_path)
                    .expect("Failed to initialize recording manager"),
            );
            let transcription_manager = Arc::new(
                TranscriptionManager::new(
                    whisper_path
                        .to_str()
                        .expect("Path contains invalid UTF-8 Chars"),
                )
                .expect("Failed to initialize transcription manager"),
            );

            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

                // Clone resources needed for the global shortcut handler
                let rm_for_shortcut_handler = recording_manager.clone();
                let tm_for_shortcut_handler = transcription_manager.clone();

                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_shortcuts(["alt+space"])? // Register "alt+space"
                        .with_handler(move |app_handle_from_plugin, shortcut, event| {
                            // Clone Arcs for potential async tasks to move ownership
                            let recording_manager = rm_for_shortcut_handler.clone();
                            let transcription_manager = tm_for_shortcut_handler.clone();
                            let app_handle_for_async_tasks = app_handle_from_plugin.clone();

                            if shortcut.matches(Modifiers::ALT, Code::Space) {
                                if event.state == ShortcutState::Pressed {
                                    info!("Alt+Space pressed! (Global Shortcut)");
                                    // Use the "alt-space" identifier for consistency
                                    recording_manager.try_start_recording("alt-space");
                                } else if event.state == ShortcutState::Released {
                                    info!("Alt+Space released! (Global Shortcut)");
                                    tauri::async_runtime::spawn(async move {
                                        // recording_manager, transcription_manager, and app_handle_for_async_tasks are moved into this async block
                                        if let Some(samples) =
                                            recording_manager.stop_recording("alt-space")
                                        {
                                            match transcription_manager.transcribe(samples) {
                                                // Not .await, as transcribe is synchronous
                                                Ok(transcription) => {
                                                    println!(
                                                        "Global Shortcut Transcription: {}",
                                                        transcription
                                                    );
                                                    paste(
                                                        transcription,
                                                        app_handle_for_async_tasks,
                                                    );
                                                }
                                                Err(err) => println!(
                                                    "Global Shortcut Transcription error: {}",
                                                    err
                                                ),
                                            }
                                        }
                                    });
                                }
                            }
                        })
                        .build(),
                )?;
            }

            Ok(())
        })
        .on_window_event(|app, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                app.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
