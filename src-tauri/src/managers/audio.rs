use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{FftFixedIn, Resampler};
use std::sync::{Arc, Mutex};
use std::vec::Vec;

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
    pub fn new() -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::Error::msg("No input device available"))?;

        let config = device.default_input_config()?;

        // let channels = config.channels();
        let sample_rate = config.sample_rate().0;

        // Configure the resampler with more flexible parameters
        let resampler = FftFixedIn::new(sample_rate as usize, 16000, 1024, 2, 1)?;

        let state = Arc::new(Mutex::new(RecordingState::Idle));
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let resampler = Arc::new(Mutex::new(resampler));

        let state_clone = Arc::clone(&state);
        let buffer_clone = Arc::clone(&buffer);
        let resampler_clone = Arc::clone(&resampler);

        // Create a temporary buffer to accumulate samples
        let temp_buffer = Arc::new(Mutex::new(Vec::new()));
        let temp_buffer_clone = Arc::clone(&temp_buffer);

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
                            if temp_buffer.len() >= 1024 {
                                // Convert input data to the format expected by Rubato
                                let mut chunks = Vec::new();
                                for chunk in temp_buffer.chunks(1024) {
                                    chunks.push(chunk.to_vec());
                                }
                                let input_frames = vec![chunks.remove(0)]; // Take the first complete chunk

                                // Process the audio chunk
                                let mut resampler = resampler_clone.lock().unwrap();
                                if let Ok(resampled) = resampler.process(&input_frames, None) {
                                    // Store the resampled audio
                                    let mut buffer = buffer_clone.lock().unwrap();
                                    buffer.extend_from_slice(&resampled[0]);
                                }

                                // Keep remaining samples
                                *temp_buffer = temp_buffer.split_off(1024);
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
