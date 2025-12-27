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
    show_recording_overlay, show_transcribing_overlay,
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

/// Captures screen OCR in background if the current prompt uses ${screen_context}.
/// This runs at recording start so OCR is ready by the time transcription finishes.
fn maybe_capture_screen_ocr_background(app: &AppHandle) {
    let settings = get_settings(app);

    // Determine which category/prompt will be used based on current app
    let category_id = match settings.prompt_mode {
        PromptMode::Dynamic => {
            // Detect frontmost app
            let app_info = app_detection::get_frontmost_application();
            let bundle_id = app_info
                .map(|info| info.bundle_identifier)
                .unwrap_or_default();

            // Look up category: user mappings first, then known_apps, then default
            settings
                .app_category_mappings
                .iter()
                .find(|m| m.bundle_identifier == bundle_id)
                .map(|m| m.category_id.clone())
                .or_else(|| {
                    known_apps::find_known_app(&bundle_id).map(|k| k.suggested_category.clone())
                })
                .unwrap_or_else(|| settings.default_category_id.clone())
        }
        PromptMode::Development => "development".to_string(),
        PromptMode::Conversation => "conversation".to_string(),
        PromptMode::Writing => "writing".to_string(),
        PromptMode::Email => "email".to_string(),
    };

    // Get the prompt for this category
    let prompt = settings
        .prompt_categories
        .iter()
        .find(|c| c.id == category_id)
        .or_else(|| {
            settings
                .prompt_categories
                .iter()
                .find(|c| c.id == settings.default_category_id)
        })
        .map(|c| c.prompt.clone())
        .unwrap_or_default();

    // Check if prompt uses ${screen_context}
    if !prompt.contains("${screen_context}") {
        debug!("Prompt does not use ${{screen_context}}, skipping background OCR");
        return;
    }

    debug!("Prompt uses ${{screen_context}}, starting background OCR capture...");

    // Capture and OCR the screen
    let ocr_text = crate::vision::capture_and_ocr_screen();

    if !ocr_text.is_empty() {
        debug!("Background OCR captured {} chars", ocr_text.len());

        // Store in audio manager for later use
        let audio_manager = app.state::<Arc<AudioRecordingManager>>();
        audio_manager.set_screen_ocr_context(ocr_text);
    } else {
        debug!("Background OCR returned empty result");
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

        debug!(
            "TranscribeAction::start completed in {:?}, returning {}",
            start_time.elapsed(),
            recording_started
        );

        // If recording started, check if we should capture screen OCR in background
        if recording_started {
            let app_clone = app.clone();
            std::thread::spawn(move || {
                maybe_capture_screen_ocr_background(&app_clone);
            });
        }

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
                                post_process_prompt = Some(settings.ramble_prompt.clone());

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
                                        // Show error overlay and return without pasting
                                        error!("Coherent processing failed: {}", error_msg);
                                        utils::show_error_overlay(&ah, &error_msg);
                                        change_tray_icon(&ah, TrayIconState::Idle);
                                        return;
                                    }
                                }
                            } else {
                                // Raw mode: standard processing path
                                // Apply filler word filter to raw transcription
                                let filtered_raw = filter_filler_words(
                                    &transcription,
                                    settings.filler_word_filter.as_deref(),
                                );
                                if filtered_raw != transcription {
                                    final_text = filtered_raw.clone();
                                }

                                // First, check if Chinese variant conversion is needed
                                if let Some(converted_text) =
                                    maybe_convert_chinese_variant(&settings, &filtered_raw).await
                                {
                                    final_text = converted_text.clone();
                                    post_processed_text = Some(converted_text);
                                }
                                // Then apply regular post-processing if enabled
                                else if settings.post_process_enabled {
                                    match maybe_post_process_transcription(
                                        &ah,
                                        &settings,
                                        &filtered_raw,
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
                                                    post_process_prompt =
                                                        Some(prompt.prompt.clone());
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

    // === OCR: Use pre-captured screen context from recording start ===
    let screen_context = if prompt.contains("${screen_context}") {
        // Get the cached OCR result that was captured in background when recording started
        let audio_manager = app.state::<Arc<AudioRecordingManager>>();
        match audio_manager.get_screen_ocr_context() {
            Some(ocr_text) => {
                debug!("Using cached OCR result ({} chars)", ocr_text.len());
                Some(ocr_text)
            }
            None => {
                // Fallback: capture synchronously if background capture didn't complete
                debug!("No cached OCR, falling back to synchronous capture...");
                utils::log_to_frontend(app, "info", "Analyzing screen...");
                let ocr_text = crate::vision::capture_and_ocr_screen();
                if !ocr_text.is_empty() {
                    debug!("Fallback OCR captured {} chars", ocr_text.len());
                }
                Some(ocr_text)
            }
        }
    } else {
        debug!("Prompt does not contain ${{screen_context}}, skipping OCR");
        None
    };

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

    // Check for screenshots context to determine model
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    let vision_context = audio_manager.get_vision_context();
    let has_screenshots = !vision_context.is_empty();

    // Determine which model to use
    let model = if settings.ramble_use_vision_model && has_screenshots {
        if !settings.ramble_vision_model.trim().is_empty() {
            debug!(
                "Using specialized vision model for screenshots: {}",
                settings.ramble_vision_model
            );
            &settings.ramble_vision_model
        } else {
            warn!("Vision model enabled but empty, falling back to standard model");
            &settings.ramble_model
        }
    } else {
        &settings.ramble_model
    };

    if model.trim().is_empty() {
        let msg = "No model configured".to_string();
        utils::log_to_frontend(app, "error", &msg);
        return Err(msg);
    }

    // Log the model being used to the frontend
    utils::log_to_frontend(app, "info", &format!("Using model: {}", model));

    info!(
        "Starting Ramble to Coherent with provider '{}' (model: {}), category: '{}', app: '{}'",
        provider.id, model, category_id, app_name
    );
    utils::log_to_frontend(app, "info", &format!("Using {} mode", category_id));

    // Emit event to update overlay icon with the detected category
    let _ = app.emit("category-detected", &category_id);

    // Replace variables in the prompt
    // ${application} - The detected app name
    // ${category} - The category name
    // ${selection} - Selected text captured before recording
    // ${output} - The transcribed speech
    // ${screen_context} - OCR text from screen capture
    let screen_text = screen_context.as_deref().unwrap_or("");

    let processed_prompt = if let Some(selection) = selection_context {
        if prompt.contains("${selection}") {
            // User has explicitly included ${selection} in their prompt
            prompt
                .replace("${application}", &app_name)
                .replace("${category}", &category_id)
                .replace("${output}", transcription)
                .replace("${selection}", &selection)
                .replace("${screen_context}", screen_text)
        } else {
            // User hasn't included ${selection}, so we ignore it to respect "not combined" requested by user unless explicit.
            warn!("Selection context available but ${{selection}} variable missing in prompt. Ignoring selection.");
            prompt
                .replace("${application}", &app_name)
                .replace("${category}", &category_id)
                .replace("${output}", transcription)
                .replace("${screen_context}", screen_text)
        }
    } else {
        // No selection context, just clear the variable if it exists
        prompt
            .replace("${application}", &app_name)
            .replace("${category}", &category_id)
            .replace("${output}", transcription)
            .replace("${selection}", "")
            .replace("${screen_context}", screen_text)
    };

    debug!(
        "Processed prompt ({} chars):\n{}",
        processed_prompt.len(),
        processed_prompt
    );

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
        .model(model)
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
        Err(e) => Err(extract_llm_error(&e, model)),
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

// Pause Action
struct PauseAction;

impl ShortcutAction for PauseAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) -> bool {
        crate::utils::toggle_pause_operation(app);
        true
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

// Voice Command Action
struct VoiceCommandAction;

impl ShortcutAction for VoiceCommandAction {
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

        // Show a different overlay state for command mode
        let _ = app.emit("voice-command-mode", true);
        show_recording_overlay(app);

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
        show_transcribing_overlay(app);

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

                            // Emit processing state to update overlay
                            let _ = ah.emit("processing-command", transcription.clone());

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
    let model = &settings.voice_command_default_model;
    if model.trim().is_empty() {
        return Err("No default model configured for voice commands".to_string());
    }

    let provider_id = &settings.ramble_provider_id;
    let provider = settings
        .post_process_providers
        .iter()
        .find(|p| &p.id == provider_id)
        .cloned()
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    let api_key = settings
        .post_process_api_keys
        .get(provider_id)
        .cloned()
        .unwrap_or_default();

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
        .model(model)
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
        .map_err(|e| extract_llm_error(&e, model))?;

    let llm_response = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .ok_or_else(|| "LLM returned empty response".to_string())?;

    debug!("Voice command LLM response: {}", llm_response);

    // Parse the JSON response
    match serde_json::from_str::<serde_json::Value>(llm_response) {
        Ok(json) => {
            let exec_type = json
                .get("execution_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Check for computer_use FIRST - this can be triggered even without a matched command
            // Also treat invalid types that suggest web interaction as computer_use
            let is_computer_use = exec_type == "computer_use"
                || exec_type == "web_search"
                || exec_type == "browse"
                || exec_type == "web"
                || exec_type == "search";

            if is_computer_use {
                let task_description = json
                    .get("task_description")
                    .and_then(|v| v.as_str())
                    // Fallback to command field if task_description missing
                    .or_else(|| json.get("command").and_then(|v| v.as_str()))
                    .unwrap_or(transcription);

                return execute_computer_use_command(app, task_description, &api_key).await;
            }

            if json
                .get("matched_command")
                .and_then(|v| v.as_str())
                .is_some()
            {
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

                if command.is_empty() {
                    // Check if it's a bespoke command
                    let cmd_id = json
                        .get("matched_command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if let Some(bespoke) = settings.voice_commands.iter().find(|c| c.id == cmd_id) {
                        return Ok(crate::voice_commands::execute_bespoke_command(bespoke));
                    }
                }

                debug!(
                    "Executing voice command: type={}, command={}",
                    exec_type, command
                );

                match exec_type {
                    "applescript" => Ok(execute_applescript_command(command)),
                    _ => Ok(execute_shell_command(command)),
                }
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

/// Execute a computer use command via the Gemini agent
async fn execute_computer_use_command(
    app: &AppHandle,
    task: &str,
    api_key: &str,
) -> Result<crate::voice_commands::CommandResult, String> {
    use crate::computer_use::ComputerUseAgent;

    info!("Starting Computer Use agent for task: {}", task);

    // Get the stop signal from the audio manager
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    audio_manager.clear_computer_use_stop();
    let stop_signal = audio_manager.get_computer_use_stop_signal();

    // Create and run the agent
    let agent = ComputerUseAgent::new(app.clone(), stop_signal)
        .map_err(|e| format!("Failed to create agent: {}", e))?;

    let result = agent.run(task, api_key).await;

    if result.success {
        // Don't paste the output - the toast notification is shown via emit_end event
        Ok(crate::voice_commands::CommandResult::Success)
    } else {
        let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
        Ok(crate::voice_commands::CommandResult::Error(error_msg))
    }
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
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    map
});
