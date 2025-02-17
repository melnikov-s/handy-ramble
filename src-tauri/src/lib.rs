mod managers;

use log::info;
use managers::audio::AudioRecordingManager;
use managers::keybinding::KeyBindingManager;
use managers::transcription::TranscriptionManager;
use rdev::{simulate, EventType, Key, SimulateError};
use rig::{completion::Prompt, providers::anthropic};
use std::sync::{Arc, Mutex};
use std::{thread, time};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_clipboard_manager::ClipboardExt;

fn try_send_event(event: &EventType) {
    if let Err(SimulateError) = simulate(event) {
        println!("We could not send {:?}", event);
    }
}

fn send(event: EventType) {
    try_send_event(&event);
    thread::sleep(time::Duration::from_millis(40));
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

fn send_copy() {
    // Determine the modifier key based on the OS
    #[cfg(target_os = "macos")]
    let modifier_key = Key::MetaLeft; // Command key on macOS
    #[cfg(not(target_os = "macos"))]
    let modifier_key = Key::ControlLeft; // Control key on other systems

    // Press both keys
    send(EventType::KeyPress(modifier_key));
    send(EventType::KeyPress(Key::KeyC));

    // Release both keys
    send(EventType::KeyRelease(Key::KeyC));
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

fn get_highlighted_text(app_handle: tauri::AppHandle) -> String {
    let clipboard = app_handle.clipboard();

    // save the clipboard content
    let clipboard_content = clipboard.read_text().unwrap_or_default();

    // empty the clipboard
    clipboard.write_text("").unwrap();

    // issue 'copy'
    send_copy();

    // get the highlighted text
    let highlighted_text = clipboard.read_text().unwrap_or_default();

    // restore the clipboard content
    clipboard.write_text(&clipboard_content).unwrap();

    highlighted_text
}

const INSTRUCT_SYS: &str = r#"
You are a helpful assistant. You will receive voice transcriptions
from a user that may include both a command/question and some
minimal context to help you respond appropriately.

For example, the user might say:
- A direct question with no context: "What is the capital of France?"
- A command with context: "get commit message I fixed the bug in the login system"
"#;

const CODE_SYS: &str = r#"
You are a code-only assistant. I will provide you with selected text or clipboard content along with instructions. If I request code, output only the exact code implementation. If I request a terminal command, provide only the valid command syntax. Never use markdown, explanations, or additional text.

    When I share selected text or clipboard content, use that as context for generating your response. The output should be ready to copy and paste directly, with no formatting or commentary. For terminal commands, ensure they are valid for the specified environment. Note for terminal commands, I typically use lowercase instead of uppercase. You may also be given them directly, but need to translate them into a way that can actually be executed in the terminal because the transcription you are given might be poor.

    Output only:
    - Raw code implementation when code is requested
    - Terminal command syntax when a command is requested
    - No markdown, no backticks, no explanations
    - No additional text or descriptions
"#;

fn register_bindings(manager: &mut KeyBindingManager) {
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
                        Ok(transcription) => {
                            println!("Transcription: {}", transcription);
                            paste(transcription, ctx.app_handle.clone());
                        }
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

                        let instruct = ctx
                            .anthropic
                            .agent(anthropic::CLAUDE_3_5_SONNET)
                            .preamble(INSTRUCT_SYS)
                            .temperature(0.5)
                            .build();

                        let highlighted_text = get_highlighted_text(ctx.app_handle.clone());
                        println!("Highlighted Text: {}", highlighted_text);
                        let prompt = format!("{}\n\ncontext:{}\n", transcription, highlighted_text);

                        match instruct.prompt(prompt).await {
                            Ok(response) => {
                                println!("Sonnet response: {}", response);
                                paste(response, ctx.app_handle.clone());
                            }
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

                            let code = ctx
                                .anthropic
                                .agent(anthropic::CLAUDE_3_5_SONNET)
                                .preamble(CODE_SYS)
                                .temperature(0.5)
                                .build();

                            let highlighted_text = get_highlighted_text(ctx.app_handle.clone());
                            let prompt =
                                format!("{}\n\ncontext:{}\n", transcription, highlighted_text);

                            match code.prompt(prompt).await {
                                Ok(response) => {
                                    println!("Sonnet response: {}", response);
                                    paste(response, ctx.app_handle.clone());
                                }
                                Err(err) => println!("Sonnet error: {}", err),
                            }
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let recording_manager =
        Arc::new(AudioRecordingManager::new().expect("Failed to initialize recording manager"));
    let transcription_manager =
        Arc::new(TranscriptionManager::new().expect("Failed to initialize transcription manager"));
    // let transcription_manager = Arc::new(TranscriptionManager::new());
    let claude_client = Arc::new(anthropic::Client::from_env());

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--auto-launch"]),
        ))
        .plugin(tauri_plugin_macos_permissions::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();

            let manager = Arc::new(Mutex::new(KeyBindingManager::new(
                recording_manager.clone(),
                transcription_manager.clone(),
                claude_client.clone(),
                app_handle.clone(),
            )));

            {
                let mut manager = manager.lock().unwrap();
                register_bindings(&mut manager);
            }

            let manager_clone = manager.clone();
            tauri::async_runtime::spawn(async move {
                rdev::listen(move |event| {
                    if let Ok(manager) = manager_clone.lock() {
                        manager.handle_event(&event);
                    }
                })
                .unwrap();
            });
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
