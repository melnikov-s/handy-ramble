pub mod audio;
pub mod chat;
pub mod history;
pub mod models;
pub mod providers;
pub mod transcription;

use crate::settings::{get_settings, write_settings, AppSettings, LogLevel};
use crate::utils::{cancel_current_operation, resume_current_operation};
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, Manager, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

// Counter for unique chat window labels
static CHAT_WINDOW_COUNTER: AtomicU32 = AtomicU32::new(0);

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

    match WebviewWindowBuilder::new(&app, &window_label, tauri::WebviewUrl::App(url.into()))
        .title("Ramble Chat")
        .inner_size(500.0, 600.0)
        .min_inner_size(400.0, 400.0)
        .resizable(true)
        .visible(true)
        .focused(true)
        .build()
    {
        Ok(_window) => {
            log::info!("Chat window '{}' created successfully", window_label);
            Ok(window_label)
        }
        Err(e) => {
            log::error!("Failed to create chat window: {}", e);
            Err(format!("Failed to create chat window: {}", e))
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
