#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use crate::apple_intelligence;
use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::HistoryManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{
    get_settings, write_settings, AppSettings, DetectedApp, PromptMode,
    APPLE_INTELLIGENCE_PROVIDER_ID,
};
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{
    self, is_operation_paused, resume_current_operation, show_making_coherent_overlay,
    show_recording_overlay, show_transcribing_overlay, show_voice_command_recording_overlay,
    show_voice_command_transcribing_overlay,
};
use crate::{app_detection, known_apps};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPartImageArgs,
    ChatCompletionRequestMessageContentPartTextArgs, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, ChatCompletionRequestUserMessageContent,
    ChatCompletionRequestUserMessageContentPart, CreateChatCompletionRequestArgs,
};
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

use crate::ManagedToggleState;

/// Resolved LLM configuration for making API calls
pub struct ResolvedLLMConfig {
    pub provider: crate::settings::LLMProvider,
    pub model: crate::settings::LLMModel,
    pub api_key: String,
}

/// Resolve LLM configuration from a model ID
/// Returns the provider, model, and API key needed to make an LLM call
pub fn resolve_llm_config(
    settings: &AppSettings,
    model_id: &str,
) -> Result<ResolvedLLMConfig, String> {
    let model = settings
        .get_model(model_id)
        .cloned()
        .ok_or_else(|| format!("Model '{}' not found", model_id))?;

    let provider = settings
        .get_provider(&model.provider_id)
        .cloned()
        .ok_or_else(|| {
            format!(
                "Provider '{}' not found for model '{}'",
                model.provider_id, model_id
            )
        })?;

    if provider.api_key.is_empty() {
        return Err(format!(
            "No API key configured for provider '{}'",
            provider.name
        ));
    }

    Ok(ResolvedLLMConfig {
        api_key: provider.api_key.clone(),
        provider,
        model,
    })
}

/// interaction styles for different types of shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionBehavior {
    /// Action fires exactly once on Press. Release is ignored.
    Instant,
    /// Tap (short press) = Toggle. Hold (long press) = Push-to-Talk.
    Hybrid,
    /// Action starts on Press and stops on Release.
    Momentary,
}

// Shortcut Action Trait
pub trait ShortcutAction: Send + Sync {
    /// The style of interaction this action supports
    fn interaction_behavior(&self) -> InteractionBehavior;

    /// Start the action. Returns true if the action started successfully, false otherwise.
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) -> bool;

    /// Stop the action (for PTT or Toggle-off)
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

/// Record a detected app in the history for UI suggestions
fn record_detected_app(app: &AppHandle, bundle_id: &str, display_name: &str) {
    let mut settings = get_settings(app);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Check if app already exists in history
    if let Some(existing) = settings
        .detected_apps_history
        .iter_mut()
        .find(|a| a.bundle_identifier == bundle_id)
    {
        // Update last seen timestamp
        existing.last_seen = now;
        existing.display_name = display_name.to_string();
    } else {
        // Add new app to history
        settings.detected_apps_history.push(DetectedApp {
            bundle_identifier: bundle_id.to_string(),
            display_name: display_name.to_string(),
            last_seen: now,
        });
    }

    // Limit history size to 100 most recent apps
    if settings.detected_apps_history.len() > 100 {
        settings
            .detected_apps_history
            .sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        settings.detected_apps_history.truncate(100);
    }

    write_settings(app, settings);
    debug!("Recorded detected app: {} ({})", display_name, bundle_id);
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

    // Get the model ID to use for coherent mode
    let model_id = match settings.default_coherent_model_id.as_ref() {
        Some(id) => id,
        None => {
            let msg = "No coherent model configured";
            utils::log_to_frontend(app, "error", msg);
            debug!("{}", msg);
            return Err(msg.to_string());
        }
    };

    // Resolve the LLM config using the unified helper
    let llm_config = match resolve_llm_config(settings, model_id) {
        Ok(config) => config,
        Err(e) => {
            utils::log_to_frontend(app, "error", &e);
            debug!("{}", e);
            return Err(e);
        }
    };

    let provider = llm_config.provider.clone();
    let model = llm_config.model.model_id.clone();

    let selected_prompt_id = match &settings.coherent_selected_prompt_id {
        Some(id) => id.clone(),
        None => {
            let msg = "No coherent prompt is selected";
            debug!("{}", msg);
            return Err(msg.to_string());
        }
    };

    let prompt = match settings
        .coherent_prompts
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

    // Create OpenAI-compatible client
    let client = match crate::llm_client::create_client(&provider, llm_config.api_key.clone()) {
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
    fn interaction_behavior(&self) -> InteractionBehavior {
        InteractionBehavior::Hybrid
    }

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

        debug!(
            "TranscribeAction::start completed in {:?}, returning {}",
            start_time.elapsed(),
            recording_started
        );

        recording_started
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
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

        // CRITICAL: Stop recording synchronously to transition state to Idle immediately.
        // This prevents race conditions where user tries to start new recording before state changes.
        let stop_recording_time = Instant::now();
        let samples = rm.stop_recording(&binding_id);
        debug!(
            "Recording stopped synchronously in {:?}, samples: {}",
            stop_recording_time.elapsed(),
            samples.as_ref().map(|s| s.len()).unwrap_or(0)
        );

        tauri::async_runtime::spawn(async move {
            debug!(
                "Starting async transcription task for binding: {}",
                binding_id
            );

            if let Some(samples) = samples {
                debug!("Processing {} samples for transcription", samples.len());
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

                            // Check if coherent mode is enabled (unified hotkey: quick press)
                            let coherent_mode = rm.get_coherent_mode();
                            let selection_context = rm.get_selection_context();

                            if coherent_mode {
                                // Coherent mode: route through LLM refinement
                                debug!("Coherent mode enabled - routing through ramble processing");
                                show_making_coherent_overlay(&ah);
                                // Get prompt from coherent_prompts based on selected ID
                                if let Some(prompt_id) = &settings.coherent_selected_prompt_id {
                                    if let Some(p) = settings
                                        .coherent_prompts
                                        .iter()
                                        .find(|p| &p.id == prompt_id)
                                    {
                                        post_process_prompt = Some(p.prompt.clone());
                                    }
                                }

                                // Apply filler word filter before refinement
                                let filtered_transcription = filter_filler_words(
                                    &transcription,
                                    settings.filler_word_filter.as_deref(),
                                );

                                match process_ramble_to_coherent(
                                    &ah,
                                    &settings,
                                    &filtered_transcription,
                                    selection_context,
                                )
                                .await
                                {
                                    Ok(Some(processed)) => {
                                        final_text = processed.clone();
                                        post_processed_text = Some(processed);
                                    }
                                    Ok(None) => {
                                        // Ramble processing skipped, use original
                                    }
                                    Err(error_msg) => {
                                        // Show error overlay but fall back to raw text output
                                        error!("Coherent processing failed: {}", error_msg);
                                        utils::show_error_overlay(&ah, &error_msg);
                                        // Continue with raw text - final_text already contains the original
                                        // filtered transcription, so we just let the code continue to paste it
                                    }
                                }
                            } else {
                                // Raw mode: standard processing path
                                // Raw mode NEVER does LLM post-processing - that's the whole point
                                // Apply filler word filter to raw transcription
                                let filtered_raw = filter_filler_words(
                                    &transcription,
                                    settings.filler_word_filter.as_deref(),
                                );
                                if filtered_raw != transcription {
                                    final_text = filtered_raw.clone();
                                }

                                // Chinese variant conversion is allowed in raw mode
                                if let Some(converted_text) =
                                    maybe_convert_chinese_variant(&settings, &filtered_raw).await
                                {
                                    final_text = converted_text.clone();
                                    post_processed_text = Some(converted_text);
                                }
                                // No LLM post-processing in raw mode - just use the filtered text
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
                            // We do NOT run this on the main thread because utils::paste contains sleep calls
                            // that would block the main event loop, preventing the app's own windows (like quick chat)
                            // from receiving the simulated paste events before the clipboard is restored.
                            let paste_time = Instant::now();
                            match utils::paste(final_text, ah.clone()) {
                                Ok(()) => {
                                    debug!("Text pasted successfully in {:?}", paste_time.elapsed())
                                }
                                Err(e) => error!("Failed to paste transcription: {}", e),
                            }

                            // Perform UI updates on the main thread
                            let ah_clone = ah.clone();
                            ah.run_on_main_thread(move || {
                                // Hide the overlay after transcription is complete
                                utils::hide_recording_overlay(&ah_clone);
                                change_tray_icon(&ah_clone, TrayIconState::Idle);
                            })
                            .unwrap_or_else(|e| {
                                error!("Failed to update UI on main thread: {:?}", e);
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

/// Filter filler words from transcription using the configured regex pattern
fn filter_filler_words(text: &str, pattern: Option<&str>) -> String {
    match pattern {
        Some(p) if !p.is_empty() => {
            match Regex::new(p) {
                Ok(re) => {
                    let filtered = re.replace_all(text, "").to_string();
                    // Clean up any double spaces created by removal
                    let cleaned = filtered.split_whitespace().collect::<Vec<_>>().join(" ");
                    if cleaned != text {
                        debug!(
                            "Filtered filler words: {} chars -> {} chars",
                            text.len(),
                            cleaned.len()
                        );
                    }
                    cleaned
                }
                Err(e) => {
                    warn!("Invalid filler word filter regex: {}", e);
                    text.to_string()
                }
            }
        }
        _ => text.to_string(),
    }
}

/// Process transcription through LLM using ramble-specific settings
/// Returns Ok(Some(processed)) on success, Ok(None) if disabled/skipped, Err(msg) on error
async fn process_ramble_to_coherent(
    app: &AppHandle,
    settings: &AppSettings,
    transcription: &str,
    selection_context: Option<String>,
) -> Result<Option<String>, String> {
    // If the shortcut is pressed, we ALWAYS process regardless of ramble_enabled setting.
    // The setting is mostly for UI/default state.
    info!(
        "Starting Ramble to Coherent processing ({} chars)",
        transcription.len()
    );
    utils::log_to_frontend(app, "info", "Starting refinement...");

    // === Determine prompt FIRST so we can check if OCR is needed ===
    // Determine which category to use based on prompt mode and frontmost app
    let (category_id, app_name) = match settings.prompt_mode {
        PromptMode::Dynamic => {
            // Detect frontmost app
            let app_info = app_detection::get_frontmost_application();
            let (bundle_id, name) = app_info
                .map(|info| (info.bundle_identifier, info.display_name))
                .unwrap_or_else(|| ("".to_string(), "Unknown".to_string()));

            // Record this app in detected_apps_history for UI suggestions
            if !bundle_id.is_empty() {
                record_detected_app(app, &bundle_id, &name);
            }

            // Look up category: user mappings first, then known_apps, then default category
            let cat_id = settings
                .app_category_mappings
                .iter()
                .find(|m| m.bundle_identifier == bundle_id)
                .map(|m| m.category_id.clone())
                .or_else(|| {
                    known_apps::find_known_app(&bundle_id).map(|k| k.suggested_category.clone())
                })
                .unwrap_or_else(|| settings.default_category_id.clone());

            debug!(
                "Dynamic mode: detected app '{}' ({}), using category '{}'",
                name, bundle_id, cat_id
            );
            (cat_id, name)
        }
        PromptMode::Development => ("development".to_string(), "Unknown".to_string()),
        PromptMode::Conversation => ("conversation".to_string(), "Unknown".to_string()),
        PromptMode::Writing => ("writing".to_string(), "Unknown".to_string()),
        PromptMode::Email => ("email".to_string(), "Unknown".to_string()),
    };

    // Find the prompt for this category, falling back to default category's prompt
    let prompt = settings
        .prompt_categories
        .iter()
        .find(|c| c.id == category_id)
        .or_else(|| {
            debug!(
                "Category '{}' not found, falling back to default category '{}'",
                category_id, settings.default_category_id
            );
            settings
                .prompt_categories
                .iter()
                .find(|c| c.id == settings.default_category_id)
        })
        .map(|c| c.prompt.clone())
        .unwrap_or_default();

    if prompt.trim().is_empty() {
        let msg = "Prompt is empty".to_string();
        utils::log_to_frontend(app, "error", &msg);
        return Err(msg);
    }

    // Get the model ID to use - check for vision model if screenshots are present
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    let vision_context = audio_manager.get_vision_context();
    let has_screenshots = !vision_context.is_empty();

    // Use vision-compatible model if screenshots present and vision is enabled
    let model_id = if has_screenshots && settings.coherent_use_vision {
        // Use the same default model but ensure it supports vision
        settings
            .default_coherent_model_id
            .as_ref()
            .ok_or_else(|| "No coherent model configured".to_string())?
    } else {
        settings
            .default_coherent_model_id
            .as_ref()
            .ok_or_else(|| "No coherent model configured".to_string())?
    };

    // Resolve the LLM config using the unified helper
    let llm_config = resolve_llm_config(settings, model_id)?;
    let provider = llm_config.provider.clone();
    let model = llm_config.model.model_id.clone();

    // Log the model being used to the frontend
    utils::log_to_frontend(app, "info", &format!("Using model: {}", model));

    info!(
        "Starting Ramble to Coherent with provider '{}' (model: {}), category: '{}', app: '{}'",
        provider.name, model, category_id, app_name
    );
    utils::log_to_frontend(app, "info", &format!("Using {} mode", category_id));

    // Emit event to update overlay icon with the detected category
    let _ = app.emit("category-detected", &category_id);

    // Replace variables in the prompt
    // ${application} - The detected app name
    // ${category} - The category name
    // ${selection} - Selected text captured before recording
    // ${output} - The transcribed speech
    // ${screen_context} - (REMOVED) - was OCR text from screen capture

    let processed_prompt = if let Some(selection) = selection_context {
        if prompt.contains("${selection}") {
            // User has explicitly included ${selection} in their prompt
            prompt
                .replace("${application}", &app_name)
                .replace("${category}", &category_id)
                .replace("${output}", transcription)
                .replace("${selection}", &selection)
                .replace("${screen_context}", "")
        } else {
            // User hasn't included ${selection}, so we ignore it to respect "not combined" requested by user unless explicit.
            warn!("Selection context available but ${{selection}} variable missing in prompt. Ignoring selection.");
            prompt
                .replace("${application}", &app_name)
                .replace("${category}", &category_id)
                .replace("${output}", transcription)
                .replace("${screen_context}", "")
        }
    } else {
        // No selection context, just clear the variable if it exists
        prompt
            .replace("${application}", &app_name)
            .replace("${category}", &category_id)
            .replace("${output}", transcription)
            .replace("${selection}", "")
            .replace("${screen_context}", "")
    };

    debug!(
        "Processed prompt ({} chars):\n{}",
        processed_prompt.len(),
        processed_prompt
    );

    // Create OpenAI-compatible client using the resolved config
    let client = match crate::llm_client::create_client(&provider, llm_config.api_key) {
        Ok(client) => client,
        Err(e) => {
            return Err(format!("Failed to create client: {}", e));
        }
    };

    // Build the chat completion request
    // If vision is supported and a screenshot is available, use array content (vision)
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    let vision_context = audio_manager.get_vision_context();

    let message = if provider.supports_vision {
        if !vision_context.is_empty() {
            info!(
                "Vision enabled: Attaching {} screenshots to request",
                vision_context.len()
            );
            utils::log_to_frontend(app, "info", "Analyzing screenshots...");

            let text_part = ChatCompletionRequestMessageContentPartTextArgs::default()
                .text(processed_prompt)
                .build()
                .map_err(|e| format!("Request error (text part): {}", e))?;

            let mut parts = vec![ChatCompletionRequestUserMessageContentPart::Text(text_part)];

            for (i, base64_image) in vision_context.iter().enumerate() {
                debug!(
                    "Attaching screenshot {} ({} chars)",
                    i + 1,
                    base64_image.len()
                );
                let image_part = ChatCompletionRequestMessageContentPartImageArgs::default()
                    .image_url(format!("data:image/png;base64,{}", base64_image))
                    .build()
                    .map_err(|e| format!("Request error (image part {}): {}", i, e))?;
                parts.push(ChatCompletionRequestUserMessageContentPart::ImageUrl(
                    image_part,
                ));
            }

            let content = ChatCompletionRequestUserMessageContent::Array(parts);

            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(content)
                    .build()
                    .map_err(|e| format!("Request error (user message): {}", e))?,
            )
        } else {
            warn!("Provider supports vision but no screenshot context found.");
            // Proceed with text only
            match ChatCompletionRequestUserMessageArgs::default()
                .content(processed_prompt)
                .build()
            {
                Ok(msg) => ChatCompletionRequestMessage::User(msg),
                Err(e) => {
                    return Err(format!("Request error: {}", e));
                }
            }
        }
    } else {
        if !vision_context.is_empty() {
            warn!(
                "Screenshots captured but provider '{}' does NOT support vision. Ignoring {} images.",
                provider.id,
                vision_context.len()
            );
            utils::log_to_frontend(
                app,
                "warning",
                "Provider doesn't support images - ignoring screenshots",
            );
        }
        match ChatCompletionRequestUserMessageArgs::default()
            .content(processed_prompt)
            .build()
        {
            Ok(msg) => ChatCompletionRequestMessage::User(msg),
            Err(e) => {
                return Err(format!("Request error: {}", e));
            }
        }
    };

    // Create the system message to enforce proxy persona
    let system_message = ChatCompletionRequestSystemMessageArgs::default()
        .content("You are an AI assistant acting as the user's proxy. You must speak **as** the user, in the first person. Do not address the user directly. Do not explain your response. Your output will be sent to another agent or system as if the user wrote it.")
        .build()
        .map_err(|e| format!("Request error (system message): {}", e))?;

    let request = match CreateChatCompletionRequestArgs::default()
        .model(&model)
        .messages(vec![
            ChatCompletionRequestMessage::System(system_message),
            message,
        ])
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
        Err(e) => Err(extract_llm_error(&e, &model)),
    }
}

// Cancel Action
struct CancelAction;

impl ShortcutAction for CancelAction {
    fn interaction_behavior(&self) -> InteractionBehavior {
        InteractionBehavior::Instant
    }

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
    fn interaction_behavior(&self) -> InteractionBehavior {
        InteractionBehavior::Instant
    }

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

// Pause Action
struct PauseAction;

impl ShortcutAction for PauseAction {
    fn interaction_behavior(&self) -> InteractionBehavior {
        InteractionBehavior::Instant
    }

    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) -> bool {
        crate::utils::toggle_pause_operation(app);
        true
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

// Quick Chat Action - Opens a new chat window immediately
struct QuickChatAction;

impl ShortcutAction for QuickChatAction {
    fn interaction_behavior(&self) -> InteractionBehavior {
        InteractionBehavior::Instant
    }

    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) -> bool {
        debug!("[ACTION] QuickChatAction::start - opening chat window");

        // Get current selection context (if user has text selected)
        let selection = match crate::clipboard::get_selected_text(app) {
            Ok(text) => text,
            Err(e) => {
                debug!("Failed to get selected text: {}", e);
                None
            }
        };

        // Open a new chat window
        match crate::commands::open_chat_window(app.clone(), selection) {
            Ok(label) => {
                info!("Opened quick chat window: {}", label);
                true
            }
            Err(e) => {
                error!("Failed to open quick chat window: {}", e);
                false
            }
        }
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Quick chat is instant - no stop action needed
    }
}

// Voice Command Action
struct VoiceCommandAction;

impl ShortcutAction for VoiceCommandAction {
    fn interaction_behavior(&self) -> InteractionBehavior {
        InteractionBehavior::Hybrid
    }

    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) -> bool {
        debug!(
            "[ACTION] VoiceCommandAction::start called for binding: {}",
            binding_id
        );

        // Check if we're resuming from a paused state
        if is_operation_paused(app, binding_id) {
            debug!("Resuming paused voice command for binding: {}", binding_id);
            resume_current_operation(app);
            return true;
        }

        // Load model in the background (for transcription)
        let tm = app.state::<Arc<TranscriptionManager>>();
        tm.initiate_model_load();

        let binding_id = binding_id.to_string();
        change_tray_icon(app, TrayIconState::Recording);

        // Show voice command recording overlay (purple theme)
        show_voice_command_recording_overlay(app);

        let rm = app.state::<Arc<AudioRecordingManager>>();
        let _settings = get_settings(app);

        // Play audio feedback
        let rm_clone = Arc::clone(&rm);
        let app_clone = app.clone();
        std::thread::spawn(move || {
            play_feedback_sound_blocking(&app_clone, SoundType::Start);
            rm_clone.apply_mute();
        });

        rm.try_start_recording(&binding_id)
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        // Reset toggle state
        let toggle_state_manager = app.state::<ManagedToggleState>();
        if let Ok(mut states) = toggle_state_manager.lock() {
            states.active_toggles.insert(binding_id.to_string(), false);
        }

        debug!(
            "VoiceCommandAction::stop called for binding: {}",
            binding_id
        );

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());

        change_tray_icon(app, TrayIconState::Transcribing);
        show_voice_command_transcribing_overlay(app);

        rm.remove_mute();
        play_feedback_sound(app, SoundType::Stop);

        let binding_id = binding_id.to_string();
        let samples = rm.stop_recording(&binding_id);

        tauri::async_runtime::spawn(async move {
            if let Some(samples) = samples {
                match tm.transcribe(samples) {
                    Ok(transcription) => {
                        if !transcription.is_empty() {
                            debug!("Voice command transcription: '{}'", transcription);

                            // Emit processing state to update overlay (must emit to overlay window, not globally)
                            if let Some(overlay) = ah.get_webview_window("recording_overlay") {
                                let _ = overlay.emit("show-overlay", "processing_command");
                            }

                            // Process voice command
                            match process_voice_command(&ah, &transcription).await {
                                Ok(result) => {
                                    debug!("Voice command result: {:?}", result);
                                    match result {
                                        crate::voice_commands::CommandResult::PasteOutput(text) => {
                                            let ah_clone = ah.clone();
                                            ah.run_on_main_thread(move || {
                                                match utils::paste(text, ah_clone.clone()) {
                                                    Ok(()) => debug!("Command output pasted"),
                                                    Err(e) => error!("Failed to paste: {}", e),
                                                }
                                                utils::hide_recording_overlay(&ah_clone);
                                                change_tray_icon(&ah_clone, TrayIconState::Idle);
                                            })
                                            .unwrap_or_else(|e| {
                                                error!("Failed to run on main thread: {:?}", e);
                                            });
                                        }
                                        crate::voice_commands::CommandResult::Success => {
                                            // Show brief feedback
                                            utils::hide_recording_overlay(&ah);
                                            change_tray_icon(&ah, TrayIconState::Idle);
                                        }
                                        crate::voice_commands::CommandResult::Error(msg) => {
                                            utils::show_error_overlay(&ah, &msg);
                                            change_tray_icon(&ah, TrayIconState::Idle);
                                        }
                                        crate::voice_commands::CommandResult::InternalCommand(
                                            cmd,
                                        ) => {
                                            if cmd == "open_chat_window" {
                                                // Open chat window with selection context
                                                let selection_context = rm.get_selection_context();
                                                match crate::commands::open_chat_window(
                                                    ah.clone(),
                                                    selection_context,
                                                ) {
                                                    Ok(label) => {
                                                        debug!("Opened chat window: {}", label)
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to open chat window: {}", e)
                                                    }
                                                }
                                            } else {
                                                warn!("Unknown internal command: {}", cmd);
                                            }
                                            utils::hide_recording_overlay(&ah);
                                            change_tray_icon(&ah, TrayIconState::Idle);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Voice command processing failed: {}", e);
                                    utils::show_error_overlay(&ah, &e);
                                    change_tray_icon(&ah, TrayIconState::Idle);
                                }
                            }
                        } else {
                            utils::hide_recording_overlay(&ah);
                            change_tray_icon(&ah, TrayIconState::Idle);
                        }
                    }
                    Err(err) => {
                        error!("Voice command transcription error: {}", err);
                        utils::hide_recording_overlay(&ah);
                        change_tray_icon(&ah, TrayIconState::Idle);
                    }
                }
            } else {
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
            }
        });
    }
}

/// Process a voice command through LLM interpretation and execution
async fn process_voice_command(
    app: &AppHandle,
    transcription: &str,
) -> Result<crate::voice_commands::CommandResult, String> {
    let settings = get_settings(app);

    if !settings.voice_commands_enabled {
        return Err("Voice commands are not enabled".to_string());
    }

    let commands = &settings.voice_commands;
    if commands.is_empty() {
        return Err("No voice commands configured".to_string());
    }

    // Get selection context if available
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    let selection_context = audio_manager.get_selection_context();

    // Let LLM interpret the command and determine what to execute
    execute_via_llm(app, &settings, transcription, selection_context).await
}

fn execute_shell_command(cmd: &str) -> crate::voice_commands::CommandResult {
    use std::process::Command;

    match Command::new("sh").arg("-c").arg(cmd).output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if stdout.is_empty() {
                    crate::voice_commands::CommandResult::Success
                } else {
                    crate::voice_commands::CommandResult::PasteOutput(stdout)
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                crate::voice_commands::CommandResult::Error(format!("Command failed: {}", stderr))
            }
        }
        Err(e) => crate::voice_commands::CommandResult::Error(format!("Failed to run: {}", e)),
    }
}

#[cfg(target_os = "macos")]
fn execute_applescript_command(script: &str) -> crate::voice_commands::CommandResult {
    use std::process::Command;

    match Command::new("osascript").arg("-e").arg(script).output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if stdout.is_empty() {
                    crate::voice_commands::CommandResult::Success
                } else {
                    crate::voice_commands::CommandResult::PasteOutput(stdout)
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                crate::voice_commands::CommandResult::Error(format!(
                    "AppleScript failed: {}",
                    stderr
                ))
            }
        }
        Err(e) => crate::voice_commands::CommandResult::Error(format!("Failed to run: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
fn execute_applescript_command(_script: &str) -> crate::voice_commands::CommandResult {
    crate::voice_commands::CommandResult::Error(
        "AppleScript is only supported on macOS".to_string(),
    )
}

/// Use LLM to interpret and execute an unknown command
async fn execute_via_llm(
    app: &AppHandle,
    settings: &AppSettings,
    transcription: &str,
    selection: Option<String>,
) -> Result<crate::voice_commands::CommandResult, String> {
    let transcription_lower = transcription.to_lowercase();

    // Pre-check: For custom commands, try direct phrase matching first
    // This avoids LLM misinterpreting commands like "open chat" as "open app"
    for cmd in &settings.voice_commands {
        if cmd.command_type == crate::settings::VoiceCommandType::Custom {
            for phrase in &cmd.phrases {
                if transcription_lower.contains(&phrase.to_lowercase()) {
                    debug!(
                        "Direct phrase match for custom command '{}' (phrase: '{}')",
                        cmd.name, phrase
                    );
                    return Ok(crate::voice_commands::execute_bespoke_command(
                        cmd,
                        selection.as_deref(),
                    ));
                }
            }
        }
    }

    let model = match settings.default_voice_model_id.as_ref() {
        Some(id) if !id.trim().is_empty() => id,
        _ => {
            return Err("No default model configured for voice commands".to_string());
        }
    };

    // Resolve the LLM config using the voice command default model
    let llm_config = resolve_llm_config(settings, model)?;
    let provider = llm_config.provider.clone();
    let api_key = llm_config.api_key.clone();
    let api_model = llm_config.model.model_id.clone(); // The actual API model ID (e.g., "gemini-2.5-flash-lite")

    let client = crate::llm_client::create_client(&provider, api_key.clone())
        .map_err(|e| format!("Failed to create LLM client: {}", e))?;

    // Build prompt with available commands
    let prompt =
        crate::voice_commands::build_command_prompt(&settings.voice_commands, selection.as_deref());

    let user_message = ChatCompletionRequestUserMessageArgs::default()
        .content(format!("User command: \"{}\"", transcription))
        .build()
        .map_err(|e| format!("Failed to build message: {}", e))?;

    let system_message = ChatCompletionRequestSystemMessageArgs::default()
        .content(prompt)
        .build()
        .map_err(|e| format!("Failed to build system message: {}", e))?;

    let request = CreateChatCompletionRequestArgs::default()
        .model(&api_model)
        .messages(vec![
            ChatCompletionRequestMessage::System(system_message),
            ChatCompletionRequestMessage::User(user_message),
        ])
        .build()
        .map_err(|e| format!("Failed to build request: {}", e))?;

    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|e| extract_llm_error(&e, &api_model))?;

    let llm_response = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .ok_or_else(|| "LLM returned empty response".to_string())?;

    debug!("Voice command LLM response: {}", llm_response);

    // Strip markdown code blocks if present (LLM sometimes wraps JSON in ```json ... ```)
    let json_str = llm_response
        .trim()
        .strip_prefix("```json")
        .or_else(|| llm_response.trim().strip_prefix("```"))
        .unwrap_or(llm_response)
        .trim()
        .strip_suffix("```")
        .unwrap_or(llm_response)
        .trim();

    // Parse the JSON response
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(json) => {
            let exec_type = json
                .get("execution_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if let Some(matched_id) = json.get("matched_command").and_then(|v| v.as_str()) {
                // LLM matched a command, execute it
                let command = json.get("command").and_then(|v| v.as_str()).unwrap_or("");

                // Check for paste execution type first (used by print/echo commands)
                if exec_type == "paste" {
                    let output = json
                        .get("output")
                        .and_then(|v| v.as_str())
                        .unwrap_or(command);
                    debug!("Paste output: {}", output);
                    return Ok(crate::voice_commands::CommandResult::PasteOutput(
                        output.to_string(),
                    ));
                }

                // Look up the matched command to determine how to execute it
                if let Some(cmd) = settings.voice_commands.iter().find(|c| c.id == matched_id) {
                    match cmd.command_type {
                        crate::settings::VoiceCommandType::Custom => {
                            // Execute user-defined script
                            debug!("Executing custom command by ID: {}", matched_id);
                            return Ok(crate::voice_commands::execute_bespoke_command(
                                cmd,
                                selection.as_deref(),
                            ));
                        }
                        crate::settings::VoiceCommandType::Builtin
                        | crate::settings::VoiceCommandType::LegacyInferable => {
                            // Execute built-in command with native handler
                            debug!("Executing built-in command: {}", matched_id);
                            return execute_builtin_command(
                                matched_id,
                                transcription,
                                selection.as_deref(),
                            );
                        }
                    }
                }

                // If no command found by ID but we have a command string, execute it as shell
                if !command.is_empty() {
                    debug!(
                        "Executing voice command: type={}, command={}",
                        exec_type, command
                    );

                    return match exec_type {
                        "applescript" => Ok(execute_applescript_command(command)),
                        _ => Ok(execute_shell_command(command)),
                    };
                }

                // No executable command found
                Ok(crate::voice_commands::CommandResult::Error(format!(
                    "Command '{}' not found",
                    matched_id
                )))
            } else {
                // No match, return the explanation or paste output
                if exec_type == "paste" {
                    let output = json
                        .get("output")
                        .and_then(|v| v.as_str())
                        .unwrap_or("No output");
                    Ok(crate::voice_commands::CommandResult::PasteOutput(
                        output.to_string(),
                    ))
                } else {
                    let message = json
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Command not understood");
                    Ok(crate::voice_commands::CommandResult::PasteOutput(
                        message.to_string(),
                    ))
                }
            }
        }
        Err(_) => {
            // LLM didn't return valid JSON, treat response as the output
            Ok(crate::voice_commands::CommandResult::PasteOutput(
                llm_response.clone(),
            ))
        }
    }
}

/// Execute a built-in command with native handler
fn execute_builtin_command(
    command_id: &str,
    transcription: &str,
    selection: Option<&str>,
) -> Result<crate::voice_commands::CommandResult, String> {
    match command_id {
        "web_search" => {
            // Extract search query from transcription
            let query = extract_search_query(transcription);
            if query.is_empty() {
                return Ok(crate::voice_commands::CommandResult::Error(
                    "No search query provided".to_string(),
                ));
            }
            // URL encode the query and open in browser
            let encoded_query = urlencoding::encode(&query);
            let url = format!("https://google.com/search?q={}", encoded_query);
            Ok(execute_shell_command(&format!("open \"{}\"", url)))
        }
        "open_app" => {
            // Extract app name from transcription
            let app_name = extract_app_name(transcription);
            if app_name.is_empty() {
                return Ok(crate::voice_commands::CommandResult::Error(
                    "No application name provided".to_string(),
                ));
            }
            Ok(execute_shell_command(&format!("open -a \"{}\"", app_name)))
        }
        "print" => {
            // Extract text to print (everything after trigger words)
            let text = extract_print_text(transcription);
            Ok(crate::voice_commands::CommandResult::PasteOutput(text))
        }
        "refactor_code" => {
            // For refactor, we need to process selection through LLM
            // For now, just return the selection with a note
            if let Some(sel) = selection {
                Ok(crate::voice_commands::CommandResult::PasteOutput(format!(
                    "// TODO: Refactor the following code:\n{}",
                    sel
                )))
            } else {
                Ok(crate::voice_commands::CommandResult::Error(
                    "No code selected for refactoring".to_string(),
                ))
            }
        }
        _ => {
            // Unknown built-in command, treat as error
            Ok(crate::voice_commands::CommandResult::Error(format!(
                "Unknown built-in command: {}",
                command_id
            )))
        }
    }
}

/// Extract search query from transcription like "search for weather in nyc"
fn extract_search_query(transcription: &str) -> String {
    let lower = transcription.to_lowercase();
    // Common trigger phrases for web search
    let triggers = ["search for ", "look up ", "google ", "search "];
    for trigger in triggers {
        if let Some(pos) = lower.find(trigger) {
            return transcription[pos + trigger.len()..].trim().to_string();
        }
    }
    // If no trigger found, use the whole transcription
    transcription.trim().to_string()
}

/// Extract app name from transcription like "open chrome" or "launch safari"
fn extract_app_name(transcription: &str) -> String {
    let lower = transcription.to_lowercase();
    let triggers = ["open ", "launch ", "start "];
    for trigger in triggers {
        if let Some(pos) = lower.find(trigger) {
            return transcription[pos + trigger.len()..].trim().to_string();
        }
    }
    transcription.trim().to_string()
}

/// Extract text to print from transcription like "print hello world" -> "hello world"
fn extract_print_text(transcription: &str) -> String {
    let lower = transcription.to_lowercase();
    let triggers = ["print ", "echo ", "say ", "type "];
    for trigger in triggers {
        if let Some(pos) = lower.find(trigger) {
            return transcription[pos + trigger.len()..].trim().to_string();
        }
    }
    transcription.trim().to_string()
}

// Static Action Map
pub static ACTION_MAP: Lazy<HashMap<String, Arc<dyn ShortcutAction>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "transcribe".to_string(),
        Arc::new(TranscribeAction) as Arc<dyn ShortcutAction>,
    );
    // Note: ramble_to_coherent is no longer a separate action.
    // Unified hotkey: hold transcribe key = raw, quick tap = coherent.
    map.insert(
        "cancel".to_string(),
        Arc::new(CancelAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "pause_toggle".to_string(),
        Arc::new(PauseAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "voice_command".to_string(),
        Arc::new(VoiceCommandAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "quick_chat".to_string(),
        Arc::new(QuickChatAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    map
});
