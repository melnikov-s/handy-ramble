use std::{
    io::Error,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Sample, SizedSample,
};

use crate::audio_toolkit::{
    audio::{AudioVisualiser, FrameResampler},
    constants,
    vad::{self, VadFrame},
    VoiceActivityDetector,
};

#[derive(Clone, Debug)]
pub struct SpeechSegment {
    pub index: u64,
    pub samples: Vec<f32>,
}

pub struct StopResult {
    pub raw_full: Vec<f32>,
}

enum Cmd {
    Start,
    Stop(mpsc::Sender<StopResult>),
    Shutdown,
}

pub struct AudioRecorder {
    device: Option<Device>,
    cmd_tx: Option<mpsc::Sender<Cmd>>,
    worker_handle: Option<std::thread::JoinHandle<()>>,
    vad: Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    level_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
    segment_tx: Arc<Mutex<Option<mpsc::Sender<SpeechSegment>>>>,
}

impl AudioRecorder {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(AudioRecorder {
            device: None,
            cmd_tx: None,
            worker_handle: None,
            vad: None,
            level_cb: None,
            segment_tx: Arc::new(Mutex::new(None)),
        })
    }

    pub fn with_vad(mut self, vad: Box<dyn VoiceActivityDetector>) -> Self {
        self.vad = Some(Arc::new(Mutex::new(vad)));
        self
    }

    pub fn with_level_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(Vec<f32>) + Send + Sync + 'static,
    {
        self.level_cb = Some(Arc::new(cb));
        self
    }

    pub fn set_segment_sender(&self, tx: Option<mpsc::Sender<SpeechSegment>>) {
        *self.segment_tx.lock().unwrap() = tx;
    }

    pub fn open(&mut self, device: Option<Device>) -> Result<(), Box<dyn std::error::Error>> {
        if self.worker_handle.is_some() {
            return Ok(()); // already open
        }

        let (sample_tx, sample_rx) = mpsc::channel::<Vec<f32>>();
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();

        let host = crate::audio_toolkit::get_cpal_host();
        let device = match device {
            Some(dev) => dev,
            None => host
                .default_input_device()
                .ok_or_else(|| Error::new(std::io::ErrorKind::NotFound, "No input device found"))?,
        };

        let thread_device = device.clone();
        let vad = self.vad.clone();
        // Move the optional level callback into the worker thread
        let level_cb = self.level_cb.clone();
        let segment_tx = self.segment_tx.clone();

        let worker = std::thread::spawn(move || {
            let config = AudioRecorder::get_preferred_config(&thread_device)
                .expect("failed to fetch preferred config");

            let sample_rate = config.sample_rate().0;
            let channels = config.channels() as usize;

            log::info!(
                "Using device: {:?}\nSample rate: {}\nChannels: {}\nFormat: {:?}",
                thread_device.name(),
                sample_rate,
                channels,
                config.sample_format()
            );

            let stream = match config.sample_format() {
                cpal::SampleFormat::U8 => {
                    AudioRecorder::build_stream::<u8>(&thread_device, &config, sample_tx, channels)
                        .unwrap()
                }
                cpal::SampleFormat::I8 => {
                    AudioRecorder::build_stream::<i8>(&thread_device, &config, sample_tx, channels)
                        .unwrap()
                }
                cpal::SampleFormat::I16 => {
                    AudioRecorder::build_stream::<i16>(&thread_device, &config, sample_tx, channels)
                        .unwrap()
                }
                cpal::SampleFormat::I32 => {
                    AudioRecorder::build_stream::<i32>(&thread_device, &config, sample_tx, channels)
                        .unwrap()
                }
                cpal::SampleFormat::F32 => {
                    AudioRecorder::build_stream::<f32>(&thread_device, &config, sample_tx, channels)
                        .unwrap()
                }
                _ => panic!("unsupported sample format"),
            };

            stream.play().expect("failed to start stream");

            // keep the stream alive while we process samples
            run_consumer(sample_rate, vad, sample_rx, cmd_rx, level_cb, segment_tx);
            // stream is dropped here, after run_consumer returns
        });

        self.device = Some(device);
        self.cmd_tx = Some(cmd_tx);
        self.worker_handle = Some(worker);

        Ok(())
    }

    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tx) = &self.cmd_tx {
            tx.send(Cmd::Start)?;
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<StopResult, Box<dyn std::error::Error>> {
        let (resp_tx, resp_rx) = mpsc::channel();
        if let Some(tx) = &self.cmd_tx {
            tx.send(Cmd::Stop(resp_tx))?;
        }
        Ok(resp_rx.recv()?)
    }

    pub fn close(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tx) = self.cmd_tx.take() {
            let _ = tx.send(Cmd::Shutdown);
        }
        if let Some(h) = self.worker_handle.take() {
            let _ = h.join();
        }
        self.device = None;
        Ok(())
    }

    fn build_stream<T>(
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        sample_tx: mpsc::Sender<Vec<f32>>,
        channels: usize,
    ) -> Result<cpal::Stream, cpal::BuildStreamError>
    where
        T: Sample + SizedSample + Send + 'static,
        f32: cpal::FromSample<T>,
    {
        let mut output_buffer = Vec::new();

        let stream_cb = move |data: &[T], _: &cpal::InputCallbackInfo| {
            output_buffer.clear();

            if channels == 1 {
                // Direct conversion without intermediate Vec
                output_buffer.extend(data.iter().map(|&sample| sample.to_sample::<f32>()));
            } else {
                // Convert to mono directly
                let frame_count = data.len() / channels;
                output_buffer.reserve(frame_count);

                for frame in data.chunks_exact(channels) {
                    let mono_sample = frame
                        .iter()
                        .map(|&sample| sample.to_sample::<f32>())
                        .sum::<f32>()
                        / channels as f32;
                    output_buffer.push(mono_sample);
                }
            }

            if sample_tx.send(output_buffer.clone()).is_err() {
                log::error!("Failed to send samples");
            }
        };

        device.build_input_stream(
            &config.clone().into(),
            stream_cb,
            |err| log::error!("Stream error: {}", err),
            None,
        )
    }

    fn get_preferred_config(
        device: &cpal::Device,
    ) -> Result<cpal::SupportedStreamConfig, Box<dyn std::error::Error>> {
        let supported_configs = device.supported_input_configs()?;
        let mut best_config: Option<cpal::SupportedStreamConfigRange> = None;

        // Try to find a config that supports 16kHz, prioritizing better formats
        for config_range in supported_configs {
            if config_range.min_sample_rate().0 <= constants::WHISPER_SAMPLE_RATE
                && config_range.max_sample_rate().0 >= constants::WHISPER_SAMPLE_RATE
            {
                match best_config {
                    None => best_config = Some(config_range),
                    Some(ref current) => {
                        // Prioritize F32 > I16 > I32 > others
                        let score = |fmt: cpal::SampleFormat| match fmt {
                            cpal::SampleFormat::F32 => 4,
                            cpal::SampleFormat::I16 => 3,
                            cpal::SampleFormat::I32 => 2,
                            _ => 1,
                        };

                        if score(config_range.sample_format()) > score(current.sample_format()) {
                            best_config = Some(config_range);
                        }
                    }
                }
            }
        }

        if let Some(config) = best_config {
            return Ok(config.with_sample_rate(cpal::SampleRate(constants::WHISPER_SAMPLE_RATE)));
        }

        // If no config supports 16kHz, fall back to default
        Ok(device.default_input_config()?)
    }
}

fn run_consumer(
    in_sample_rate: u32,
    vad: Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
    sample_rx: mpsc::Receiver<Vec<f32>>,
    cmd_rx: mpsc::Receiver<Cmd>,
    level_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
    segment_tx: Arc<Mutex<Option<mpsc::Sender<SpeechSegment>>>>,
) {
    let mut frame_resampler = FrameResampler::new(
        in_sample_rate as usize,
        constants::WHISPER_SAMPLE_RATE as usize,
        Duration::from_millis(30),
    );

    let mut processed_samples = Vec::<f32>::new();
    let mut recording = false;

    let mut raw_full: Vec<f32> = Vec::new();
    let mut current_segment: Vec<f32> = Vec::new();
    let mut in_segment = false;
    let mut segment_index: u64 = 0;
    let mut silence_run_frames: usize = 0;

    const END_SILENCE_FRAMES: usize = 10; // ~300ms at 30ms/frame
    const MIN_SEGMENT_SAMPLES: usize = 16000; // ~1 second minimum

    // ---------- spectrum visualisation setup ---------------------------- //
    const BUCKETS: usize = 16;
    const WINDOW_SIZE: usize = 512;
    let mut visualizer = AudioVisualiser::new(
        in_sample_rate,
        WINDOW_SIZE,
        BUCKETS,
        400.0,  // vocal_min_hz
        4000.0, // vocal_max_hz
    );

    fn handle_frame(
        samples: &[f32],
        recording: bool,
        vad: &Option<Arc<Mutex<Box<dyn vad::VoiceActivityDetector>>>>,
        out_buf: &mut Vec<f32>,
        raw_full: &mut Vec<f32>,
        current_segment: &mut Vec<f32>,
        in_segment: &mut bool,
        segment_index: &mut u64,
        silence_run_frames: &mut usize,
        segment_tx: &Arc<Mutex<Option<mpsc::Sender<SpeechSegment>>>>,
    ) {
        if !recording {
            return;
        }

        if let Some(vad_arc) = vad {
            let mut det = vad_arc.lock().unwrap();
            match det.push_frame(samples).unwrap_or(VadFrame::Speech(samples)) {
                VadFrame::Speech(buf) => {
                    out_buf.extend_from_slice(buf);
                    raw_full.extend_from_slice(buf);
                    current_segment.extend_from_slice(buf);
                    *in_segment = true;
                    *silence_run_frames = 0;
                }
                VadFrame::Noise => {
                    if *in_segment {
                        *silence_run_frames += 1;
                        if *silence_run_frames >= END_SILENCE_FRAMES {
                            if current_segment.len() >= MIN_SEGMENT_SAMPLES {
                                if let Some(tx) = segment_tx.lock().unwrap().as_ref() {
                                    let segment = SpeechSegment {
                                        index: *segment_index,
                                        samples: std::mem::take(current_segment),
                                    };
                                    let _ = tx.send(segment);
                                } else {
                                    current_segment.clear();
                                }
                                *segment_index += 1;
                            } else {
                                current_segment.clear();
                            }
                            *in_segment = false;
                            *silence_run_frames = 0;
                        }
                    }
                }
            }
        } else {
            out_buf.extend_from_slice(samples);
            raw_full.extend_from_slice(samples);
            current_segment.extend_from_slice(samples);
            *in_segment = true;
            *silence_run_frames = 0;
        }
    }

    loop {
        let raw = match sample_rx.recv() {
            Ok(s) => s,
            Err(_) => break, // stream closed
        };

        // ---------- spectrum processing ---------------------------------- //
        if let Some(buckets) = visualizer.feed(&raw) {
            if let Some(cb) = &level_cb {
                cb(buckets);
            }
        }

        // ---------- existing pipeline ------------------------------------ //
        frame_resampler.push(&raw, &mut |frame: &[f32]| {
            handle_frame(
                frame,
                recording,
                &vad,
                &mut processed_samples,
                &mut raw_full,
                &mut current_segment,
                &mut in_segment,
                &mut segment_index,
                &mut silence_run_frames,
                &segment_tx,
            )
        });

        // non-blocking check for a command
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Cmd::Start => {
                    processed_samples.clear();
                    raw_full.clear();
                    current_segment.clear();
                    in_segment = false;
                    segment_index = 0;
                    silence_run_frames = 0;
                    recording = true;
                    visualizer.reset(); // Reset visualization buffer
                    if let Some(v) = &vad {
                        v.lock().unwrap().reset();
                    }
                }
                Cmd::Stop(reply_tx) => {
                    recording = false;

                    frame_resampler.finish(&mut |frame: &[f32]| {
                        // we still want to process the last few frames
                        handle_frame(
                            frame,
                            true,
                            &vad,
                            &mut processed_samples,
                            &mut raw_full,
                            &mut current_segment,
                            &mut in_segment,
                            &mut segment_index,
                            &mut silence_run_frames,
                            &segment_tx,
                        )
                    });

                    // Emit final segment if in_segment and current_segment is non-empty
                    if in_segment && !current_segment.is_empty() {
                        if let Some(tx) = segment_tx.lock().unwrap().as_ref() {
                            let segment = SpeechSegment {
                                index: segment_index,
                                samples: std::mem::take(&mut current_segment),
                            };
                            let _ = tx.send(segment);
                        }
                    }

                    // Clear segment state
                    current_segment.clear();
                    in_segment = false;
                    segment_index = 0;
                    silence_run_frames = 0;

                    let _ = reply_tx.send(StopResult {
                        raw_full: std::mem::take(&mut raw_full),
                    });
                    processed_samples.clear();
                }
                Cmd::Shutdown => return,
            }
        }
    }
}
