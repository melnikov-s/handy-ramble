// Unified LLM Provider Commands
//
// This module provides Tauri commands for managing LLM providers and models.
// It replaces the deprecated post_process_* and ramble_* settings commands.

use crate::settings::{self, LLMModel, LLMProvider};
use tauri::AppHandle;

/// Get all configured LLM providers, deduplicated by ID
#[tauri::command]
#[specta::specta]
pub fn get_llm_providers(app: AppHandle) -> Vec<LLMProvider> {
    let settings = settings::get_settings(&app);
    let mut providers = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for provider in settings.llm_providers {
        if seen.insert(provider.id.clone()) {
            providers.push(provider);
        }
    }
    providers
}

/// Get all configured LLM models with their provider info, deduplicated by (provider_id, model_id)
#[tauri::command]
#[specta::specta]
pub fn get_llm_models(app: AppHandle) -> Vec<LLMModel> {
    let settings = settings::get_settings(&app);
    let mut models = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for model in settings.llm_models {
        let key = format!("{}:{}", model.provider_id, model.model_id);
        if seen.insert(key) {
            models.push(model);
        }
    }
    models
}

/// Update an LLM provider's API key
#[tauri::command]
#[specta::specta]
pub fn update_provider_api_key(
    app: AppHandle,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    let provider = settings
        .llm_providers
        .iter_mut()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    provider.api_key = api_key;
    settings::write_settings(&app, settings);
    Ok(())
}

/// Save (create or update) an LLM provider
#[tauri::command]
#[specta::specta]
pub fn save_llm_provider(app: AppHandle, provider: LLMProvider) -> Result<LLMProvider, String> {
    let mut settings = settings::get_settings(&app);

    // Check if provider already exists
    if let Some(existing) = settings
        .llm_providers
        .iter_mut()
        .find(|p| p.id == provider.id)
    {
        // Update existing provider
        existing.name = provider.name.clone();
        existing.base_url = provider.base_url.clone();
        existing.api_key = provider.api_key.clone();
        existing.supports_vision = provider.supports_vision;
        // Don't update is_custom - preserve the original value
    } else {
        // Add new provider
        settings.llm_providers.push(provider.clone());
    }

    settings::write_settings(&app, settings);
    Ok(provider)
}

/// Delete an LLM provider (any provider can be deleted)
#[tauri::command]
#[specta::specta]
pub fn delete_llm_provider(app: AppHandle, provider_id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // Check provider exists
    if !settings.llm_providers.iter().any(|p| p.id == provider_id) {
        return Err(format!("Provider '{}' not found", provider_id));
    }

    // Remove the provider
    settings.llm_providers.retain(|p| p.id != provider_id);

    // Also remove any models associated with this provider
    settings.llm_models.retain(|m| m.provider_id != provider_id);

    settings::write_settings(&app, settings);
    Ok(())
}

/// Save (create or update) an LLM model
#[tauri::command]
#[specta::specta]
pub fn save_llm_model(app: AppHandle, model: LLMModel) -> Result<LLMModel, String> {
    let mut settings = settings::get_settings(&app);

    // Validate that the provider exists
    if !settings
        .llm_providers
        .iter()
        .any(|p| p.id == model.provider_id)
    {
        return Err(format!("Provider '{}' not found", model.provider_id));
    }

    // Check if model already exists
    if let Some(existing) = settings.llm_models.iter_mut().find(|m| m.id == model.id) {
        // Update existing model
        existing.provider_id = model.provider_id.clone();
        existing.model_id = model.model_id.clone();
        existing.display_name = model.display_name.clone();
        existing.supports_vision = model.supports_vision;
        existing.enabled = model.enabled; // CRITICAL: persist the enabled state
    } else {
        // Add new model
        settings.llm_models.push(model.clone());
    }

    settings::write_settings(&app, settings);
    Ok(model)
}

/// Delete an LLM model
#[tauri::command]
#[specta::specta]
pub fn delete_llm_model(app: AppHandle, model_id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    let original_len = settings.llm_models.len();
    settings.llm_models.retain(|m| m.id != model_id);

    if settings.llm_models.len() == original_len {
        return Err(format!("Model '{}' not found", model_id));
    }

    // Clear default selections if this model was selected
    if settings.default_chat_model_id.as_ref() == Some(&model_id) {
        settings.default_chat_model_id = None;
    }
    if settings.default_coherent_model_id.as_ref() == Some(&model_id) {
        settings.default_coherent_model_id = None;
    }
    if settings.default_voice_model_id.as_ref() == Some(&model_id) {
        settings.default_voice_model_id = None;
    }
    if settings.default_context_chat_model_id.as_ref() == Some(&model_id) {
        settings.default_context_chat_model_id = None;
    }

    settings::write_settings(&app, settings);
    Ok(())
}

/// Set the default model for a specific feature
#[tauri::command]
#[specta::specta]
pub fn set_default_model(
    app: AppHandle,
    feature: String,
    model_id: Option<String>,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // Validate model exists if specified
    if let Some(ref id) = model_id {
        if !settings.llm_models.iter().any(|m| &m.id == id) {
            return Err(format!("Model '{}' not found", id));
        }
    }

    // Update the appropriate default
    match feature.as_str() {
        "chat" => settings.default_chat_model_id = model_id,
        "coherent" => settings.default_coherent_model_id = model_id,
        "voice" => settings.default_voice_model_id = model_id,
        "context_chat" => settings.default_context_chat_model_id = model_id,
        _ => {
            return Err(format!(
                "Unknown feature '{}'. Valid: chat, coherent, voice, context_chat",
                feature
            ))
        }
    }

    settings::write_settings(&app, settings);
    Ok(())
}

/// Get default model IDs for all features
#[tauri::command]
#[specta::specta]
pub fn get_default_models(app: AppHandle) -> DefaultModels {
    let settings = settings::get_settings(&app);
    DefaultModels {
        chat: settings.default_chat_model_id.clone(),
        coherent: settings.default_coherent_model_id.clone(),
        voice: settings.default_voice_model_id.clone(),
        context_chat: settings.default_context_chat_model_id.clone(),
    }
}

#[derive(serde::Serialize, specta::Type)]
pub struct DefaultModels {
    pub chat: Option<String>,
    pub coherent: Option<String>,
    pub voice: Option<String>,
    pub context_chat: Option<String>,
}
