use crate::managers::model::ModelManager; // Removed unused EngineType
use crate::settings::get_settings;
use crate::tts::kokoro::KokoroEngine;
use crate::tts::TTSEngine;
use anyhow::Result;
use log::info;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;

pub struct TTSManager {
    app_handle: AppHandle,
    model_manager: Arc<ModelManager>,
    engine: Mutex<Option<Box<dyn TTSEngine>>>,
}

impl TTSManager {
    pub fn new(app_handle: &AppHandle, model_manager: Arc<ModelManager>) -> Self {
        Self {
            app_handle: app_handle.clone(),
            model_manager,
            engine: Mutex::new(None),
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

        let mut engine_guard = self.engine.lock().await;
        if let Some(engine) = engine_guard.as_mut() {
            engine
                .speak(text, settings.tts_speed, settings.tts_volume)
                .await?;
        }

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let engine_guard = self.engine.lock().await;
        if let Some(engine) = engine_guard.as_ref() {
            engine.stop().await?;
        }
        Ok(())
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

        let mut kokoro = KokoroEngine::new();
        kokoro.load_model(model_path)?;

        *engine_guard = Some(Box::new(kokoro) as Box<dyn TTSEngine>);
        info!("TTS engine loaded successfully");

        Ok(())
    }
}
