use crate::tts::TTSEngine;
use anyhow::Result;
use kokorox::tts::koko::TTSKoko;
use log::info;
use rodio::{OutputStreamBuilder, Sink};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct KokoroEngine {
    tts: Arc<RwLock<Option<TTSKoko>>>,
    _stream_handle: Option<SendWrapper<rodio::OutputStream>>,
    sink: Option<Sink>,
}

struct SendWrapper<T>(T);
unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {}

/// Split text into sentences for streaming playback
fn split_into_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        // Split on sentence-ending punctuation
        if ch == '.' || ch == '!' || ch == '?' || ch == ';' || ch == '\n' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() && trimmed.len() > 1 {
                sentences.push(trimmed);
            }
            current = String::new();
        }
    }

    // Don't forget remaining text
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }

    // If no sentences found (no punctuation), return the whole text
    if sentences.is_empty() && !text.trim().is_empty() {
        sentences.push(text.trim().to_string());
    }

    sentences
}

impl KokoroEngine {
    pub fn new() -> Self {
        // Initialize rodio stream
        let stream_handle = match OutputStreamBuilder::from_default_device() {
            Ok(builder) => match builder.open_stream() {
                Ok(h) => Some(h),
                Err(e) => {
                    log::error!("Failed to open audio stream: {}", e);
                    None
                }
            },
            Err(e) => {
                log::error!("Failed to create audio stream builder: {}", e);
                None
            }
        };

        Self {
            tts: Arc::new(RwLock::new(None)),
            _stream_handle: stream_handle.map(SendWrapper),
            sink: None,
        }
    }

    pub async fn load_model(&mut self, model_path: PathBuf, voice_path: PathBuf) -> Result<()> {
        info!("Loading Kokoro model from: {}", model_path.display());
        info!("Using voice from: {}", voice_path.display());

        // TTSKoko::from_paths expects model and voices paths
        let tts = TTSKoko::from_paths(
            model_path.to_string_lossy().as_ref(),
            voice_path.to_string_lossy().as_ref(),
        )
        .await;

        *self.tts.write().await = Some(tts);

        info!("Kokoro model loaded successfully via kokorox");
        Ok(())
    }
}

#[async_trait::async_trait]
impl TTSEngine for KokoroEngine {
    async fn speak(&mut self, text: &str, speed: f32, volume: f32) -> Result<()> {
        info!(
            "Kokoro speaking: '{}' (speed: {}, volume: {})",
            text, speed, volume
        );

        let sh = self
            ._stream_handle
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Audio output handle not initialized"))?;

        // Stop any currently playing audio
        if let Some(ref old_sink) = self.sink {
            old_sink.stop();
        }

        // Create a new sink for streaming playback
        let mixer = sh.0.mixer();
        let (sink, queue_output) = Sink::new();
        sink.set_volume(volume);
        mixer.add(queue_output);
        self.sink = Some(sink);

        // Split text into sentences for streaming
        let sentences = split_into_sentences(text);
        info!("Streaming {} sentences", sentences.len());

        // Get TTS instance
        let tts_guard = self.tts.read().await;
        let tts = tts_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Kokoro TTS not initialized"))?;

        // Generate and play each sentence as it's ready
        for (i, sentence) in sentences.iter().enumerate() {
            info!(
                "Generating sentence {}/{}: '{}'",
                i + 1,
                sentences.len(),
                if sentence.len() > 50 {
                    &sentence[..50]
                } else {
                    sentence
                }
            );

            // Generate speech for this sentence
            let samples = match tts.tts_raw_audio(
                sentence, "en",       // language
                "af_bella", // style/voice name
                speed,      // speed
                None,       // initial_silence
                true,       // auto_detect_language
                false,      // force_style
                false,      // phonemes (input is text, not phonemes)
            ) {
                Ok(samples) => samples,
                Err(e) => {
                    log::warn!("Failed to generate speech for sentence: {:?}", e);
                    continue;
                }
            };

            // Append to the playing queue immediately
            if let Some(ref sink) = self.sink {
                let source = rodio::buffer::SamplesBuffer::new(1, 24000, samples);
                sink.append(source);
                info!("Appended sentence {} to playback queue", i + 1);
            }
        }

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Kokoro stop requested");
        if let Some(ref sink) = self.sink {
            sink.stop();
        }
        Ok(())
    }

    fn is_playing(&self) -> bool {
        if let Some(ref sink) = self.sink {
            !sink.empty()
        } else {
            false
        }
    }
}
