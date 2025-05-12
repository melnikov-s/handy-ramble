use rdev::{simulate, EventType, Key, SimulateError};
use std::thread;
use std::time;
use tauri::image::Image;
use tauri::tray::TrayIcon;
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;

fn try_send_event(event: &EventType) {
    if let Err(SimulateError) = simulate(event) {
        println!("We could not send {:?}", event);
    }
}

fn send_with_delay(event: EventType, delay_ms: u64) {
    try_send_event(&event);
    thread::sleep(time::Duration::from_millis(delay_ms));
}

fn send_paste() {
    // Determine the modifier key based on the OS
    #[cfg(target_os = "macos")]
    let modifier_key = Key::MetaLeft; // Command key on macOS
    #[cfg(not(target_os = "macos"))]
    let modifier_key = Key::ControlLeft; // Control key on other systems

    // Press both keys
    send_with_delay(EventType::KeyPress(modifier_key), 100);
    send_with_delay(EventType::KeyPress(Key::KeyV), 100);

    // Release both keys
    send_with_delay(EventType::KeyRelease(Key::KeyV), 0);
    send_with_delay(EventType::KeyRelease(modifier_key), 0);
}

pub fn paste(text: String, app_handle: AppHandle) {
    let clipboard = app_handle.clipboard();

    // get the current clipboard content
    let clipboard_content = clipboard.read_text().unwrap_or_default();

    clipboard.write_text(&text).unwrap();
    send_paste();

    // restore the clipboard
    clipboard.write_text(&clipboard_content).unwrap();
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
