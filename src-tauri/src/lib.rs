mod actions;
mod managers;
mod settings;
mod shortcut;
mod utils;

use managers::audio::AudioRecordingManager;
use managers::transcription::TranscriptionManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

#[derive(Default)]
struct ShortcutToggleStates {
    // Map: shortcut_binding_id -> is_active
    active_toggles: HashMap<String, bool>,
}

type ManagedToggleState = Mutex<ShortcutToggleStates>;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--auto-launch"]),
        ))
        .plugin(tauri_plugin_macos_permissions::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Mutex::new(ShortcutToggleStates::default()))
        .setup(move |app| {
            let settings_i = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_i, &quit_i])?;
            let tray = TrayIconBuilder::new()
                .icon(Image::from_path(app.path().resolve(
                    "resources/tray_idle.png",
                    tauri::path::BaseDirectory::Resource,
                )?)?)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => {
                        let settings_window = app.get_webview_window("main").unwrap();
                        settings_window.show().unwrap();
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;
            app.manage(tray);

            let recording_manager = Arc::new(
                AudioRecordingManager::new(app).expect("Failed to initialize recording manager"),
            );
            let transcription_manager = Arc::new(
                TranscriptionManager::new(&app)
                    .expect("Failed to initialize transcription manager"),
            );

            // Add managers to Tauri's managed state
            app.manage(recording_manager.clone());
            app.manage(transcription_manager.clone());

            shortcut::init_shortcuts(app);

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                #[cfg(target_os = "macos")]
                let res = window
                    .app_handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory);
                if let Err(e) = res {
                    println!("Failed to set activation policy: {}", e);
                }

                api.prevent_close();

                // TODO may be different on windows, this works for macos
                tauri::AppHandle::hide(window.app_handle()).unwrap();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            shortcut::change_binding,
            shortcut::reset_binding,
            shortcut::change_ptt_setting
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
