mod managers;

use log::info;
use managers::audio::AudioRecordingManager;
use managers::transcription::TranscriptionManager;
use rdev::{simulate, EventType, Key, SimulateError};
use std::sync::Arc;
use std::{thread, time};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};
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
            let tray = TrayIconBuilder::new().build(app)?;

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

            // Add managers to Tauri's managed state
            app.manage(recording_manager.clone());
            app.manage(transcription_manager.clone());

            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_shortcuts(["alt+space", "ctrl+d"])? // Register "alt+space"
                        .with_handler(move |app_handle_from_plugin, shortcut, event| {
                            // Retrieve managers from state
                            let recording_manager_state =
                                app_handle_from_plugin.state::<Arc<AudioRecordingManager>>();
                            let transcription_manager_state =
                                app_handle_from_plugin.state::<Arc<TranscriptionManager>>();
                            let app_handle_for_async_tasks = app_handle_from_plugin.clone();

                            if shortcut.matches(Modifiers::ALT, Code::Space) {
                                if event.state == ShortcutState::Pressed {
                                    info!("Alt+Space pressed! (Global Shortcut)");
                                    // Use the "alt-space" identifier for consistency
                                    recording_manager_state.try_start_recording("alt-space");
                                } else if event.state == ShortcutState::Released {
                                    info!("Alt+Space released! (Global Shortcut)");
                                    // Clone Arcs for the async block
                                    let rm_clone = recording_manager_state.inner().clone();
                                    let tm_clone = transcription_manager_state.inner().clone();

                                    tauri::async_runtime::spawn(async move {
                                        if let Some(samples) = rm_clone.stop_recording("alt-space")
                                        {
                                            match tm_clone.transcribe(samples) {
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
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();

                // TODO may be different on windows, this works for macos
                tauri::AppHandle::hide(window.app_handle()).unwrap();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
