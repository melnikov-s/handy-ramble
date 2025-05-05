use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{FftFixedIn, Resampler};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::vec::Vec;
use vad_rs::{Vad, VadStatus};

#[derive(Clone, Debug)]
pub enum RecordingState {
    Idle,
    Recording { binding_id: String },
}

pub struct AudioRecordingManager {
    state: Arc<Mutex<RecordingState>>,
    buffer: Arc<Mutex<Vec<f32>>>,
}

impl AudioRecordingManager {
    pub fn new(vad_path: &PathBuf) -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::Error::msg("No input device available"))?;

        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;

        // Configure the resampler - keeping 1024 as input size for FFT efficiency
        let resampler = FftFixedIn::new(sample_rate as usize, 16000, 1024, 2, 1)?;

        let vad = Arc::new(Mutex::new(Vad::new(vad_path, 16000).unwrap()));
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

        std::thread::spawn(move || {
            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let state_guard = state_clone.lock().unwrap();
                        if let RecordingState::Recording { .. } = *state_guard {
                            let mut temp_buffer = temp_buffer_clone.lock().unwrap();
                            temp_buffer.extend_from_slice(data);

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
                                            // println!("VAD lock acquired");
                                            match vad.compute(&chunk) {
                                                Ok(mut result) => {
                                                    if result.prob > 0.15 {
                                                        let mut buffer =
                                                            buffer_clone.lock().unwrap();
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
                    },
                    |err| eprintln!("Error in stream: {}", err),
                    None,
                ),
                sample_format => panic!("Unsupported sample format: {:?}", sample_format),
            }
            .unwrap();

            stream.play().unwrap();
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

                // Calculate duration in milliseconds
                // 16000 is our target sample rate after resampling
                let duration_ms = (audio_data.len() as f32 / 16000.0) * 1000.0;

                if duration_ms < 300.0 {
                    // Discard the audio if it's too short
                    Some(Vec::new())
                } else {
                    // Pad to minimum 1000ms if needed
                    if duration_ms < 1000.0 {
                        let target_samples = (16400.0 * (1000.0 / 1000.0)) as usize; // 16000 samples for 1 second
                        let mut padded_audio = audio_data;
                        padded_audio.resize(target_samples, 0.0); // Pad with silence (zeros)
                        Some(padded_audio)
                    } else {
                        Some(audio_data)
                    }
                }
            }
            _ => {
                // println!("Cannot stop recording: not recording or wrong binding");
                None
            }
        }
    }
}
