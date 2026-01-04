use crate::managers::tts::TTSManager;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn speak_text(
    tts_manager: State<'_, Arc<TTSManager>>,
    text: String,
) -> Result<(), String> {
    tts_manager.speak(&text).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn stop_tts(tts_manager: State<'_, Arc<TTSManager>>) -> Result<(), String> {
    tts_manager.stop().await.map_err(|e| e.to_string())
}
