use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use rubato::{FftFixedIn, Resampler};
use std::sync::{Arc, Mutex};
use std::vec::Vec;
use tauri::{App, Manager};
use vad_rs::Vad;

const WHISPER_SAMPLE_RATE: usize = 16000;

#[derive(Clone, Debug)]
pub enum RecordingState {
    Idle,
    Recording { binding_id: String },
}

#[derive(Clone)]
pub struct AudioRecordingManager {
    state: Arc<Mutex<RecordingState>>,
    buffer: Arc<Mutex<Vec<f32>>>,
}

trait SampleToF32 {
    fn to_f32(&self) -> f32;
}

impl SampleToF32 for i8 {
    fn to_f32(&self) -> f32 {
        *self as f32 / 128.0
    }
}

impl SampleToF32 for i16 {
    fn to_f32(&self) -> f32 {
        *self as f32 / 32768.0
    }
}

impl SampleToF32 for i32 {
    fn to_f32(&self) -> f32 {
        *self as f32 / 2147483648.0
    }
}

impl SampleToF32 for f32 {
    fn to_f32(&self) -> f32 {
        *self
    }
}

impl AudioRecordingManager {
    pub fn new(app: &App) -> Result<Self, anyhow::Error> {
        let vad_path = app.path().resolve(
            "resources/models/silero_vad_v4.onnx",
            tauri::path::BaseDirectory::Resource,
        )?;
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::Error::msg("No input device available"))?;

        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;

        // Configure the resampler - keeping 1024 as input size for FFT efficiency
        let resampler = FftFixedIn::new(sample_rate as usize, WHISPER_SAMPLE_RATE, 1024, 2, 1)?;

        let vad = Arc::new(Mutex::new(Vad::new(vad_path, WHISPER_SAMPLE_RATE).unwrap()));
        let vad_clone = Arc::clone(&vad);

        let state = Arc::new(Mutex::new(RecordingState::Idle));
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let resampler = Arc::new(Mutex::new(resampler));

        let state_clone = Arc::clone(&state);
        let buffer_clone = Arc::clone(&buffer);
        let resampler_clone = Arc::clone(&resampler);

        // Create a temporary buffer to accumulate samples
        let temp_buffer = Arc::new(Mutex::new(Vec::new()));
        let temp_buffer_clone = Arc::clone(&temp_buffer);

        // Create a buffer for resampled chunks waiting for VAD processing
        let vad_buffer = Arc::new(Mutex::new(Vec::new()));
        let vad_buffer_clone = Arc::clone(&vad_buffer);

        // Generic function to process audio data
        fn process_audio<T: SampleToF32 + Send + 'static>(
            data: &[T],
            channels: usize,
            state_clone: Arc<Mutex<RecordingState>>,
            temp_buffer_clone: Arc<Mutex<Vec<f32>>>,
            resampler_clone: Arc<Mutex<FftFixedIn<f32>>>,
            vad_buffer_clone: Arc<Mutex<Vec<f32>>>,
            buffer_clone: Arc<Mutex<Vec<f32>>>,
            vad_clone: Arc<Mutex<Vad>>,
        ) {
            let state_guard = state_clone.lock().unwrap();
            if let RecordingState::Recording { .. } = *state_guard {
                let mut temp_buffer = temp_buffer_clone.lock().unwrap();

                // Handle multichannel audio by mixing down to mono
                if channels > 1 {
                    // Process chunks of `channels` samples at a time (each chunk is one audio frame)
                    for chunk in data.chunks(channels) {
                        // Average the channels to create a mono sample
                        let mono_sample: f32 =
                            chunk.iter().map(|s| s.to_f32()).sum::<f32>() / channels as f32;
                        temp_buffer.push(mono_sample);
                    }
                } else {
                    // Single channel processing
                    let f32_data: Vec<f32> = data.iter().map(|sample| sample.to_f32()).collect();
                    temp_buffer.extend_from_slice(&f32_data);
                }

                // Process when we have enough samples
                while temp_buffer.len() >= 1024 {
                    // Take the first 1024 samples for processing
                    let chunk: Vec<f32> = temp_buffer.drain(..1024).collect();

                    // Convert input data to the format expected by Rubato
                    let input_frames = vec![chunk];

                    // Process the audio chunk through the resampler
                    let mut resampler = resampler_clone.lock().unwrap();
                    if let Ok(resampled) = resampler.process(&input_frames, None) {
                        // Add resampled data to VAD buffer
                        let mut vad_buffer = vad_buffer_clone.lock().unwrap();
                        vad_buffer.extend_from_slice(&resampled[0]);

                        // Process 30ms chunks (480 samples) for VAD
                        while vad_buffer.len() >= 480 {
                            let chunk = vad_buffer.drain(..480).collect::<Vec<f32>>();

                            // Use VAD to detect speech
                            if let Ok(mut vad) = vad_clone.lock() {
                                match vad.compute(&chunk) {
                                    Ok(result) => {
                                        if result.prob > 0.1 {
                                            let mut buffer = buffer_clone.lock().unwrap();
                                            buffer.extend_from_slice(&chunk);
                                        }
                                    }
                                    Err(error) => {
                                        eprintln!("Error computing VAD: {:?}", error)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        std::thread::spawn(move || {
            let err_fn = |err| eprintln!("Error in stream: {}", err);

            // Build the appropriate stream based on the sample format
            // Store the number of channels for use in the closure
            let channels = config.channels() as usize;
            println!("Using {} channel(s) for recording", channels);

            let stream = match config.sample_format() {
                SampleFormat::I8 => device.build_input_stream(
                    &config.into(),
                    move |data: &[i8], _| {
                        process_audio(
                            data,
                            channels,
                            Arc::clone(&state_clone),
                            Arc::clone(&temp_buffer_clone),
                            Arc::clone(&resampler_clone),
                            Arc::clone(&vad_buffer_clone),
                            Arc::clone(&buffer_clone),
                            Arc::clone(&vad_clone),
                        )
                    },
                    err_fn,
                    None,
                ),
                SampleFormat::I16 => device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _| {
                        process_audio(
                            data,
                            channels,
                            Arc::clone(&state_clone),
                            Arc::clone(&temp_buffer_clone),
                            Arc::clone(&resampler_clone),
                            Arc::clone(&vad_buffer_clone),
                            Arc::clone(&buffer_clone),
                            Arc::clone(&vad_clone),
                        )
                    },
                    err_fn,
                    None,
                ),
                SampleFormat::I32 => device.build_input_stream(
                    &config.into(),
                    move |data: &[i32], _| {
                        process_audio(
                            data,
                            channels,
                            Arc::clone(&state_clone),
                            Arc::clone(&temp_buffer_clone),
                            Arc::clone(&resampler_clone),
                            Arc::clone(&vad_buffer_clone),
                            Arc::clone(&buffer_clone),
                            Arc::clone(&vad_clone),
                        )
                    },
                    err_fn,
                    None,
                ),
                SampleFormat::F32 => device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _| {
                        process_audio(
                            data,
                            channels,
                            Arc::clone(&state_clone),
                            Arc::clone(&temp_buffer_clone),
                            Arc::clone(&resampler_clone),
                            Arc::clone(&vad_buffer_clone),
                            Arc::clone(&buffer_clone),
                            Arc::clone(&vad_clone),
                        )
                    },
                    err_fn,
                    None,
                ),
                sample_format => {
                    // Use anyhow to return a proper error instead of panicking
                    panic!("Unsupported sample format: {:?}", sample_format);
                }
            }
            .expect("Failed to build input stream");

            stream.play().expect("Failed to play stream");
            std::thread::park();
        });

        Ok(Self { state, buffer })
    }

    pub fn try_start_recording(&self, binding_id: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        match *state {
            RecordingState::Idle => {
                // Clear the buffer before starting new recording
                self.buffer.lock().unwrap().clear();
                *state = RecordingState::Recording {
                    binding_id: binding_id.to_string(),
                };
                println!("Started recording for binding {}", binding_id);
                true
            }
            RecordingState::Recording {
                binding_id: ref active_id,
            } => {
                println!(
                    "Cannot start recording: already recording for binding {}",
                    active_id
                );
                false
            }
        }
    }

    pub fn stop_recording(&self, binding_id: &str) -> Option<Vec<f32>> {
        let mut state = self.state.lock().unwrap();
        println!("Stop recording called from binding {}", binding_id);
        match *state {
            RecordingState::Recording {
                binding_id: ref active_id,
            } if active_id == binding_id => {
                *state = RecordingState::Idle;
                println!("Stopped recording for binding {}", binding_id);

                let mut buffer = self.buffer.lock().unwrap();
                let audio_data: Vec<f32> = buffer.drain(..).collect();

                let samples = audio_data.len();

                if samples < WHISPER_SAMPLE_RATE && samples > 1000 {
                    let target_samples = WHISPER_SAMPLE_RATE * 5 / 4; // sample rate * 1.25
                    let mut padded_audio = audio_data;
                    padded_audio.resize(target_samples, 0.0); // Pad with silence (zeros)
                    Some(padded_audio)
                } else {
                    Some(audio_data)
                }
            }
            _ => {
                // println!("Cannot stop recording: not recording or wrong binding");
                None
            }
        }
    }
}
