pub mod audio;
pub mod chat;
pub mod history;
pub mod models;
pub mod providers;
pub mod transcription;

use crate::settings::{get_settings, write_settings, AppSettings, LogLevel};
use crate::utils::{cancel_current_operation, resume_current_operation};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

// Counter for unique chat window labels
static CHAT_WINDOW_COUNTER: AtomicU32 = AtomicU32::new(0);

// Storage for pending clip attachments (shared between clipping tool and chat windows)
static PENDING_CLIP: Mutex<Option<String>> = Mutex::new(None);

/// Opens a new chat window, optionally with initial context
#[tauri::command]
#[specta::specta]
pub fn open_chat_window(app: AppHandle, context: Option<String>) -> Result<String, String> {
    let window_id = CHAT_WINDOW_COUNTER.fetch_add(1, Ordering::SeqCst);
    let window_label = format!("chat_{}", window_id);

    // Build the URL with optional context parameter
    let url = if let Some(ctx) = &context {
        let encoded_context = urlencoding::encode(ctx);
        format!("src/chat/index.html?context={}", encoded_context)
    } else {
        "src/chat/index.html".to_string()
    };

    let mut builder =
        WebviewWindowBuilder::new(&app, &window_label, tauri::WebviewUrl::App(url.into()))
            .title("Ramble Chat")
            .inner_size(500.0, 600.0)
            .min_inner_size(400.0, 400.0)
            .resizable(true)
            .visible(true)
            .focused(true)
            .always_on_top(true);

    #[cfg(target_os = "macos")]
    {
        use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
        if let Ok(menu) = Menu::with_id(&app, "chat_menu") {
            if let Ok(edit_menu) = Submenu::with_id(&app, "edit", "Edit", true) {
                let _ = edit_menu.append_items(&[
                    &PredefinedMenuItem::undo(&app, None).unwrap(),
                    &PredefinedMenuItem::redo(&app, None).unwrap(),
                    &PredefinedMenuItem::separator(&app).unwrap(),
                    &PredefinedMenuItem::cut(&app, None).unwrap(),
                    &PredefinedMenuItem::copy(&app, None).unwrap(),
                    &PredefinedMenuItem::paste(&app, None).unwrap(),
                    &PredefinedMenuItem::select_all(&app, None).unwrap(),
                ]);
                let _ = menu.append(&edit_menu);
                builder = builder.menu(menu);
            }
        }
    }

    match builder.build() {
        Ok(window) => {
            log::info!("Chat window '{}' created successfully", window_label);
            let _ = window.set_focus();
            Ok(window_label)
        }
        Err(e) => {
            log::error!("Failed to create chat window: {}", e);
            Err(format!("Failed to create chat window: {}", e))
        }
    }
}

/// Message structure for forking conversations
#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type, Clone)]
pub struct ForkMessage {
    pub role: String,
    pub content: String,
}

/// Opens a new chat window with initial messages (for forking conversations)
#[tauri::command]
#[specta::specta]
pub fn open_chat_window_with_messages(
    app: AppHandle,
    messages: Vec<ForkMessage>,
) -> Result<String, String> {
    let window_id = CHAT_WINDOW_COUNTER.fetch_add(1, Ordering::SeqCst);
    let window_label = format!("chat_{}", window_id);

    // Serialize messages to JSON and URL-encode them
    let messages_json = serde_json::to_string(&messages)
        .map_err(|e| format!("Failed to serialize messages: {}", e))?;
    let encoded_messages = urlencoding::encode(&messages_json);
    let url = format!("src/chat/index.html?messages={}", encoded_messages);

    let mut builder =
        WebviewWindowBuilder::new(&app, &window_label, tauri::WebviewUrl::App(url.into()))
            .title("Ramble Chat")
            .inner_size(500.0, 600.0)
            .min_inner_size(400.0, 400.0)
            .resizable(true)
            .visible(true)
            .focused(true)
            .always_on_top(true);

    #[cfg(target_os = "macos")]
    {
        use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
        if let Ok(menu) = Menu::with_id(&app, "chat_menu") {
            if let Ok(edit_menu) = Submenu::with_id(&app, "edit", "Edit", true) {
                let _ = edit_menu.append_items(&[
                    &PredefinedMenuItem::undo(&app, None).unwrap(),
                    &PredefinedMenuItem::redo(&app, None).unwrap(),
                    &PredefinedMenuItem::separator(&app).unwrap(),
                    &PredefinedMenuItem::cut(&app, None).unwrap(),
                    &PredefinedMenuItem::copy(&app, None).unwrap(),
                    &PredefinedMenuItem::paste(&app, None).unwrap(),
                    &PredefinedMenuItem::select_all(&app, None).unwrap(),
                ]);
                let _ = menu.append(&edit_menu);
                builder = builder.menu(menu);
            }
        }
    }

    match builder.build() {
        Ok(window) => {
            log::info!(
                "Forked chat window '{}' created with {} messages",
                window_label,
                messages.len()
            );
            let _ = window.set_focus();
            Ok(window_label)
        }
        Err(e) => {
            log::error!("Failed to create forked chat window: {}", e);
            Err(format!("Failed to create forked chat window: {}", e))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn cancel_operation(app: AppHandle) {
    cancel_current_operation(&app);
}

#[tauri::command]
#[specta::specta]
pub fn pause_operation(app: AppHandle) -> bool {
    crate::utils::toggle_pause_operation(&app);
    true
}

#[tauri::command]
#[specta::specta]
pub fn resume_operation(app: AppHandle) -> bool {
    resume_current_operation(&app).is_some()
}

#[tauri::command]
#[specta::specta]
pub fn get_app_dir_path(app: AppHandle) -> Result<String, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    Ok(app_data_dir.to_string_lossy().to_string())
}

#[tauri::command]
#[specta::specta]
pub fn get_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    // Wrap in catch_unwind to prevent app crash if serialization fails
    // This seems to be happening with serde_json::ser::format_escaped_str_contents
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let settings = get_settings(&app);
        // Force serialization check
        if let Err(e) = serde_json::to_string(&settings) {
            log::error!("Settings serialization check failed: {}", e);
        }
        settings
    }));

    match result {
        Ok(settings) => Ok(settings),
        Err(e) => {
            log::error!("get_app_settings panicked: {:?}", e);
            Err("Failed to retrieve settings due to internal error".to_string())
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_default_settings() -> Result<AppSettings, String> {
    Ok(crate::settings::get_default_settings())
}

#[tauri::command]
#[specta::specta]
pub fn get_log_dir_path(app: AppHandle) -> Result<String, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to get log directory: {}", e))?;

    Ok(log_dir.to_string_lossy().to_string())
}

#[specta::specta]
#[tauri::command]
pub fn set_log_level(app: AppHandle, level: LogLevel) -> Result<(), String> {
    let tauri_log_level: tauri_plugin_log::LogLevel = level.into();
    let log_level: log::Level = tauri_log_level.into();
    // Update the file log level atomic so the filter picks up the new level
    crate::FILE_LOG_LEVEL.store(
        log_level.to_level_filter() as u8,
        std::sync::atomic::Ordering::Relaxed,
    );

    let mut settings = get_settings(&app);
    settings.log_level = level;
    write_settings(&app, settings);

    Ok(())
}

#[specta::specta]
#[tauri::command]
pub fn open_recordings_folder(app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    let recordings_dir = app_data_dir.join("recordings");

    let path = recordings_dir.to_string_lossy().as_ref().to_string();
    app.opener()
        .open_path(path, None::<String>)
        .map_err(|e| format!("Failed to open recordings folder: {}", e))?;

    Ok(())
}

#[specta::specta]
#[tauri::command]
pub fn open_log_dir(app: AppHandle) -> Result<(), String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to get log directory: {}", e))?;

    let path = log_dir.to_string_lossy().as_ref().to_string();
    app.opener()
        .open_path(path, None::<String>)
        .map_err(|e| format!("Failed to open log directory: {}", e))?;

    Ok(())
}

#[specta::specta]
#[tauri::command]
pub fn open_app_data_dir(app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    let path = app_data_dir.to_string_lossy().as_ref().to_string();
    app.opener()
        .open_path(path, None::<String>)
        .map_err(|e| format!("Failed to open app data directory: {}", e))?;

    Ok(())
}

// === App-to-Prompt Category Mapping Commands ===

/// Get the list of known applications with suggested categories
#[tauri::command]
#[specta::specta]
pub fn get_known_applications() -> Vec<crate::known_apps::KnownApp> {
    crate::known_apps::get_known_applications()
}

/// Get the list of installed applications on the system
#[tauri::command]
#[specta::specta]
pub fn get_installed_applications() -> Vec<crate::app_detection::InstalledApp> {
    crate::app_detection::get_installed_applications()
}

/// Get current user-defined app-to-category mappings
#[tauri::command]
#[specta::specta]
pub fn get_app_category_mappings(app: AppHandle) -> Vec<crate::settings::AppCategoryMapping> {
    let settings = get_settings(&app);
    settings.app_category_mappings
}

/// Set or update an app-to-category mapping
#[tauri::command]
#[specta::specta]
pub fn set_app_category_mapping(
    app: AppHandle,
    bundle_id: String,
    display_name: String,
    category_id: String,
) -> Result<(), String> {
    let mut settings = get_settings(&app);

    // Check if mapping already exists for this bundle_id
    if let Some(existing) = settings
        .app_category_mappings
        .iter_mut()
        .find(|m| m.bundle_identifier == bundle_id)
    {
        existing.category_id = category_id;
        existing.display_name = display_name;
    } else {
        // Add new mapping
        settings
            .app_category_mappings
            .push(crate::settings::AppCategoryMapping {
                bundle_identifier: bundle_id,
                display_name,
                category_id,
            });
    }

    write_settings(&app, settings);
    Ok(())
}

/// Remove an app-to-category mapping
#[tauri::command]
#[specta::specta]
pub fn remove_app_category_mapping(app: AppHandle, bundle_id: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings
        .app_category_mappings
        .retain(|m| m.bundle_identifier != bundle_id);
    write_settings(&app, settings);
    Ok(())
}

/// Sets the visibility of all chat windows (but NOT the main settings window)
pub fn set_chat_window_visibility(app: &AppHandle, visible: bool) {
    let windows = app.webview_windows();
    log::info!(
        "Setting chat window visibility to {}. scanning {} windows...",
        visible,
        windows.len()
    );
    for (label, window) in windows {
        if label.starts_with("chat_") {
            log::info!(
                "--> {} '{}'",
                if visible { "Showing" } else { "Hiding" },
                label
            );
            if visible {
                let _ = window.show();
            } else {
                let _ = window.hide();
            }
        } else {
            // log::debug!("Skipping window '{}' (not a chat window)", label);
        }
    }
}

/// Command to capture a screenshot or region, hiding all app windows first
#[tauri::command]
#[specta::specta]
pub async fn capture_screen_mode(app: AppHandle, region: bool) -> Result<String, String> {
    // 1. Hide all chat windows and the overlay
    set_chat_window_visibility(&app, false);
    crate::overlay::set_overlay_visibility(&app, false);

    // Give the OS a moment to hide the windows
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // 2. Capture
    let result = if region {
        return Err("Please use capture_region_command for regional capture".to_string());
    } else {
        crate::vision::capture_screen()
    };

    // 3. Restore visibility
    set_chat_window_visibility(&app, true);
    crate::overlay::set_overlay_visibility(&app, true);

    result
}

#[tauri::command]
#[specta::specta]
pub async fn open_clipping_tool(app: AppHandle) -> Result<(), String> {
    let window_label = "clipping_overlay";

    // Always hide chat windows, overlay, AND main window first
    set_chat_window_visibility(&app, false);
    crate::overlay::set_overlay_visibility(&app, false);

    // Explicitly hide main window to prevent it from appearing during clipping
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.hide();
    }

    // If window exists, destroy it and wait for cleanup
    if let Some(window) = app.get_webview_window(window_label) {
        log::info!("Destroying existing clipping window to ensure fresh state");
        let _ = window.destroy();
        // Wait for Tauri to clean up the window
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // Create a fresh window
    let builder = WebviewWindowBuilder::new(
        &app,
        window_label,
        tauri::WebviewUrl::App("src/clipping-overlay/index.html".into()),
    )
    .title("Clipping Tool")
    .transparent(true)
    .decorations(false)
    .always_on_top(true)
    .maximized(true)
    .shadow(false)
    .visible(true);

    match builder.build() {
        Ok(window) => {
            log::info!("Created fresh clipping tool window");

            // Force focus
            if let Err(e) = window.set_focus() {
                log::error!("Failed to focus clipping window: {}", e);
            }

            Ok(())
        }
        Err(e) => {
            log::error!("Failed to create clipping tool window: {}", e);
            Err(format!("Failed to create clipping tool window: {}", e))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn restore_app_visibility(app: AppHandle) -> Result<(), String> {
    log::info!("Restoring app visibility via command");
    set_chat_window_visibility(&app, true);
    crate::overlay::set_overlay_visibility(&app, true);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn capture_region_command(
    app: AppHandle,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    log::info!(
        "Capture region command received: {}x{} at {},{}",
        width,
        height,
        x,
        y
    );

    // Hide clipping tool
    if let Some(win) = app.get_webview_window("clipping_overlay") {
        let _ = win.hide();
    }

    // Give the OS a moment to ensure invisibility is processed before capture
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // 2. Capture
    // We already moved panic handling into vision::capture_region, so we can just call it.
    let result = crate::vision::capture_region(x, y, width, height);

    // 3. Restore visibility BEFORE emitting event to ensure frontend is awake
    log::info!("Restoring visibility before storing capture");
    set_chat_window_visibility(&app, true);
    crate::overlay::set_overlay_visibility(&app, true);

    if let Ok(ref base64) = result {
        log::info!(
            "Storing captured clip ({} bytes) in PENDING_CLIP",
            base64.len()
        );

        // Store in PENDING_CLIP for ChatWindow to retrieve
        if let Ok(mut pending) = PENDING_CLIP.lock() {
            *pending = Some(base64.clone());
            log::info!("Clip stored successfully");
        } else {
            log::error!("Failed to lock PENDING_CLIP mutex");
        }
    } else if let Err(ref e) = result {
        log::error!("Region capture failed: {}", e);
    }

    result
}

/// Retrieves and clears any pending clip attachment
/// Called by ChatWindow to get captured images
#[tauri::command]
#[specta::specta]
pub fn get_pending_clip() -> Option<String> {
    if let Ok(mut pending) = PENDING_CLIP.lock() {
        let clip = pending.take();
        if clip.is_some() {
            log::info!("Pending clip retrieved and cleared");
        }
        clip
    } else {
        log::error!("Failed to lock PENDING_CLIP mutex");
        None
    }
}
