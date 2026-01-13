use crate::tts::TTSEngine;
use anyhow::Result;
use log::info;
use ort::session::Session;
use ort::value::Value;
use rodio::{OutputStreamBuilder, Sink};
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct KokoroEngine {
    session: Option<Session>,
    _stream_handle: Option<SendWrapper<rodio::OutputStream>>,
    sink: Option<Sink>,
    tokenizer: Option<EspeakIpaTokenizer>,
    voice: Option<VoiceStyle>,
}

struct SendWrapper<T>(T);
unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {}

#[derive(Deserialize)]
struct KokoroConfig {
    vocab: HashMap<String, i64>,
}

const KOKORO_CONFIG_JSON: &str = include_str!("../../resources/kokoro_config.json");

fn find_espeak_binary() -> Option<String> {
    let path_var = std::env::var("PATH").ok()?;
    for dir in path_var.split(':') {
        let candidate = Path::new(dir).join("espeak-ng");
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    for dir in path_var.split(':') {
        let candidate = Path::new(dir).join("espeak");
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

struct EspeakG2P {
    binary: String,
}

impl EspeakG2P {
    fn new() -> Result<Self> {
        let binary = find_espeak_binary()
            .ok_or_else(|| anyhow::anyhow!("espeak-ng not found in PATH"))?;
        Ok(Self { binary })
    }

    fn text_to_ipa(&self, text: &str) -> Result<String> {
        let output = Command::new(&self.binary)
            .arg("-q")
            .arg("--ipa=3")
            .arg("-v")
            .arg("en-us")
            .arg(text)
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "espeak-ng failed with status: {}",
                output.status
            ));
        }

        let mut phonemes = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if phonemes.is_empty() {
            return Err(anyhow::anyhow!("No phonemes returned from espeak-ng"));
        }
        phonemes = phonemes.replace('\n', " ");
        Ok(phonemes)
    }
}

struct EspeakIpaTokenizer {
    vocab: HashMap<String, i64>,
    model_max_length: usize,
    g2p: EspeakG2P,
    max_token_chars: usize,
}

struct VoiceStyle {
    data: Vec<f32>,
    vector_size: usize,
}

impl VoiceStyle {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        if bytes.len() % 4 != 0 {
            return Err(anyhow::anyhow!("Voice file has invalid length"));
        }

        let data: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        Ok(Self {
            data,
            vector_size: 256,
        })
    }

    fn style_for_token_length(&self, token_length: usize) -> Vec<f32> {
        let offset = token_length * self.vector_size;
        if offset + self.vector_size <= self.data.len() {
            return self.data[offset..offset + self.vector_size].to_vec();
        }

        let last_vector_start = (self.data.len() / self.vector_size) * self.vector_size;
        if last_vector_start + self.vector_size <= self.data.len() {
            return self.data[last_vector_start..last_vector_start + self.vector_size].to_vec();
        }

        self.data
            .iter()
            .take(self.vector_size)
            .cloned()
            .collect()
    }
}

impl EspeakIpaTokenizer {
    fn new(vocab: HashMap<String, i64>) -> Result<Self> {
        let g2p = EspeakG2P::new()?;
        let max_token_chars = Self::max_token_chars(&vocab);

        Ok(Self {
            vocab,
            model_max_length: 512,
            g2p,
            max_token_chars,
        })
    }

    fn max_token_chars(vocab: &HashMap<String, i64>) -> usize {
        vocab.keys().map(|k| k.chars().count()).max().unwrap_or(1)
    }

    fn espeak_ipa_to_misaki(&self, ipa: &str) -> String {
        let mut result = ipa.replace('\u{0361}', "^");

        let from_espeaks = vec![
            ("ʔˌn\u{0329}", "tᵊn"),
            ("a^ɪ", "I"),
            ("a^ʊ", "W"),
            ("d^ʒ", "ʤ"),
            ("e^ɪ", "A"),
            ("t^ʃ", "ʧ"),
            ("ɔ^ɪ", "Y"),
            ("ə^l", "ᵊl"),
            ("ʔn", "tᵊn"),
            ("ɚ", "əɹ"),
            ("ʲO", "jO"),
            ("ʲQ", "jQ"),
            ("\u{0303}", ""),
            ("e", "A"),
            ("r", "ɹ"),
            ("x", "k"),
            ("ç", "k"),
            ("ɐ", "ə"),
            ("ɬ", "l"),
            ("ʔ", "t"),
            ("ʲ", ""),
        ];

        for (old, new) in from_espeaks {
            result = result.replace(old, new);
        }

        let mut chars: Vec<char> = result.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if i + 1 < chars.len() && chars[i + 1] == '\u{0329}' {
                let consonant = chars[i];
                chars[i] = 'ᵊ';
                chars[i + 1] = consonant;
                i += 2;
            } else {
                i += 1;
            }
        }
        result = chars.into_iter().collect();

        result = result.replace('\u{0329}', "");
        result = result.replace("o^ʊ", "O");
        result = result.replace("ɜːɹ", "ɜɹ");
        result = result.replace("ɜː", "ɜɹ");
        result = result.replace("ɪə", "iə");
        result = result.replace("ː", "");
        result = result.replace("^", "");

        result
    }

    fn tokenize_longest(&self, phonemes: &str) -> Vec<i64> {
        let mut ids = Vec::with_capacity(phonemes.len());
        let chars: Vec<char> = phonemes.chars().collect();
        let mut i = 0;
        let max_len = self.max_token_chars;

        while i < chars.len() {
            let mut matched = false;
            let limit = max_len.min(chars.len() - i);

            for len in (1..=limit).rev() {
                let cand: String = chars[i..i + len].iter().collect();
                if let Some(&id) = self.vocab.get(&cand) {
                    ids.push(id);
                    i += len;
                    matched = true;
                    break;
                }
            }

            if !matched {
                if !chars[i].is_whitespace() {
                    log::warn!("Unknown phoneme token: {:?}", chars[i]);
                }
                i += 1;
            }
        }

        ids
    }

    fn encode(&self, text: &str) -> Result<Vec<i64>> {
        let max_len = self.model_max_length;
        let ipa_text = self.g2p.text_to_ipa(text)?;
        let phonemes = self.espeak_ipa_to_misaki(&ipa_text);

        let mut tokens = Vec::with_capacity(phonemes.len() + 2);
        tokens.push(0);
        let mut inner = self.tokenize_longest(&phonemes);
        tokens.append(&mut inner);
        tokens.push(0);

        if tokens.len() > max_len {
            let keep_inner = max_len.saturating_sub(2);
            let mut truncated = Vec::with_capacity(max_len);
            truncated.push(0);
            truncated.extend_from_slice(&tokens[1..1 + keep_inner]);
            truncated.push(0);
            return Ok(truncated);
        }

        Ok(tokens)
    }
}

fn fallback_tokenize(text: &str) -> Vec<i64> {
    let mut tokens = vec![0i64];
    for c in text.to_lowercase().chars() {
        let token = match c {
            ' ' => 16,
            'a' => 43,
            'b' => 44,
            'c' => 45,
            'd' => 46,
            'e' => 47,
            'f' => 48,
            'g' => 92,
            'h' => 50,
            'i' => 51,
            'j' => 52,
            'k' => 53,
            'l' => 54,
            'm' => 55,
            'n' => 56,
            'o' => 57,
            'p' => 58,
            'q' => 59,
            'r' => 60,
            's' => 61,
            't' => 62,
            'u' => 63,
            'v' => 64,
            'w' => 65,
            'x' => 66,
            'y' => 67,
            'z' => 68,
            '.' => 4,
            ',' => 3,
            '!' => 5,
            '?' => 6,
            _ => continue,
        };
        tokens.push(token);
    }
    tokens.push(0);

    if tokens.len() > 512 {
        tokens.truncate(512);
        tokens[0] = 0;
        if let Some(last) = tokens.last_mut() {
            *last = 0;
        }
    }

    tokens
}

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
            tokenizer: None,
            voice: None,
        }
    }

    pub fn load_model(&mut self, model_path: PathBuf, voice_path: PathBuf) -> Result<()> {
        info!("Loading Kokoro ONNX model from: {}", model_path.display());

        let session = Session::builder()?.commit_from_file(model_path)?;

        let tokenizer = match serde_json::from_str::<KokoroConfig>(KOKORO_CONFIG_JSON) {
            Ok(config) => match EspeakIpaTokenizer::new(config.vocab) {
                Ok(tokenizer) => Some(tokenizer),
                Err(err) => {
                    log::warn!("Failed to initialize espeak tokenizer: {}", err);
                    None
                }
            },
            Err(err) => {
                log::warn!("Failed to parse Kokoro config: {}", err);
                None
            }
        };

        let voice = match VoiceStyle::load(&voice_path) {
            Ok(voice) => Some(voice),
            Err(err) => {
                log::warn!("Failed to load Kokoro voice style: {}", err);
                None
            }
        };

        self.session = Some(session);
        self.tokenizer = tokenizer;
        self.voice = voice;
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

        // 1. Tokenization (IPA phonemes via espeak-ng, with fallback)
        let tokens = if let Some(tokenizer) = &self.tokenizer {
            match tokenizer.encode(_text) {
                Ok(tokens) => tokens,
                Err(err) => {
                    log::warn!("Failed to tokenize with espeak-ng: {}", err);
                    fallback_tokenize(_text)
                }
            }
        } else {
            fallback_tokenize(_text)
        };
        let token_len = tokens.len();

        let tokens_tensor =
            Value::from_array(ndarray::Array2::from_shape_vec([1, token_len], tokens)?)?;

        // 2. Style Embedding
        let style = if let Some(voice) = &self.voice {
            voice.style_for_token_length(token_len)
        } else {
            vec![0.0f32; 256]
        };
        let style_tensor =
            Value::from_array(ndarray::Array2::from_shape_vec([1, 256], style).unwrap())?;

        // 3. Speed (Must be f32 tensor of shape [1])
        let speed_tensor = Value::from_array(ndarray::Array1::from_vec(vec![_speed]))?;

        // 4. Run Inference
        let outputs = session.run(ort::inputs![
            "input_ids" => tokens_tensor,
            "style" => style_tensor,
            "speed" => speed_tensor,
        ])?;

        // The model output is unnamed (at index 0)
        let (_, audio_value) = outputs.into_iter().next().ok_or_else(|| anyhow::anyhow!("No output found"))?;
        let audio = audio_value.try_extract_tensor::<f32>()?;
        
        // The audio output from Kokoro v1.0 ONNX is [1, samples] or [samples]
        // We ensure we get the flat sample data.
        let samples: Vec<f32> = audio.1.to_vec();

        // 5. Playback via Mixer with controllable Sink
        if let Some(ref old_sink) = self.sink {
            old_sink.stop();
        }

        let mixer = sh.0.mixer();
        let (sink, queue_output) = Sink::new();
        // Kokoro v1.0 usually outputs at 24000Hz
        let source = rodio::buffer::SamplesBuffer::new(1, 24000, samples);
        sink.set_volume(_volume);
        sink.append(source);
        
        mixer.add(queue_output);
        
        // Store sink so we can stop it
        self.sink = Some(sink);

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
