use std::sync::Arc;

use serde::Serialize;
use tauri::image::Image;
use tauri::tray::TrayIcon;
use tauri::App;
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

use crate::managers::audio::AudioRecordingManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings;
use crate::settings::ShortcutBinding;
use crate::utils;

fn transcribe_pressed(app: &AppHandle) {
    let tray = app.state::<TrayIcon>();
    tray.set_icon(Some(
        Image::from_path(
            app.path()
                .resolve(
                    "resources/tray_recording_64x64.png",
                    tauri::path::BaseDirectory::Resource,
                )
                .expect("failed to resolve"),
        )
        .expect("failed to set icon"),
    ));

    let rm = app.state::<Arc<AudioRecordingManager>>();
    rm.try_start_recording("transcribe");
}

fn transcribe_released(app: &AppHandle) {
    let tray = app.state::<TrayIcon>();
    tray.set_icon(Some(
        Image::from_path(
            app.path()
                .resolve(
                    "resources/tray_64x64.png",
                    tauri::path::BaseDirectory::Resource,
                )
                .expect("failed to resolve"),
        )
        .expect("failed to set icon"),
    ));

    let ah = app.clone();
    let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
    let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());

    tauri::async_runtime::spawn(async move {
        if let Some(samples) = rm.stop_recording("transcribe") {
            match tm.transcribe(samples) {
                // Not .await, as transcribe is synchronous
                Ok(transcription) => {
                    println!("Global Shortcut Transcription: {}", transcription);
                    utils::paste(transcription, ah);
                }
                Err(err) => println!("Global Shortcut Transcription error: {}", err),
            }
        }
    });
}

pub fn init_shortcuts(app: &App) {
    let settings = settings::load_or_create_app_settings(app);

    // Register shortcuts with the bindings from settings
    for (_id, binding) in settings.bindings {
        _register_shortcut(app.handle(), binding);
    }
}

#[derive(Serialize)]
pub struct BindingResponse {
    success: bool,
    binding: Option<ShortcutBinding>,
    error: Option<String>,
}

#[tauri::command]
pub fn change_binding(
    app: AppHandle,
    id: String,
    binding: String,
) -> Result<BindingResponse, String> {
    let mut settings = settings::get_settings(&app);

    // Get the binding to modify
    let binding_to_modify = match settings.bindings.get(&id) {
        Some(binding) => binding.clone(),
        None => {
            return Ok(BindingResponse {
                success: false,
                binding: None,
                error: Some(format!("Binding with id '{}' not found", id)),
            })
        }
    };

    // Unregister the existing binding
    if let Err(e) = _unregister_shortcut(&app, binding_to_modify.clone()) {
        return Ok(BindingResponse {
            success: false,
            binding: None,
            error: Some(format!("Failed to unregister shortcut: {}", e)),
        });
    }

    // Create an updated binding
    let mut updated_binding = binding_to_modify;
    updated_binding.current_binding = binding;

    // Register the new binding
    if let Err(e) = _register_shortcut(&app, updated_binding.clone()) {
        return Ok(BindingResponse {
            success: false,
            binding: None,
            error: Some(format!("Failed to register shortcut: {}", e)),
        });
    }

    // Update the binding in the settings
    settings.bindings.insert(id, updated_binding.clone());

    // Save the settings
    settings::write_settings(&app, settings);

    // Return the updated binding
    Ok(BindingResponse {
        success: true,
        binding: Some(updated_binding),
        error: None,
    })
}

#[tauri::command]
pub fn reset_binding(app: AppHandle, id: String) -> Result<BindingResponse, String> {
    let binding = settings::get_stored_binding(&app, &id);

    return change_binding(app, id, binding.default_binding);
}

fn _register_shortcut(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {
    // Parse shortcut and return error if it fails
    let shortcut = match binding.current_binding.parse::<Shortcut>() {
        Ok(s) => s,
        Err(e) => return Err(format!("Failed to parse shortcut: {}", e)),
    };

    app.global_shortcut()
        .on_shortcut(shortcut, move |handler_app, scut, event| {
            if scut == &shortcut {
                println!("Global Shortcut pressed! {}", scut.into_string());
                if event.state == ShortcutState::Pressed {
                    transcribe_pressed(handler_app);
                } else if event.state == ShortcutState::Released {
                    transcribe_released(handler_app);
                }
            }
        })
        .map_err(|e| format!("Couldn't register shortcut: {}", e))?;

    Ok(())
}

fn _unregister_shortcut(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {
    let shortcut = match binding.current_binding.parse::<Shortcut>() {
        Ok(s) => s,
        Err(e) => return Err(format!("Failed to parse shortcut: {}", e)),
    };

    app.global_shortcut()
        .unregister(shortcut)
        .map_err(|e| format!("Failed to unregister shortcut: {}", e))?;

    Ok(())
}
