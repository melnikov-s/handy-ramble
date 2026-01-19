use crate::tts::TTSEngine;
use anyhow::Result;
use kokorox::tts::koko::TTSKoko;
use log::{info, debug, error};
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
        info!("Initializing KokoroEngine audio output...");
        let stream_handle = match OutputStreamBuilder::from_default_device() {
            Ok(builder) => {
                info!("Got audio output stream builder for default device");
                match builder.open_stream() {
                    Ok(h) => {
                        info!("Successfully opened audio output stream");
                        Some(h)
                    },
                    Err(e) => {
                        error!("Failed to open audio stream: {}", e);
                        None
                    }
                }
            },
            Err(e) => {
                error!("Failed to create audio stream builder: {}", e);
                None
            }
        };

        if stream_handle.is_none() {
            error!("KokoroEngine initialized WITHOUT audio output - TTS will not work!");
        }

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

        info!("speak() called with text length: {}, speed: {}, volume: {}", text.len(), speed, volume);
        
        let sh = self
            ._stream_handle
            .as_ref()
            .ok_or_else(|| {
                error!("Audio output handle not initialized!");
                anyhow::anyhow!("Audio output handle not initialized")
            })?;

        // Stop any currently playing audio
        if let Some(ref old_sink) = self.sink {
            info!("Stopping previous sink");
            old_sink.stop();
        }

        // Create a new sink for streaming playback
        let mixer = sh.0.mixer();
        info!("Got mixer from stream handle, creating new sink...");
        let (sink, queue_output) = Sink::new();
        sink.set_volume(volume);
        info!("Created new sink with volume: {}", volume);
        mixer.add(queue_output);
        self.sink = Some(sink);
        info!("Sink added to mixer, ready for audio");

        // Split text into sentences for streaming
        let sentences = split_into_sentences(text);
        info!("Streaming {} sentences", sentences.len());

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

            // Clone data needed for the blocking task
            let sentence_clone = sentence.clone();
            let speed_clone = speed;
            let tts_clone = self.tts.clone();

            // Run TTS generation in a blocking task to avoid blocking the async runtime
            info!("Starting TTS audio generation for sentence {} (spawning blocking task)", i + 1);
            let gen_start = std::time::Instant::now();
            
            let samples_result = tokio::task::spawn_blocking(move || {
                // Acquire read lock synchronously inside the blocking task
                let tts_guard = tts_clone.blocking_read();
                let tts = match tts_guard.as_ref() {
                    Some(t) => t,
                    None => return Err(anyhow::anyhow!("Kokoro TTS not initialized")),
                };
                
                info!("Inside blocking task, calling tts_raw_audio...");
                match tts.tts_raw_audio(
                    &sentence_clone,
                    "en",           // language
                    "af_bella",     // style/voice name
                    speed_clone,    // speed
                    None,           // initial_silence
                    true,           // auto_detect_language
                    false,          // force_style
                    false,          // phonemes (input is text, not phonemes)
                ) {
                    Ok(samples) => {
                        info!("tts_raw_audio returned {} samples", samples.len());
                        Ok(samples)
                    }
                    Err(e) => Err(anyhow::anyhow!("TTS generation failed: {:?}", e)),
                }
            })
            .await;

            let samples = match samples_result {
                Ok(Ok(samples)) => {
                    info!("TTS audio generation completed in {:?}, got {} samples ({:.2}s of audio)", 
                        gen_start.elapsed(), samples.len(), samples.len() as f32 / 24000.0);
                    samples
                }
                Ok(Err(e)) => {
                    error!("Failed to generate speech for sentence: {:?}", e);
                    continue;
                }
                Err(e) => {
                    error!("Blocking task panicked: {:?}", e);
                    continue;
                }
            };

            // Append to the playing queue immediately
            if let Some(ref sink) = self.sink {
                let source = rodio::buffer::SamplesBuffer::new(1, 24000, samples.clone());
                info!("Appending {} samples to sink (sink empty before: {})", samples.len(), sink.empty());
                sink.append(source);
                info!("Appended sentence {} to playback queue (sink empty after: {}, is_paused: {})", 
                    i + 1, sink.empty(), sink.is_paused());
            } else {
                error!("No sink available to append audio!");
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
