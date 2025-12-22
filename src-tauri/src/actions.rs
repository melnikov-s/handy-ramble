#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use crate::apple_intelligence;
use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::HistoryManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, AppSettings, APPLE_INTELLIGENCE_PROVIDER_ID};
use crate::shortcut;
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{
    self, is_operation_paused, resume_current_operation, show_making_coherent_overlay,
    show_ramble_recording_overlay, show_ramble_transcribing_overlay, show_recording_overlay,
    show_transcribing_overlay,
};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tauri::AppHandle;
use tauri::Manager;

use crate::ManagedToggleState;

// Shortcut Action Trait
pub trait ShortcutAction: Send + Sync {
    /// Start the action. Returns true if the action started successfully, false otherwise.
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) -> bool;
    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
}

// Transcribe Action
struct TranscribeAction;

/// Extract a human-readable error message from LLM API errors
fn extract_llm_error(error: &dyn std::error::Error, model: &str) -> String {
    let error_str = error.to_string();
    let lower_error = error_str.to_lowercase();

    if lower_error.contains("401")
        || lower_error.contains("unauthorized")
        || lower_error.contains("invalid_api_key")
    {
        "Invalid API key".to_string()
    } else if lower_error.contains("429")
        || lower_error.contains("rate limit")
        || lower_error.contains("too many requests")
        || lower_error.contains("resource_exhausted")
    {
        "Rate limited - try again".to_string()
    } else if lower_error.contains("model") || lower_error.contains("404") {
        format!("Invalid model: {}", model)
    } else if lower_error.contains("500") || lower_error.contains("503") {
        "AI service unavailable".to_string()
    } else {
        format!("API error: {}", error_str)
    }
}

async fn maybe_post_process_transcription(
    app: &AppHandle,
    settings: &AppSettings,
    transcription: &str,
) -> Result<Option<String>, String> {
    // If this is called, we process. The caller (TranscribeAction) should check settings.post_process_enabled.
    info!(
        "Starting LLM post-processing for transcription ({} chars)",
        transcription.len()
    );
    utils::log_to_frontend(app, "info", "Starting post-processing...");

    let provider = match settings.active_post_process_provider().cloned() {
        Some(provider) => provider,
        None => {
            let msg = "Post-processing enabled but no provider is selected";
            utils::log_to_frontend(app, "error", msg);
            debug!("{}", msg);
            return Err(msg.to_string());
        }
    };

    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    if model.trim().is_empty() {
        let msg = format!("Provider '{}' has no model configured", provider.id);
        utils::log_to_frontend(app, "error", &msg);
        debug!("{}", msg);
        return Err(msg.to_string());
    }

    let selected_prompt_id = match &settings.post_process_selected_prompt_id {
        Some(id) => id.clone(),
        None => {
            let msg = "No post-processing prompt is selected";
            debug!("{}", msg);
            return Err(msg.to_string());
        }
    };

    let prompt = match settings
        .post_process_prompts
        .iter()
        .find(|prompt| prompt.id == selected_prompt_id)
    {
        Some(prompt) => prompt.prompt.clone(),
        None => {
            let msg = format!("Prompt '{}' was not found", selected_prompt_id);
            debug!("{}", msg);
            return Err(msg.to_string());
        }
    };

    if prompt.trim().is_empty() {
        let msg = "The selected post-processing prompt is empty";
        debug!("{}", msg);
        return Err(msg.to_string());
    }

    info!(
        "Starting LLM post-processing with provider '{}' (model: {})",
        provider.id, model
    );

    // Replace ${output} variable in the prompt with the actual text
    let processed_prompt = prompt.replace("${output}", transcription);
    debug!("Processed prompt length: {} chars", processed_prompt.len());

    if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            if !apple_intelligence::check_apple_intelligence_availability() {
                let msg = "Apple Intelligence is not currently available on this device";
                debug!("{}", msg);
                return Err(msg.to_string());
            }

            let token_limit = model.trim().parse::<i32>().unwrap_or(0);
            return match apple_intelligence::process_text(&processed_prompt, token_limit) {
                Ok(result) => {
                    if result.trim().is_empty() {
                        let msg = "Apple Intelligence returned an empty response";
                        debug!("{}", msg);
                        Err(msg.to_string())
                    } else {
                        info!(
                            "Apple Intelligence post-processing succeeded. Output length: {} chars",
                            result.len()
                        );
                        utils::log_to_frontend(app, "info", "Post-processing complete");
                        Ok(Some(result))
                    }
                }
                Err(err) => {
                    let msg = format!("Apple Intelligence post-processing failed: {}", err);
                    error!("{}", msg);
                    Err(msg)
                }
            };
        }

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let msg = "Apple Intelligence provider selected on unsupported platform";
            debug!("{}", msg);
            return Err(msg.to_string());
        }
    }

    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    // Create OpenAI-compatible client
    let client = match crate::llm_client::create_client(&provider, api_key) {
        Ok(client) => client,
        Err(e) => {
            let msg = format!("Failed to create LLM client: {}", e);
            utils::log_to_frontend(app, "error", &msg);
            error!("{}", msg);
            return Err(msg);
        }
    };

    // Build the chat completion request
    let message = match ChatCompletionRequestUserMessageArgs::default()
        .content(processed_prompt)
        .build()
    {
        Ok(msg) => ChatCompletionRequestMessage::User(msg),
        Err(e) => {
            let msg = format!("Failed to build chat message: {}", e);
            error!("{}", msg);
            return Err(msg);
        }
    };

    let request = match CreateChatCompletionRequestArgs::default()
        .model(&model)
        .messages(vec![message])
        .build()
    {
        Ok(req) => req,
        Err(e) => {
            let msg = format!("Failed to build chat completion request: {}", e);
            error!("{}", msg);
            return Err(msg);
        }
    };

    // Send the request
    match client.chat().create(request).await {
        Ok(response) => {
            if let Some(choice) = response.choices.first() {
                if let Some(content) = &choice.message.content {
                    info!(
                        "LLM post-processing succeeded for provider '{}'. Output length: {} chars",
                        provider.id,
                        content.len()
                    );
                    utils::log_to_frontend(app, "info", "Post-processing complete");
                    return Ok(Some(content.clone()));
                }
            }
            let msg = "LLM API response has no content".to_string();
            error!("{}", msg);
            Err(msg)
        }
        Err(e) => {
            let error_msg = extract_llm_error(&e, &model);
            let msg = format!(
                "LLM post-processing failed for provider '{}': {}",
                provider.id, error_msg
            );
            utils::log_to_frontend(app, "error", &msg);
            error!("{}", msg);
            Err(error_msg)
        }
    }
}

async fn maybe_convert_chinese_variant(
    settings: &AppSettings,
    transcription: &str,
) -> Option<String> {
    // Check if language is set to Simplified or Traditional Chinese
    let is_simplified = settings.selected_language == "zh-Hans";
    let is_traditional = settings.selected_language == "zh-Hant";

    if !is_simplified && !is_traditional {
        debug!("selected_language is not Simplified or Traditional Chinese; skipping translation");
        return None;
    }

    debug!(
        "Starting Chinese translation using OpenCC for language: {}",
        settings.selected_language
    );

    // Use OpenCC to convert based on selected language
    let config = if is_simplified {
        // Convert Traditional Chinese to Simplified Chinese
        BuiltinConfig::Tw2sp
    } else {
        // Convert Simplified Chinese to Traditional Chinese
        BuiltinConfig::S2twp
    };

    match OpenCC::from_config(config) {
        Ok(converter) => {
            let converted = converter.convert(transcription);
            debug!(
                "OpenCC translation completed. Input length: {}, Output length: {}",
                transcription.len(),
                converted.len()
            );
            Some(converted)
        }
        Err(e) => {
            error!("Failed to initialize OpenCC converter: {}. Falling back to original transcription.", e);
            None
        }
    }
}

impl ShortcutAction for TranscribeAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) -> bool {
        let start_time = Instant::now();
        debug!(
            "[ACTION] TranscribeAction::start called for binding: {}",
            binding_id
        );

        // Check if we're resuming from a paused state
        if is_operation_paused(app, binding_id) {
            debug!("Resuming paused transcription for binding: {}", binding_id);
            resume_current_operation(app);
            return true;
        }

        // Load model in the background
        let tm = app.state::<Arc<TranscriptionManager>>();
        tm.initiate_model_load();

        let binding_id = binding_id.to_string();
        change_tray_icon(app, TrayIconState::Recording);
        show_recording_overlay(app);

        let rm = app.state::<Arc<AudioRecordingManager>>();

        // Get the microphone mode to determine audio feedback timing
        let settings = get_settings(app);
        let is_always_on = settings.always_on_microphone;
        debug!("Microphone mode - always_on: {}", is_always_on);

        let mut recording_started = false;
        if is_always_on {
            // Always-on mode: Play audio feedback immediately, then apply mute after sound finishes
            debug!("Always-on mode: Playing audio feedback immediately");
            let rm_clone = Arc::clone(&rm);
            let app_clone = app.clone();
            // The blocking helper exits immediately if audio feedback is disabled,
            // so we can always reuse this thread to ensure mute happens right after playback.
            std::thread::spawn(move || {
                play_feedback_sound_blocking(&app_clone, SoundType::Start);
                rm_clone.apply_mute();
            });

            recording_started = rm.try_start_recording(&binding_id);
            debug!(
                "[ACTION] try_start_recording returned: {}",
                recording_started
            );
        } else {
            // On-demand mode: Start recording first, then play audio feedback, then apply mute
            // This allows the microphone to be activated before playing the sound
            debug!("On-demand mode: Starting recording first, then audio feedback");
            let recording_start_time = Instant::now();
            if rm.try_start_recording(&binding_id) {
                recording_started = true;
                debug!("Recording started in {:?}", recording_start_time.elapsed());
                // Small delay to ensure microphone stream is active
                let app_clone = app.clone();
                let rm_clone = Arc::clone(&rm);
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    debug!("Handling delayed audio feedback/mute sequence");
                    // Helper handles disabled audio feedback by returning early, so we reuse it
                    // to keep mute sequencing consistent in every mode.
                    play_feedback_sound_blocking(&app_clone, SoundType::Start);
                    rm_clone.apply_mute();
                });
            } else {
                debug!("Failed to start recording");
            }
        }

        if recording_started {
            // Dynamically register the cancel shortcut in a separate task to avoid deadlock
            shortcut::register_cancel_shortcut(app);
        }

        debug!(
            "TranscribeAction::start completed in {:?}, returning {}",
            start_time.elapsed(),
            recording_started
        );
        recording_started
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        // Unregister the cancel shortcut when transcription stops
        shortcut::unregister_cancel_shortcut(app);

        // Reset toggle state so next press starts fresh
        let toggle_state_manager = app.state::<ManagedToggleState>();
        if let Ok(mut states) = toggle_state_manager.lock() {
            states.active_toggles.insert(binding_id.to_string(), false);
            debug!(
                "[ACTION] Reset active_toggles['{}'] = false in TranscribeAction::stop",
                binding_id
            );
        } else {
            warn!("Failed to lock toggle state manager in TranscribeAction::stop");
        }

        let stop_time = Instant::now();
        debug!("TranscribeAction::stop called for binding: {}", binding_id);

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
        let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());

        change_tray_icon(app, TrayIconState::Transcribing);
        show_transcribing_overlay(app);

        // Unmute before playing audio feedback so the stop sound is audible
        rm.remove_mute();

        // Play audio feedback for recording stop
        play_feedback_sound(app, SoundType::Stop);

        // Unmute before playing audio feedback so the stop sound is audible
        rm.remove_mute();

        // Play audio feedback for recording stop
        play_feedback_sound(app, SoundType::Stop);

        let binding_id = binding_id.to_string(); // Clone binding_id for the async task

        tauri::async_runtime::spawn(async move {
            let binding_id = binding_id.clone(); // Clone for the inner async task
            debug!(
                "Starting async transcription task for binding: {}",
                binding_id
            );

            let stop_recording_time = Instant::now();
            if let Some(samples) = rm.stop_recording(&binding_id) {
                debug!(
                    "Recording stopped and samples retrieved in {:?}, sample count: {}",
                    stop_recording_time.elapsed(),
                    samples.len()
                );
                let transcription_time = Instant::now();
                let samples_clone = samples.clone(); // Clone for history saving
                match tm.transcribe(samples) {
                    Ok(transcription) => {
                        debug!(
                            "Transcription completed in {:?}: '{}'",
                            transcription_time.elapsed(),
                            transcription
                        );
                        if !transcription.is_empty() {
                            let settings = get_settings(&ah);
                            let mut final_text = transcription.clone();
                            let mut post_processed_text: Option<String> = None;
                            let mut post_process_prompt: Option<String> = None;

                            // First, check if Chinese variant conversion is needed
                            if let Some(converted_text) =
                                maybe_convert_chinese_variant(&settings, &transcription).await
                            {
                                final_text = converted_text.clone();
                                post_processed_text = Some(converted_text);
                            }
                            // Then apply regular post-processing if enabled
                            else if settings.post_process_enabled {
                                match maybe_post_process_transcription(
                                    &ah,
                                    &settings,
                                    &transcription,
                                )
                                .await
                                {
                                    Ok(Some(processed_text)) => {
                                        final_text = processed_text.clone();
                                        post_processed_text = Some(processed_text);

                                        // Get the prompt that was used
                                        if let Some(prompt_id) =
                                            &settings.post_process_selected_prompt_id
                                        {
                                            if let Some(prompt) = settings
                                                .post_process_prompts
                                                .iter()
                                                .find(|p| &p.id == prompt_id)
                                            {
                                                post_process_prompt = Some(prompt.prompt.clone());
                                            }
                                        }
                                    }
                                    Ok(None) => {
                                        // Post-processing disabled, use original
                                    }
                                    Err(error_msg) => {
                                        // Show error overlay and return without pasting
                                        error!("Post-processing failed: {}", error_msg);
                                        utils::show_error_overlay(&ah, &error_msg);
                                        change_tray_icon(&ah, TrayIconState::Idle);
                                        return;
                                    }
                                }
                            }

                            // Save to history with post-processed text and prompt
                            let hm_clone = Arc::clone(&hm);
                            let transcription_for_history = transcription.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = hm_clone
                                    .save_transcription(
                                        samples_clone,
                                        transcription_for_history,
                                        post_processed_text,
                                        post_process_prompt,
                                    )
                                    .await
                                {
                                    error!("Failed to save transcription to history: {}", e);
                                }
                            });

                            // Paste the final text (either processed or original)
                            let ah_clone = ah.clone();
                            let paste_time = Instant::now();
                            ah.run_on_main_thread(move || {
                                match utils::paste(final_text, ah_clone.clone()) {
                                    Ok(()) => debug!(
                                        "Text pasted successfully in {:?}",
                                        paste_time.elapsed()
                                    ),
                                    Err(e) => error!("Failed to paste transcription: {}", e),
                                }
                                // Hide the overlay after transcription is complete
                                utils::hide_recording_overlay(&ah_clone);
                                change_tray_icon(&ah_clone, TrayIconState::Idle);
                            })
                            .unwrap_or_else(|e| {
                                error!("Failed to run paste on main thread: {:?}", e);
                                utils::hide_recording_overlay(&ah);
                                change_tray_icon(&ah, TrayIconState::Idle);
                            });
                        } else {
                            utils::hide_recording_overlay(&ah);
                            change_tray_icon(&ah, TrayIconState::Idle);
                        }
                    }
                    Err(err) => {
                        debug!("Global Shortcut Transcription error: {}", err);
                        utils::hide_recording_overlay(&ah);
                        change_tray_icon(&ah, TrayIconState::Idle);
                    }
                }
            } else {
                debug!("No samples retrieved from recording stop");
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
            }
        });

        debug!(
            "TranscribeAction::stop completed in {:?}",
            stop_time.elapsed()
        );
    }
}

// Ramble to Coherent Action
struct RambleToCoherentAction;

/// Process transcription through LLM using ramble-specific settings
/// Returns Ok(Some(processed)) on success, Ok(None) if disabled/skipped, Err(msg) on error
async fn process_ramble_to_coherent(
    app: &AppHandle,
    settings: &AppSettings,
    transcription: &str,
) -> Result<Option<String>, String> {
    // If the shortcut is pressed, we ALWAYS process regardless of ramble_enabled setting.
    // The setting is mostly for UI/default state.
    info!(
        "Starting Ramble to Coherent processing ({} chars)",
        transcription.len()
    );
    utils::log_to_frontend(app, "info", "Starting refinement...");

    let provider_id = &settings.ramble_provider_id;
    let provider = match settings
        .post_process_providers
        .iter()
        .find(|p| &p.id == provider_id)
        .cloned()
    {
        Some(provider) => provider,
        None => {
            let msg = format!("Provider '{}' not found", provider_id);
            utils::log_to_frontend(app, "error", &msg);
            return Err(msg);
        }
    };

    let model = &settings.ramble_model;
    if model.trim().is_empty() {
        let msg = "No model configured".to_string();
        utils::log_to_frontend(app, "error", &msg);
        return Err(msg);
    }

    let prompt = &settings.ramble_prompt;
    if prompt.trim().is_empty() {
        let msg = "Prompt is empty".to_string();
        utils::log_to_frontend(app, "error", &msg);
        return Err(msg);
    }

    info!(
        "Starting Ramble to Coherent with provider '{}' (model: {})",
        provider.id, model
    );

    // Replace ${output} variable in the prompt with the actual text
    let processed_prompt = prompt.replace("${output}", transcription);

    // Get API key from post_process_api_keys (reuses same keys)
    let api_key = settings
        .post_process_api_keys
        .get(provider_id)
        .cloned()
        .unwrap_or_default();

    if api_key.is_empty() {
        let msg = "API key not configured".to_string();
        utils::log_to_frontend(app, "error", &msg);
        return Err(msg);
    }

    // Create OpenAI-compatible client
    let client = match crate::llm_client::create_client(&provider, api_key) {
        Ok(client) => client,
        Err(e) => {
            return Err(format!("Failed to create client: {}", e));
        }
    };

    // Build the chat completion request
    let message = match ChatCompletionRequestUserMessageArgs::default()
        .content(processed_prompt)
        .build()
    {
        Ok(msg) => ChatCompletionRequestMessage::User(msg),
        Err(e) => {
            return Err(format!("Request error: {}", e));
        }
    };

    let request = match CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(vec![message])
        .build()
    {
        Ok(req) => req,
        Err(e) => {
            return Err(format!("Request error: {}", e));
        }
    };

    // Send the request
    match client.chat().create(request).await {
        Ok(response) => {
            if let Some(choice) = response.choices.first() {
                if let Some(content) = &choice.message.content {
                    info!(
                        "Ramble to Coherent succeeded. Output length: {} chars",
                        content.len()
                    );
                    utils::log_to_frontend(app, "info", "Refinement complete");
                    return Ok(Some(content.clone()));
                }
            }
            Err("No response from AI".to_string())
        }
        Err(e) => Err(extract_llm_error(&e, model)),
    }
}

impl ShortcutAction for RambleToCoherentAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) -> bool {
        let start_time = Instant::now();
        debug!(
            "RambleToCoherentAction::start called for binding: {}",
            binding_id
        );

        // Check if we're resuming from a paused state
        if is_operation_paused(app, binding_id) {
            debug!(
                "Resuming paused ramble recording for binding: {}",
                binding_id
            );
            resume_current_operation(app);
            return true;
        }

        debug!(
            "RambleToCoherentAction::start called for binding: {}. Attempting to start recording.",
            binding_id
        );

        // Load model in the background
        let tm = app.state::<Arc<TranscriptionManager>>();
        tm.initiate_model_load();

        let binding_id = binding_id.to_string();
        change_tray_icon(app, TrayIconState::Recording);
        show_ramble_recording_overlay(app);

        let rm = app.state::<Arc<AudioRecordingManager>>();

        // Get the microphone mode to determine audio feedback timing
        let settings = get_settings(app);
        let is_always_on = settings.always_on_microphone;

        let mut recording_started = false;
        if is_always_on {
            let rm_clone = Arc::clone(&rm);
            let app_clone = app.clone();
            std::thread::spawn(move || {
                play_feedback_sound_blocking(&app_clone, SoundType::Start);
                rm_clone.apply_mute();
            });
            recording_started = rm.try_start_recording(&binding_id);
        } else {
            if rm.try_start_recording(&binding_id) {
                recording_started = true;
                let app_clone = app.clone();
                let rm_clone = Arc::clone(&rm);
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    play_feedback_sound_blocking(&app_clone, SoundType::Start);
                    rm_clone.apply_mute();
                });
            }
        }

        if recording_started {
            shortcut::register_cancel_shortcut(app);
        }

        debug!(
            "RambleToCoherentAction::start completed in {:?}",
            start_time.elapsed()
        );
        recording_started
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        shortcut::unregister_cancel_shortcut(app);

        // Reset toggle state so next press starts fresh
        // Reset toggle state so next press starts fresh
        let toggle_state_manager = app.state::<ManagedToggleState>();
        if let Ok(mut states) = toggle_state_manager.lock() {
            states.active_toggles.insert(binding_id.to_string(), false);
        } else {
            warn!("Failed to lock toggle state manager in RambleToCoherentAction::stop");
        }

        let stop_time = Instant::now();
        debug!(
            "RambleToCoherentAction::stop execution start for binding: {}",
            binding_id
        );

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
        let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());

        change_tray_icon(app, TrayIconState::Transcribing);
        show_ramble_transcribing_overlay(app);

        rm.remove_mute();
        play_feedback_sound(app, SoundType::Stop);

        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            debug!("Starting async stop task. binding_id={}", binding_id);

            let samples_result = rm.stop_recording(&binding_id);
            if let Some(samples) = samples_result {
                debug!(
                    "Recording stopped successfully. samples_count={}",
                    samples.len()
                );
                let samples_clone = samples.clone();
                match tm.transcribe(samples) {
                    Ok(transcription) => {
                        debug!("Transcription completed: '{}'", transcription);
                        if !transcription.is_empty() {
                            let settings = get_settings(&ah);
                            let mut final_text = transcription.clone();
                            let mut post_processed_text: Option<String> = None;
                            let post_process_prompt: Option<String> =
                                Some(settings.ramble_prompt.clone());

                            // Show "making coherent" overlay during LLM processing
                            show_making_coherent_overlay(&ah);

                            // Apply ramble processing
                            match process_ramble_to_coherent(&ah, &settings, &transcription).await {
                                Ok(Some(processed)) => {
                                    final_text = processed.clone();
                                    post_processed_text = Some(processed);
                                }
                                Ok(None) => {
                                    // Ramble processing disabled/skipped, use original
                                }
                                Err(error_msg) => {
                                    // Show error overlay and return without pasting
                                    error!("Ramble to Coherent failed: {}", error_msg);
                                    utils::show_error_overlay(&ah, &error_msg);
                                    // Don't paste anything, let user dismiss error
                                    change_tray_icon(&ah, TrayIconState::Idle);
                                    return;
                                }
                            }

                            // Save to history
                            let hm_clone = Arc::clone(&hm);
                            let transcription_for_history = transcription.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = hm_clone
                                    .save_transcription(
                                        samples_clone,
                                        transcription_for_history,
                                        post_processed_text,
                                        post_process_prompt,
                                    )
                                    .await
                                {
                                    error!("Failed to save transcription to history: {}", e);
                                }
                            });

                            // Paste the final text
                            let ah_clone = ah.clone();
                            ah.run_on_main_thread(move || {
                                match utils::paste(final_text, ah_clone.clone()) {
                                    Ok(()) => debug!("Text pasted successfully"),
                                    Err(e) => error!("Failed to paste transcription: {}", e),
                                }
                                utils::hide_recording_overlay(&ah_clone);
                                change_tray_icon(&ah_clone, TrayIconState::Idle);
                            })
                            .unwrap_or_else(|e| {
                                error!("Failed to run paste on main thread: {:?}", e);
                                utils::hide_recording_overlay(&ah);
                                change_tray_icon(&ah, TrayIconState::Idle);
                            });
                        } else {
                            utils::hide_recording_overlay(&ah);
                            change_tray_icon(&ah, TrayIconState::Idle);
                        }
                    }
                    Err(err) => {
                        debug!("Ramble transcription error: {}", err);
                        utils::hide_recording_overlay(&ah);
                        change_tray_icon(&ah, TrayIconState::Idle);
                    }
                }
            } else {
                debug!("No samples retrieved from recording stop");
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
            }
        });

        debug!(
            "RambleToCoherentAction::stop completed in {:?}",
            stop_time.elapsed()
        );
    }
}

// Cancel Action
struct CancelAction;

impl ShortcutAction for CancelAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) -> bool {
        utils::cancel_current_operation(app);
        true
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Nothing to do on stop for cancel
    }
}

// Test Action
struct TestAction;

impl ShortcutAction for TestAction {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) -> bool {
        log::info!(
            "Shortcut ID '{}': Started - {} (App: {})",
            binding_id,
            shortcut_str,
            app.package_info().name
        );
        true
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Stopped - {} (App: {})",
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }
}

// Static Action Map
pub static ACTION_MAP: Lazy<HashMap<String, Arc<dyn ShortcutAction>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "transcribe".to_string(),
        Arc::new(TranscribeAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "ramble_to_coherent".to_string(),
        Arc::new(RambleToCoherentAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "cancel".to_string(),
        Arc::new(CancelAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    map
});
