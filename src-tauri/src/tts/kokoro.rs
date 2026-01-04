use crate::tts::TTSEngine;
use anyhow::Result;
use log::info;
use ort::session::Session;
use ort::value::Value;
use rodio::{OutputStreamBuilder, Sink};
use std::path::PathBuf;

pub struct KokoroEngine {
    session: Option<Session>,
    _stream_handle: Option<SendWrapper<rodio::OutputStream>>,
    sink: Option<Sink>,
}

struct SendWrapper<T>(T);
unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {}

impl KokoroEngine {
    pub fn new() -> Self {
        // Initialize rodio stream using the fork's API
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
            session: None,
            _stream_handle: stream_handle.map(SendWrapper),
            sink: None,
        }
    }

    pub fn load_model(&mut self, model_path: PathBuf) -> Result<()> {
        info!("Loading Kokoro ONNX model from: {}", model_path.display());

        let session = Session::builder()?.commit_from_file(model_path)?;

        self.session = Some(session);
        info!("Kokoro model loaded into ORT session");
        Ok(())
    }
}

#[async_trait::async_trait]
impl TTSEngine for KokoroEngine {
    async fn speak(&mut self, _text: &str, _speed: f32, _volume: f32) -> Result<()> {
        info!(
            "Kokoro speaking: '{}' (speed: {}, volume: {})",
            _text, _speed, _volume
        );

        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Kokoro model session not initialized"))?;
        let sh = self
            ._stream_handle
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Audio output handle not initialized"))?;

        // 1. Simple Tokenization (character-based mapping for Kokoro v1.0)
        let mut tokens = vec![0i64]; // Start with PAD/Start token
        for c in _text.chars() {
            let token = match c {
                ' ' => 0,
                'a'..='z' => (c as i64 - 'a' as i64) + 1,
                'A'..='Z' => (c as i64 - 'A' as i64) + 27,
                _ => continue,
            };
            tokens.push(token);
        }
        tokens.push(0); // End token

        let tokens_tensor = Value::from_array(ndarray::Array1::from_vec(tokens))?;

        // 2. Style Embedding (Default 256-dim neutral style)
        let style = vec![0.0f32; 256];
        let style_tensor =
            Value::from_array(ndarray::Array2::from_shape_vec([1, 256], style).unwrap())?;

        // 3. Speed
        let speed_tensor = Value::from_array(ndarray::Array1::from_vec(vec![_speed]))?;

        // 4. Run Inference
        let outputs = session.run(ort::inputs![
            "tokens" => tokens_tensor,
            "style" => style_tensor,
            "speed" => speed_tensor,
        ])?;

        let audio = outputs["audio"].try_extract_tensor::<f32>()?;
        let samples: Vec<f32> = audio.1.to_vec();

        // 5. Playback via Mixer
        let mixer = sh.0.mixer();
        let source = rodio::buffer::SamplesBuffer::new(1, 24000, samples);

        mixer.add(source);

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Kokoro stop requested");
        if let Some(ref sink) = self.sink {
            sink.stop();
        }
        Ok(())
    }
}
