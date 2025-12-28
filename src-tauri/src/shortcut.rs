use log::{debug, error, warn};
use serde::Serialize;
use specta::Type;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::actions::ACTION_MAP;
use crate::managers::audio::AudioRecordingManager;
use crate::overlay;
use crate::settings::ShortcutBinding;
use crate::settings::{
    self, get_settings, ClipboardHandling, LLMPrompt, OverlayPosition, PasteMethod, SoundTheme,
    APPLE_INTELLIGENCE_DEFAULT_MODEL_ID, APPLE_INTELLIGENCE_PROVIDER_ID,
};
use crate::tray;
use crate::ManagedToggleState;

#[cfg(target_os = "macos")]
use crate::key_listener;

/// Global state for tracking press timestamps (for smart PTT detection)
static PRESS_TIMESTAMPS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

fn get_press_timestamps() -> &'static Mutex<HashMap<String, Instant>> {
    PRESS_TIMESTAMPS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn init_shortcuts(app: &AppHandle) {
    let default_bindings = settings::get_default_settings().bindings;
    let user_settings = settings::load_or_create_app_settings(app);

    // Register all default shortcuts, applying user customizations
    for (id, default_binding) in default_bindings {
        let binding = user_settings
            .bindings
            .get(&id)
            .cloned()
            .unwrap_or(default_binding);

        // Skip cancel (Escape) - it's handled via a low-level listener to avoid global blocking
        if id == "cancel" {
            continue;
        }

        // For vision and pause, we use the current binding but we also register 
        // common variants to ENSURE key swallowing works on macOS.
        if id == "vision_capture" || id == "pause_toggle" {
            register_swallowing_shortcuts(app, binding);
            continue;
        }

        if let Err(e) = register_shortcut(app, binding) {
            error!("Failed to register shortcut {} during init: {}", id, e);
        }
    }
}

#[derive(Serialize, Type)]
pub struct BindingResponse {
    success: bool,
    binding: Option<ShortcutBinding>,
    error: Option<String>,
}

#[tauri::command]
#[specta::specta]
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
            let error_msg = format!("Binding with id '{}' not found", id);
            warn!("change_binding error: {}", error_msg);
            return Ok(BindingResponse {
                success: false,
                binding: None,
                error: Some(error_msg),
            });
        }
    };
    // If this is a dynamic binding (vision_capture, pause_toggle), just update settings
    // Note: cancel is handled via raw listener, but we still allow changing its binding string
    if id == "cancel" || id == "vision_capture" || id == "pause_toggle" {
        if let Some(mut b) = settings.bindings.get(&id).cloned() {
            b.current_binding = binding;
            settings.bindings.insert(id.clone(), b.clone());
            settings::write_settings(&app, settings);

            // Re-register vision/pause if changed (they are static)
            if id != "cancel" {
                register_swallowing_shortcuts(&app, b.clone());
            }

            return Ok(BindingResponse {
                success: true,
                binding: Some(b.clone()),
                error: None,
            });
        }
    }

    // Unregister the existing binding
    if let Err(e) = unregister_shortcut(&app, binding_to_modify.clone()) {
        let error_msg = format!("Failed to unregister shortcut: {}", e);
        error!("change_binding error: {}", error_msg);
    }

    // Validate the new shortcut before we touch the current registration
    if let Err(e) = validate_shortcut_string(&binding) {
        warn!("change_binding validation error: {}", e);
        return Err(e);
    }

    // Create an updated binding
    let mut updated_binding = binding_to_modify;
    updated_binding.current_binding = binding;

    // Register the new binding
    if let Err(e) = register_shortcut(&app, updated_binding.clone()) {
        let error_msg = format!("Failed to register shortcut: {}", e);
        error!("change_binding error: {}", error_msg);
        return Ok(BindingResponse {
            success: false,
            binding: None,
            error: Some(error_msg),
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
#[specta::specta]
pub fn reset_binding(app: AppHandle, id: String) -> Result<BindingResponse, String> {
    let binding = settings::get_stored_binding(&app, &id);

    return change_binding(app, id, binding.default_binding);
}

#[tauri::command]
#[specta::specta]
pub fn change_ptt_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // TODO if the setting is currently false, we probably want to
    // cancel any ongoing recordings or actions
    settings.push_to_talk = enabled;

    settings::write_settings(&app, settings);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_audio_feedback_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.audio_feedback = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_audio_feedback_volume_setting(app: AppHandle, volume: f32) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.audio_feedback_volume = volume;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_sound_theme_setting(app: AppHandle, theme: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    let parsed = match theme.as_str() {
        "marimba" => SoundTheme::Marimba,
        "pop" => SoundTheme::Pop,
        "custom" => SoundTheme::Custom,
        other => {
            warn!("Invalid sound theme '{}', defaulting to marimba", other);
            SoundTheme::Marimba
        }
    };
    settings.sound_theme = parsed;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_translate_to_english_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.translate_to_english = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_selected_language_setting(app: AppHandle, language: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.selected_language = language;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_overlay_position_setting(app: AppHandle, position: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    let parsed = match position.as_str() {
        "none" => OverlayPosition::None,
        "top" => OverlayPosition::Top,
        "bottom" => OverlayPosition::Bottom,
        other => {
            warn!("Invalid overlay position '{}', defaulting to bottom", other);
            OverlayPosition::Bottom
        }
    };
    settings.overlay_position = parsed;
    settings::write_settings(&app, settings);

    // Update overlay position without recreating window
    crate::utils::update_overlay_position(&app);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_debug_mode_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.debug_mode = enabled;
    settings::write_settings(&app, settings);

    // Emit event to notify frontend of debug mode change
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({
            "setting": "debug_mode",
            "value": enabled
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_start_hidden_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.start_hidden = enabled;
    settings::write_settings(&app, settings);

    // Notify frontend
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({
            "setting": "start_hidden",
            "value": enabled
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_autostart_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.autostart_enabled = enabled;
    settings::write_settings(&app, settings);

    // Apply the autostart setting immediately
    let autostart_manager = app.autolaunch();
    if enabled {
        let _ = autostart_manager.enable();
    } else {
        let _ = autostart_manager.disable();
    }

    // Notify frontend
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({
            "setting": "autostart_enabled",
            "value": enabled
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_update_checks_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.update_checks_enabled = enabled;
    settings::write_settings(&app, settings);

    let _ = app.emit(
        "settings-changed",
        serde_json::json!({
            "setting": "update_checks_enabled",
            "value": enabled
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn update_custom_words(app: AppHandle, words: Vec<String>) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.custom_words = words;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_word_correction_threshold_setting(
    app: AppHandle,
    threshold: f64,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.word_correction_threshold = threshold;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_paste_method_setting(app: AppHandle, method: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    let parsed = match method.as_str() {
        "ctrl_v" => PasteMethod::CtrlV,
        "direct" => PasteMethod::Direct,
        "none" => PasteMethod::None,
        "shift_insert" => PasteMethod::ShiftInsert,
        "ctrl_shift_v" => PasteMethod::CtrlShiftV,
        other => {
            warn!("Invalid paste method '{}', defaulting to ctrl_v", other);
            PasteMethod::CtrlV
        }
    };
    settings.paste_method = parsed;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_clipboard_handling_setting(app: AppHandle, handling: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    let parsed = match handling.as_str() {
        "dont_modify" => ClipboardHandling::DontModify,
        "copy_to_clipboard" => ClipboardHandling::CopyToClipboard,
        other => {
            warn!(
                "Invalid clipboard handling '{}', defaulting to dont_modify",
                other
            );
            ClipboardHandling::DontModify
        }
    };
    settings.clipboard_handling = parsed;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_post_process_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.coherent_enabled = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_post_process_base_url_setting(
    app: AppHandle,
    provider_id: String,
    base_url: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    
    // Find the provider in llm_providers
    let provider = settings
        .llm_providers
        .iter_mut()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;
    
    // Only allow editing custom providers
    if !provider.is_custom {
        return Err(format!(
            "Provider '{}' does not allow editing the base URL",
            provider.name
        ));
    }

    provider.base_url = base_url;
    settings::write_settings(&app, settings);
    Ok(())
}

/// Generic helper to validate provider exists
fn validate_provider_exists(
    settings: &settings::AppSettings,
    provider_id: &str,
) -> Result<(), String> {
    if !settings
        .llm_providers
        .iter()
        .any(|provider| provider.id == provider_id)
    {
        return Err(format!("Provider '{}' not found", provider_id));
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_post_process_api_key_setting(
    app: AppHandle,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    
    // Find the provider in llm_providers and update its API key
    let provider = settings
        .llm_providers
        .iter_mut()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;
    
    provider.api_key = api_key;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_post_process_model_setting(
    _app: AppHandle,
    _provider_id: String,
    _model: String,
) -> Result<(), String> {
    // Deprecated: Model is now set via llm_models and default_*_model_id
    // This command is kept for API compatibility but does nothing
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_post_process_provider(_app: AppHandle, _provider_id: String) -> Result<(), String> {
    // Deprecated: Provider is now selected via model selection (models link to providers)
    // This command is kept for API compatibility but does nothing
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn add_post_process_prompt(
    app: AppHandle,
    name: String,
    prompt: String,
) -> Result<LLMPrompt, String> {
    let mut settings = settings::get_settings(&app);

    // Generate unique ID using timestamp and random component
    let id = format!("prompt_{}", chrono::Utc::now().timestamp_millis());

    let new_prompt = LLMPrompt {
        id: id.clone(),
        name,
        prompt,
    };

    settings.coherent_prompts.push(new_prompt.clone());
    settings::write_settings(&app, settings);

    Ok(new_prompt)
}

#[tauri::command]
#[specta::specta]
pub fn update_post_process_prompt(
    app: AppHandle,
    id: String,
    name: String,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    if let Some(existing_prompt) = settings
        .coherent_prompts
        .iter_mut()
        .find(|p| p.id == id)
    {
        existing_prompt.name = name;
        existing_prompt.prompt = prompt;
        settings::write_settings(&app, settings);
        Ok(())
    } else {
        Err(format!("Prompt with id '{}' not found", id))
    }
}

#[tauri::command]
#[specta::specta]
pub fn delete_post_process_prompt(app: AppHandle, id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // Don't allow deleting the last prompt
    if settings.coherent_prompts.len() <= 1 {
        return Err("Cannot delete the last prompt".to_string());
    }

    // Find and remove the prompt
    let original_len = settings.coherent_prompts.len();
    settings.coherent_prompts.retain(|p| p.id != id);

    if settings.coherent_prompts.len() == original_len {
        return Err(format!("Prompt with id '{}' not found", id));
    }

    // If the deleted prompt was selected, select the first one or None
    if settings.coherent_selected_prompt_id.as_ref() == Some(&id) {
        settings.coherent_selected_prompt_id =
            settings.coherent_prompts.first().map(|p| p.id.clone());
    }

    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn fetch_post_process_models(
    app: AppHandle,
    provider_id: String,
) -> Result<Vec<String>, String> {
    let settings = settings::get_settings(&app);

    // Find the provider in unified llm_providers
    let provider = settings
        .llm_providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            return Ok(vec![APPLE_INTELLIGENCE_DEFAULT_MODEL_ID.to_string()]);
        }

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            return Err("Apple Intelligence is only available on Apple silicon Macs running macOS 15 or later.".to_string());
        }
    }

    // Get API key from provider
    let api_key = provider.api_key.clone();

    // Skip fetching if no API key for providers that typically need one
    if api_key.trim().is_empty() && !provider.is_custom {
        return Err(format!(
            "API key is required for {}. Please add an API key to list available models.",
            provider.name
        ));
    }

    // For now, use manual HTTP request to have more control over the endpoint
    fetch_models_manual(provider, api_key).await
}

/// Fetch models using manual HTTP request
/// This gives us more control and avoids issues with non-standard endpoints
async fn fetch_models_manual(
    provider: &crate::settings::LLMProvider,
    api_key: String,
) -> Result<Vec<String>, String> {
    // Build the endpoint URL - use standard /models for most providers
    let base_url = provider.base_url.trim_end_matches('/');
    let models_endpoint = "models";
    let endpoint = format!("{}/{}", base_url, models_endpoint);

    // Create HTTP client with headers
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "HTTP-Referer",
        reqwest::header::HeaderValue::from_static("https://github.com/cjpais/Ramble"),
    );
    headers.insert(
        "X-Title",
        reqwest::header::HeaderValue::from_static("Ramble"),
    );

    // Add provider-specific headers
    if provider.id == "anthropic" {
        if !api_key.is_empty() {
            headers.insert(
                "x-api-key",
                reqwest::header::HeaderValue::from_str(&api_key)
                    .map_err(|e| format!("Invalid API key: {}", e))?,
            );
        }
        headers.insert(
            "anthropic-version",
            reqwest::header::HeaderValue::from_static("2023-06-01"),
        );
    } else if !api_key.is_empty() {
        headers.insert(
            "Authorization",
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", api_key))
                .map_err(|e| format!("Invalid API key: {}", e))?,
        );
    }

    let http_client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // Make the request
    let response = http_client
        .get(&endpoint)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch models: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!(
            "Model list request failed ({}): {}",
            status, error_text
        ));
    }

    // Parse the response
    let parsed: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let mut models = Vec::new();

    // Handle OpenAI format: { data: [ { id: "..." }, ... ] }
    if let Some(data) = parsed.get("data").and_then(|d| d.as_array()) {
        for entry in data {
            if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                models.push(id.to_string());
            } else if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
                models.push(name.to_string());
            }
        }
    }
    // Handle array format: [ "model1", "model2", ... ]
    else if let Some(array) = parsed.as_array() {
        for entry in array {
            if let Some(model) = entry.as_str() {
                models.push(model.to_string());
            }
        }
    }

    Ok(models)
}

#[tauri::command]
#[specta::specta]
pub fn set_post_process_selected_prompt(app: AppHandle, id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // Verify the prompt exists
    if !settings.coherent_prompts.iter().any(|p| p.id == id) {
        return Err(format!("Prompt with id '{}' not found", id));
    }

    settings.coherent_selected_prompt_id = Some(id);
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_mute_while_recording_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.mute_while_recording = enabled;
    settings::write_settings(&app, settings);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_append_trailing_space_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.append_trailing_space = enabled;
    settings::write_settings(&app, settings);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_app_language_setting(app: AppHandle, language: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.app_language = language.clone();
    settings::write_settings(&app, settings);

    // Refresh the tray menu with the new language
    tray::update_tray_menu(&app, &tray::TrayIconState::Idle, Some(&language));

    Ok(())
}

// Ramble to Coherent settings commands

#[tauri::command]
#[specta::specta]
pub fn change_ramble_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.coherent_enabled = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ramble_provider_setting(_app: AppHandle, _provider_id: String) -> Result<(), String> {
    // Deprecated: Provider is now selected via default_coherent_model_id
    Ok(())
}

// Centralized LLM provider settings

#[tauri::command]
#[specta::specta]
pub fn change_llm_provider_setting(_app: AppHandle, _provider_id: String) -> Result<(), String> {
    // Deprecated: Provider is now selected via model selection
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ramble_model_setting(_app: AppHandle, _model: String) -> Result<(), String> {
    // Deprecated: Model is now set via default_coherent_model_id
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ramble_use_vision_model_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.coherent_use_vision = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ramble_vision_model_setting(_app: AppHandle, _model: String) -> Result<(), String> {
    // Deprecated: Vision model is now the same as coherent model (supports_vision flag)
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ramble_prompt_setting(_app: AppHandle, _prompt: String) -> Result<(), String> {
    // Deprecated: Prompts are now managed via coherent_prompts
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn reset_ramble_prompt_to_default(_app: AppHandle) -> Result<String, String> {
    // Deprecated: Prompts are now managed via coherent_prompts
    Ok(String::new())
}

#[tauri::command]
#[specta::specta]
pub fn change_hold_threshold_setting(app: AppHandle, threshold_ms: u64) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.hold_threshold_ms = threshold_ms;
    settings::write_settings(&app, settings);
    Ok(())
}

// Prompt mode and category commands

#[tauri::command]
#[specta::specta]
pub fn change_prompt_mode_setting(
    app: AppHandle,
    mode: settings::PromptMode,
) -> Result<(), String> {
    tray::set_prompt_mode(&app, mode);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn update_prompt_category(
    app: AppHandle,
    id: String,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    if let Some(category) = settings.prompt_categories.iter_mut().find(|c| c.id == id) {
        category.prompt = prompt;
        settings::write_settings(&app, settings);
        Ok(())
    } else {
        Err(format!("Category with id '{}' not found", id))
    }
}

#[tauri::command]
#[specta::specta]
pub fn reset_prompt_category_to_default(app: AppHandle, id: String) -> Result<String, String> {
    let mut settings = settings::get_settings(&app);
    let defaults = settings::get_default_settings();

    // Find the default prompt for this category
    let default_prompt = defaults
        .prompt_categories
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.prompt.clone())
        .ok_or_else(|| format!("Default prompt for category '{}' not found", id))?;

    // Update the current settings
    if let Some(category) = settings.prompt_categories.iter_mut().find(|c| c.id == id) {
        category.prompt = default_prompt.clone();
        settings::write_settings(&app, settings);
        Ok(default_prompt)
    } else {
        Err(format!("Category with id '{}' not found", id))
    }
}

#[tauri::command]
#[specta::specta]
pub fn change_default_category_setting(app: AppHandle, category_id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    
    // Verify the category exists
    if !settings.prompt_categories.iter().any(|c| c.id == category_id) {
        return Err(format!("Category with id '{}' not found", category_id));
    }
    
    settings.default_category_id = category_id;
    settings::write_settings(&app, settings);
    Ok(())
}

/// Create a new custom prompt category
#[tauri::command]
#[specta::specta]
pub fn add_prompt_category(
    app: AppHandle,
    name: String,
    icon: String,
    prompt: String,
) -> Result<settings::PromptCategory, String> {
    let mut settings = settings::get_settings(&app);
    
    // Generate unique ID from name
    let base_id = name.to_lowercase().replace(' ', "_");
    let mut id = base_id.clone();
    let mut counter = 1;
    
    // Ensure unique ID
    while settings.prompt_categories.iter().any(|c| c.id == id) {
        id = format!("{}_{}", base_id, counter);
        counter += 1;
    }
    
    let new_category = settings::PromptCategory {
        id: id.clone(),
        name,
        icon,
        prompt,
        is_builtin: false,
    };
    
    settings.prompt_categories.push(new_category.clone());
    settings::write_settings(&app, settings);
    
    Ok(new_category)
}

/// Delete a custom prompt category
#[tauri::command]
#[specta::specta]
pub fn delete_prompt_category(app: AppHandle, id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    
    // Find the category
    let category = settings.prompt_categories.iter().find(|c| c.id == id);
    
    match category {
        None => return Err(format!("Category with id '{}' not found", id)),
        Some(cat) if cat.is_builtin => {
            return Err("Cannot delete built-in categories".to_string())
        }
        _ => {}
    }
    
    // Check if this category is the default
    if settings.default_category_id == id {
        // Reset default to "development"
        settings.default_category_id = "development".to_string();
    }
    
    // Remove any app mappings that use this category
    settings.app_category_mappings.retain(|m| m.category_id != id);
    
    // Remove the category
    settings.prompt_categories.retain(|c| c.id != id);
    settings::write_settings(&app, settings);
    
    Ok(())
}

/// Update a category's name and icon (not prompt - use update_prompt_category for that)
#[tauri::command]
#[specta::specta]
pub fn update_prompt_category_details(
    app: AppHandle,
    id: String,
    name: String,
    icon: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    
    if let Some(category) = settings.prompt_categories.iter_mut().find(|c| c.id == id) {
        category.name = name;
        category.icon = icon;
        settings::write_settings(&app, settings);
        Ok(())
    } else {
        Err(format!("Category with id '{}' not found", id))
    }
}


// Voice command settings commands

#[tauri::command]
#[specta::specta]
pub fn change_voice_commands_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.voice_commands_enabled = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_voice_command_default_model_setting(
    app: AppHandle,
    model: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.voice_command_default_model = model;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn reset_voice_commands_to_default(app: AppHandle) -> Result<Vec<settings::VoiceCommand>, String> {
    let mut settings = settings::get_settings(&app);
    settings.voice_commands = settings::get_default_settings().voice_commands;
    let commands = settings.voice_commands.clone();
    settings::write_settings(&app, settings);
    Ok(commands)
}

#[tauri::command]
#[specta::specta]
pub fn change_filler_word_filter_setting(
    app: AppHandle,
    pattern: Option<String>,
) -> Result<(), String> {
    // Validate regex if provided
    if let Some(ref p) = pattern {
        if !p.is_empty() {
            regex::Regex::new(p).map_err(|e| format!("Invalid regex pattern: {}", e))?;
        }
    }
    let mut settings = settings::get_settings(&app);
    settings.filler_word_filter = pattern;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn add_voice_command(app: AppHandle, command: settings::VoiceCommand) -> Result<Vec<settings::VoiceCommand>, String> {
    let mut settings = settings::get_settings(&app);
    
    // Check for duplicate ID
    if settings.voice_commands.iter().any(|c| c.id == command.id) {
        return Err(format!("Command with ID '{}' already exists", command.id));
    }
    
    settings.voice_commands.push(command);
    let commands = settings.voice_commands.clone();
    settings::write_settings(&app, settings);
    Ok(commands)
}

#[tauri::command]
#[specta::specta]
pub fn update_voice_command(app: AppHandle, command: settings::VoiceCommand) -> Result<Vec<settings::VoiceCommand>, String> {
    let mut settings = settings::get_settings(&app);
    
    // Find and update the command
    if let Some(existing) = settings.voice_commands.iter_mut().find(|c| c.id == command.id) {
        *existing = command;
    } else {
        return Err(format!("Command with ID '{}' not found", command.id));
    }
    
    let commands = settings.voice_commands.clone();
    settings::write_settings(&app, settings);
    Ok(commands)
}

#[tauri::command]
#[specta::specta]
pub fn delete_voice_command(app: AppHandle, command_id: String) -> Result<Vec<settings::VoiceCommand>, String> {
    let mut settings = settings::get_settings(&app);
    
    let original_len = settings.voice_commands.len();
    settings.voice_commands.retain(|c| c.id != command_id);
    
    if settings.voice_commands.len() == original_len {
        return Err(format!("Command with ID '{}' not found", command_id));
    }
    
    let commands = settings.voice_commands.clone();
    settings::write_settings(&app, settings);
    Ok(commands)
}


/// Determine whether a shortcut string contains at least one non-modifier key.
/// We allow single non-modifier keys (e.g. "f5" or "space") but disallow
/// modifier-only combos (e.g. "ctrl" or "ctrl+shift").
///
/// On macOS, we also allow special raw modifier bindings like "right_option" and "left_option"
/// which are handled by a separate low-level event tap.
fn validate_shortcut_string(raw: &str) -> Result<(), String> {
    // On macOS, allow raw modifier bindings (handled separately from global shortcuts)
    #[cfg(target_os = "macos")]
    if key_listener::is_raw_modifier_binding(raw) {
        return Ok(());
    }

    let modifiers = [
        "ctrl", "control", "shift", "alt", "option", "meta", "command", "cmd", "super", "win",
        "windows",
    ];
    let has_non_modifier = raw
        .split('+')
        .any(|part| !modifiers.contains(&part.trim().to_lowercase().as_str()));
    if has_non_modifier {
        Ok(())
    } else {
        Err("Shortcut must contain at least one non-modifier key".into())
    }
}

/// Temporarily unregister a binding while the user is editing it in the UI.
/// This avoids firing the action while keys are being recorded.
#[tauri::command]
#[specta::specta]
pub fn suspend_binding(app: AppHandle, id: String) -> Result<(), String> {
    // Also suspend raw bindings on macOS
    #[cfg(target_os = "macos")]
    key_listener::suspend_raw_binding(&id);

    if let Some(b) = settings::get_bindings(&app).get(&id).cloned() {
        // Skip unregistering if it's a raw modifier binding (already suspended above)
        #[cfg(target_os = "macos")]
        if key_listener::is_raw_modifier_binding(&b.current_binding) {
            return Ok(());
        }

        if let Err(e) = unregister_shortcut(&app, b) {
            error!("suspend_binding error for id '{}': {}", id, e);
            return Err(e);
        }
    }
    Ok(())
}

/// Re-register the binding after the user has finished editing.
#[tauri::command]
#[specta::specta]
pub fn resume_binding(app: AppHandle, id: String) -> Result<(), String> {
    // Also resume raw bindings on macOS
    #[cfg(target_os = "macos")]
    key_listener::resume_raw_binding(&id);

    if let Some(b) = settings::get_bindings(&app).get(&id).cloned() {
        if let Err(e) = register_shortcut(&app, b) {
            error!("resume_binding error for id '{}': {}", id, e);
            return Err(e);
        }
    }
    Ok(())
}

pub fn register_shortcut(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {
    // Validate human-level rules first
    if let Err(e) = validate_shortcut_string(&binding.current_binding) {
        warn!(
            "_register_shortcut validation error for binding '{}': {}",
            binding.current_binding, e
        );
        return Err(e);
    }

    // On macOS, handle raw modifier bindings through the dedicated listener
    #[cfg(target_os = "macos")]
    if key_listener::is_raw_modifier_binding(&binding.current_binding) {
        return key_listener::register_raw_binding(&binding.id, &binding.current_binding);
    }

    // Parse shortcut and return error if it fails
    let shortcut = match binding.current_binding.parse::<Shortcut>() {
        Ok(s) => s,
        Err(e) => {
            let error_msg = format!(
                "Failed to parse shortcut '{}': {}",
                binding.current_binding, e
            );
            error!("_register_shortcut parse error: {}", error_msg);
            return Err(error_msg);
        }
    };

    // Prevent duplicate registrations that would silently shadow one another
    if app.global_shortcut().is_registered(shortcut) {
        let error_msg = format!("Shortcut '{}' is already in use", binding.current_binding);
        warn!("_register_shortcut duplicate error: {}", error_msg);
        return Err(error_msg);
    }

    let reg_result = app.global_shortcut().register(shortcut);
    match reg_result {
        Ok(_) => debug!("Successfully registered shortcut: {} (id={})", binding.current_binding, binding.id),
        Err(e) => {
            error!("Failed to register shortcut '{}' (id={}): {}", binding.current_binding, binding.id, e);
            return Err(e.to_string());
        }
    }

    // Clone binding.id for use in the closure
    let binding_id_for_closure = binding.id.clone();

    app.global_shortcut()
        .on_shortcut(shortcut, move |ah, scut, event| {
            if scut == &shortcut {
                let shortcut_string = scut.into_string();
                debug!(
                    "[KEY] Shortcut event received: shortcut='{}' binding_id='{}' state={:?}",
                    shortcut_string, binding_id_for_closure, event.state
                );

                if let Some(action) = ACTION_MAP.get(&binding_id_for_closure) {
                    if binding_id_for_closure == "cancel" {
                        if event.state == ShortcutState::Pressed {
                            debug!("[KEY] Cancel shortcut activated");
                            action.start(ah, &binding_id_for_closure, &shortcut_string);
                        }
                        return;
                    }
                    
                    // Smart tap/hold detection for all other bindings
                    match event.state {
                        ShortcutState::Pressed => {
                            debug!(
                                "[TOGGLE] Processing PRESSED event for binding_id='{}'",
                                binding_id_for_closure
                            );
                            // Record press timestamp
                            if let Ok(mut timestamps) = get_press_timestamps().lock() {
                                timestamps.insert(binding_id_for_closure.clone(), Instant::now());
                            }
                            
                            // Check if already recording (toggle-off tap)
                            let toggle_state_manager = ah.state::<ManagedToggleState>();
                            {
                                let mut states = toggle_state_manager
                                    .lock()
                                    .expect("Failed to lock toggle state manager");
                                let is_active = states
                                    .active_toggles
                                    .entry(binding_id_for_closure.clone())
                                    .or_insert(false);
                                
                                debug!(
                                    "[TOGGLE] Current active_toggles['{}'] = {}",
                                    binding_id_for_closure, *is_active
                                );
                                
                                if *is_active {
                                    // Already recording - this is a toggle-off tap
                                    *is_active = false;
                                    debug!(
                                        "[TOGGLE] Shortcut {} toggle stop (tap while active)",
                                        shortcut_string
                                    );
                                    drop(states);
                                    action.stop(ah, &binding_id_for_closure, &shortcut_string);
                                    return;
                                }
                                
                                // Start recording
                                *is_active = true;
                                debug!(
                                    "[TOGGLE] Setting active_toggles['{}'] = true (starting recording)",
                                    binding_id_for_closure
                                );
                            }
                            debug!("[TOGGLE] Shortcut {} start recording - calling action.start()", shortcut_string);
                            let started = action.start(ah, &binding_id_for_closure, &shortcut_string);
                            debug!("[TOGGLE] action.start() returned: {}", started);
                            
                            // If start failed, reset the toggle state
                            if !started {
                                debug!(
                                    "[TOGGLE] action.start() returned false, resetting active_toggles['{}'] = false",
                                    binding_id_for_closure
                                );
                                let toggle_state_manager = ah.state::<ManagedToggleState>();
                                if let Ok(mut states) = toggle_state_manager.lock() {
                                    states.active_toggles.insert(binding_id_for_closure.clone(), false);
                                };
                            } else {
                                // Successfully started recording - spawn a timer to emit "hold" mode after threshold
                                // This allows the "Raw" label to appear while user is still holding
                                let settings = get_settings(ah);
                                let threshold = settings.hold_threshold_ms as u64;
                                let ah_clone = ah.clone();
                                let binding_id_clone = binding_id_for_closure.clone();
                                
                                    std::thread::spawn(move || {
                                        std::thread::sleep(std::time::Duration::from_millis(threshold));
                                        
                                        // Check if still physically pressed AND recording is still active
                                        let is_still_physically_pressed = get_press_timestamps()
                                            .lock()
                                            .ok()
                                            .map(|t| t.contains_key(&binding_id_clone))
                                            .unwrap_or(false);

                                        let toggle_state_manager = ah_clone.state::<ManagedToggleState>();
                                        let is_still_active = toggle_state_manager
                                            .lock()
                                            .ok()
                                            .and_then(|s| s.active_toggles.get(&binding_id_clone).copied())
                                            .unwrap_or(false);
                                        
                                        if is_still_physically_pressed && is_still_active {
                                            // User has been holding for threshold ms - this is "hold" mode
                                            debug!("[TOGGLE] Threshold passed while still holding - emitting hold mode");
                                            overlay::emit_mode_determined(&ah_clone, "hold");
                                        }
                                    });
                            }
                        }
                        ShortcutState::Released => {
                            debug!(
                                "[TOGGLE] Processing RELEASED event for binding_id='{}'",
                                binding_id_for_closure
                            );
                            // Get press timestamp and calculate hold duration
                            let hold_duration_ms = if let Ok(mut timestamps) = get_press_timestamps().lock() {
                                timestamps.remove(&binding_id_for_closure)
                                    .map(|t| t.elapsed().as_millis())
                                    .unwrap_or(0)
                            } else {
                                0
                            };
                            
                            // Get threshold from settings
                            let settings = get_settings(ah);
                            let threshold = settings.hold_threshold_ms as u128;
                            
                            debug!(
                                "[TOGGLE] hold_duration={}ms threshold={}ms",
                                hold_duration_ms, threshold
                            );
                            
                            if hold_duration_ms >= threshold {
                                // Long hold - PTT behavior, stop immediately
                                let toggle_state_manager = ah.state::<ManagedToggleState>();
                                {
                                    let mut states = toggle_state_manager
                                        .lock()
                                        .expect("Failed to lock toggle state manager");
                                    debug!(
                                        "[TOGGLE] PTT mode: setting active_toggles['{}'] = false",
                                        binding_id_for_closure
                                    );
                                    states.active_toggles.insert(binding_id_for_closure.clone(), false);
                                }
                                debug!(
                                    "[TOGGLE] Shortcut {} released after {}ms (PTT stop) - calling action.stop()",
                                    shortcut_string, hold_duration_ms
                                );
                                
                                // Emit hold mode so UI can show "Raw" briefly before transitioning
                                overlay::emit_mode_determined(ah, "hold");
                                
                                action.stop(ah, &binding_id_for_closure, &shortcut_string);
                            } else {
                                // Quick tap - toggle mode = COHERENT mode in unified UX
                                // CRITICAL: Only emit if we are still active (i.e. this was the START tap).
                                // If we just stopped on Pressed, active_toggles will be false now.
                                let is_still_active = {
                                    let toggle_state_manager = ah.state::<ManagedToggleState>();
                                    let states = toggle_state_manager
                                        .lock()
                                        .expect("Failed to lock toggle state manager");
                                    *states.active_toggles.get(&binding_id_for_closure).unwrap_or(&false)
                                };

                                debug!(
                                    "[TOGGLE] Shortcut {} released after {}ms. is_still_active={}",
                                    shortcut_string, hold_duration_ms, is_still_active
                                );

                                if is_still_active {
                                    // Quick press = coherent mode (unified hotkey UX)
                                    let audio_manager = ah.state::<Arc<AudioRecordingManager>>();
                                    audio_manager.set_coherent_mode(true);
                                    
                                    // Emit refining mode and update overlay SYNCHRONOUSLY
                                    // Ensure the state becomes 'ramble_recording' so UI shows 'Refined' label
                                    crate::utils::show_ramble_recording_overlay(ah);
                                    overlay::emit_mode_determined(ah, "refining");
                                    
                                    // Spawn async ONLY for clipboard copy
                                    let ah_clone = ah.clone();
                                    let audio_manager_clone = Arc::clone(&audio_manager);
                                    // Run on main thread to prevent crash on macOS (TSM/Enigo requirements)
                                    let _ = ah.run_on_main_thread(move || {
                                        // Capture selection context for coherent processing
                                        if let Ok(Some(text)) = crate::clipboard::get_selected_text(&ah_clone) {
                                            debug!("Captured selection context: {} chars", text.len());
                                            audio_manager_clone.set_selection_context(text);
                                        }
                                    });
                                }
                            }
                        }
                    }
                } else {
                    // Handle dynamic/contextual shortcuts (Pause, Vision)
                    let audio_manager = ah.state::<Arc<AudioRecordingManager>>();
                    let is_active = audio_manager.is_recording() || audio_manager.get_paused_binding_id().is_some();

                    if !is_active && binding_id_for_closure != "cancel" {
                        debug!("[KEY] Ignoring contextual shortcut '{}' - not recording or paused", binding_id_for_closure);
                        return;
                    }

                    match binding_id_for_closure.as_str() {
                        "pause_toggle" => {
                            if event.state == ShortcutState::Pressed {
                                debug!("[KEY] Pause toggle shortcut activated");
                                let app_handle = ah.clone();
                                tauri::async_runtime::spawn(async move {
                                    crate::commands::pause_operation(app_handle);
                                });
                            }
                        }
                        "vision_capture" => {
                            if event.state == ShortcutState::Pressed {
                                debug!("[KEY] Vision capture shortcut activated");
                                let app_handle = ah.clone();
                                tauri::async_runtime::spawn(async move {
                                    match crate::vision::capture_screen() {
                                        Ok(base64) => {
                                            let audio_manager = app_handle.state::<Arc<AudioRecordingManager>>();
                                            audio_manager.add_vision_context(base64);
                                            // Pulse the overlay to show feedback
                                            let _ = app_handle.emit("vision-captured", ());
                                        }
                                        Err(e) => {
                                            error!("Vision capture failed: {}", e);
                                        }
                                    }
                                });
                            }
                        }
                        _ => {
                            warn!(
                                "No action defined in ACTION_MAP for shortcut ID '{}'. Shortcut: '{}', State: {:?}",
                                binding_id_for_closure, shortcut_string, event.state
                            );
                        }
                    }
                }
            }
        })
        .map_err(|e| {
            let error_msg = format!("Couldn't register shortcut '{}': {}", binding.current_binding, e);
            error!("_register_shortcut registration error: {}", error_msg);
            error_msg
        })?;

    Ok(())
}

pub fn unregister_shortcut(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {
    // On macOS, handle raw modifier bindings through the dedicated listener
    #[cfg(target_os = "macos")]
    if key_listener::is_raw_modifier_binding(&binding.current_binding) {
        return key_listener::unregister_raw_binding(&binding.current_binding);
    }

    let shortcut = match binding.current_binding.parse::<Shortcut>() {
        Ok(s) => s,
        Err(e) => {
            let error_msg = format!(
                "Failed to parse shortcut '{}' for unregistration: {}",
                binding.current_binding, e
            );
            error!("_unregister_shortcut parse error: {}", error_msg);
            return Err(error_msg);
        }
    };

    app.global_shortcut().unregister(shortcut).map_err(|e| {
        let error_msg = format!(
            "Failed to unregister shortcut '{}': {}",
            binding.current_binding, e
        );
        error!("_unregister_shortcut error: {}", error_msg);
        error_msg
    })?;

    Ok(())
}

/// Register multiple shortcut variants for the same action to ensure "swallowing" works 
/// regardless of whether the user holds Shift or other modifiers.
fn register_swallowing_shortcuts(app: &AppHandle, binding: ShortcutBinding) {
    let base_binding = binding.current_binding.clone();
    let id = binding.id.clone();
    
    // Register the primary binding
    if let Err(e) = register_shortcut(app, binding.clone()) {
        debug!("Primary swallowing shortcut {} for {} already registered or failed: {}", base_binding, id, e);
    }

    // Register a variant without Shift if it was something like Option+Shift+P
    // but the user might just press Option+P.
    let variants = if id == "pause_toggle" {
        vec!["Option+P", "Alt+P"]
    } else if id == "vision_capture" {
        vec!["Option+S", "Alt+S"]
    } else {
        vec![]
    };

    for variant in variants {
        if variant.to_lowercase() != base_binding.to_lowercase() {
            let mut v_binding = binding.clone();
            v_binding.current_binding = variant.to_string();
            if let Err(e) = register_shortcut(app, v_binding) {
                 debug!("Variant swallowing shortcut {} for {} already registered or failed: {}", variant, id, e);
            }
        }
    }
}
