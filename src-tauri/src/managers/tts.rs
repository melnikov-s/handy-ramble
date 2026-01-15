use crate::managers::model::ModelManager;
use crate::overlay::{hide_recording_overlay, show_speaking_overlay};
use crate::settings::get_settings;
use crate::tts::kokoro::KokoroEngine;
use crate::tts::TTSEngine;
use anyhow::Result;
use log::{info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;

// kokorox expects a ZIP archive with NPZ voice data, not raw .bin files
const KOKORO_VOICES_URL: &str =
    "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin";
const KOKORO_VOICES_FILENAME: &str = "kokoro-voices-v1.0.bin";

pub struct TTSManager {
    app_handle: AppHandle,
    model_manager: Arc<ModelManager>,
    engine: Arc<Mutex<Option<Box<dyn TTSEngine>>>>,
}

impl TTSManager {
    pub fn new(app_handle: &AppHandle, model_manager: Arc<ModelManager>) -> Self {
        Self {
            app_handle: app_handle.clone(),
            model_manager,
            engine: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn speak(&self, text: &str) -> Result<()> {
        let settings = get_settings(&self.app_handle);
        if !settings.tts_enabled {
            return Ok(());
        }

        let model_id = settings
            .tts_selected_model
            .as_deref()
            .unwrap_or("kokoro-82m");

        // Ensure engine is loaded
        self.ensure_engine_loaded(model_id).await?;

        // Show the speaking overlay
        show_speaking_overlay(&self.app_handle);

        {
            let mut engine_guard = self.engine.lock().await;
            if let Some(engine) = engine_guard.as_mut() {
                engine
                    .speak(text, settings.tts_speed, settings.tts_volume)
                    .await?;
            }
        }

        // Spawn a task to monitor playback and hide overlay when done
        let engine_clone = self.engine.clone();
        let app_handle_clone = self.app_handle.clone();
        tokio::spawn(async move {
            // Poll until playback finishes
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                let engine_guard = engine_clone.lock().await;
                if let Some(engine) = engine_guard.as_ref() {
                    if !engine.is_playing() {
                        drop(engine_guard);
                        hide_recording_overlay(&app_handle_clone);
                        info!("TTS playback finished, hiding overlay");
                        break;
                    }
                } else {
                    // Engine not loaded, hide overlay
                    drop(engine_guard);
                    hide_recording_overlay(&app_handle_clone);
                    break;
                }
            }
        });

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let engine_guard = self.engine.lock().await;
        if let Some(engine) = engine_guard.as_ref() {
            engine.stop().await?;
        }
        // Hide overlay when stopped
        hide_recording_overlay(&self.app_handle);
        Ok(())
    }

    async fn ensure_voices_file(&self) -> Result<PathBuf> {
        let voices_path = self
            .model_manager
            .get_models_dir()
            .join(KOKORO_VOICES_FILENAME);

        if voices_path.exists() {
            return Ok(voices_path);
        }

        info!("Downloading Kokoro voices file: {}", KOKORO_VOICES_FILENAME);
        let response = reqwest::get(KOKORO_VOICES_URL).await?.error_for_status()?;
        let bytes = response.bytes().await?;
        std::fs::write(&voices_path, &bytes)?;

        Ok(voices_path)
    }

    async fn ensure_engine_loaded(&self, model_id: &str) -> Result<()> {
        let mut engine_guard = self.engine.lock().await;
        if engine_guard.is_some() {
            return Ok(());
        }

        info!("Loading TTS engine for model: {}", model_id);
        let model_info = self
            .model_manager
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("TTS Model not found: {}", model_id))?;

        if !model_info.is_downloaded {
            return Err(anyhow::anyhow!("TTS Model not downloaded: {}", model_id));
        }

        let model_path = self.model_manager.get_model_path(model_id)?;

        let voices_path = match self.ensure_voices_file().await {
            Ok(path) => path,
            Err(err) => {
                warn!("Failed to download Kokoro voices file: {}", err);
                return Err(err);
            }
        };

        let mut kokoro = KokoroEngine::new();
        kokoro.load_model(model_path, voices_path).await?;

        *engine_guard = Some(Box::new(kokoro) as Box<dyn TTSEngine>);
        info!("TTS engine loaded successfully");

        Ok(())
    }
}
