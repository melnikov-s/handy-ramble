use crate::settings;

use enigo::Enigo;
use enigo::Key;
use enigo::Keyboard;
use enigo::Settings;

use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::thread;
use tauri::image::Image;
use tauri::tray::TrayIcon;
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;

fn send_paste() -> Result<(), String> {
    // Determine the modifier key based on the OS
    #[cfg(target_os = "macos")]
    let modifier_key = Key::Meta; // Command key on macOS
    #[cfg(not(target_os = "macos"))]
    let modifier_key = Key::Control; // Control key on other systems

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to initialize Enigo: {}", e))?;

    // Press both keys
    enigo
        .key(modifier_key, enigo::Direction::Press)
        .map_err(|e| format!("Failed to press modifier key: {}", e))?;
    enigo
        .key(Key::Unicode('v'), enigo::Direction::Press)
        .map_err(|e| format!("Failed to press V key: {}", e))?;

    // Release both keys
    enigo
        .key(Key::Unicode('v'), enigo::Direction::Release)
        .map_err(|e| format!("Failed to release V key: {}", e))?;
    enigo
        .key(modifier_key, enigo::Direction::Release)
        .map_err(|e| format!("Failed to release modifier key: {}", e))?;

    Ok(())
}

pub fn paste(text: String, app_handle: AppHandle) -> Result<(), String> {
    let clipboard = app_handle.clipboard();

    // get the current clipboard content
    let clipboard_content = clipboard.read_text().unwrap_or_default();

    clipboard
        .write_text(&text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;

    send_paste()?;

    // restore the clipboard
    clipboard
        .write_text(&clipboard_content)
        .map_err(|e| format!("Failed to restore clipboard: {}", e))?;

    Ok(())
}

pub enum TrayIconState {
    Idle,
    Recording,
}

pub fn change_tray_icon(app: &AppHandle, icon: TrayIconState) {
    let tray = app.state::<TrayIcon>();

    let icon_path = match icon {
        TrayIconState::Idle => "resources/tray_idle.png",
        TrayIconState::Recording => "resources/tray_recording.png",
    };

    let _ = tray.set_icon(Some(
        Image::from_path(
            app.path()
                .resolve(icon_path, tauri::path::BaseDirectory::Resource)
                .expect("failed to resolve"),
        )
        .expect("failed to set icon"),
    ));
}

pub fn play_recording_sound(app: &AppHandle) {
    // Check if audio feedback is enabled
    let settings = settings::get_settings(app);
    if !settings.audio_feedback {
        return;
    }

    let app_handle = app.clone();

    // Spawn a new thread to play the audio without blocking the main thread
    thread::spawn(move || {
        // Get the path to the rec.wav file in resources
        let audio_path = match app_handle
            .path()
            .resolve("resources/rec.wav", tauri::path::BaseDirectory::Resource)
        {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Failed to resolve audio file path: {}", e);
                return;
            }
        };

        // Try to play the audio file
        if let Err(e) = play_audio_file(&audio_path) {
            eprintln!("Failed to play recording sound: {}", e);
        }
    });
}

fn play_audio_file(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    // Get a output stream handle to the default physical sound device
    let (_stream, stream_handle) = OutputStream::try_default()?;

    // Load the audio file
    let file = File::open(path)?;
    let buf_reader = BufReader::new(file);
    let source = Decoder::new(buf_reader)?;

    // Create a sink to play the audio
    let sink = Sink::try_new(&stream_handle)?;
    sink.append(source);

    // Wait for the audio to finish playing
    sink.sleep_until_end();

    Ok(())
}
