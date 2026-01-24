use crate::audio_toolkit::{
    list_input_devices, vad::SmoothedVad, AudioRecorder, SileroVad, SpeechSegment,
};
use crate::helpers::clamshell;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, AppSettings};
use crate::utils;
use log::{debug, error, info, warn};
use std::collections::BTreeMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::Manager;

fn set_mute(mute: bool) {
    // Expected behavior:
    // - Windows: works on most systems using standard audio drivers.
    // - Linux: works on many systems (PipeWire, PulseAudio, ALSA),
    //   but some distros may lack the tools used.
    // - macOS: works on most standard setups via AppleScript.
    // If unsupported, fails silently.

    #[cfg(target_os = "windows")]
    {
        unsafe {
            use windows::Win32::{
                Media::Audio::{
                    eMultimedia, eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator,
                    MMDeviceEnumerator,
                },
                System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
            };

            macro_rules! unwrap_or_return {
                ($expr:expr) => {
                    match $expr {
                        Ok(val) => val,
                        Err(_) => return,
                    }
                };
            }

            // Initialize the COM library for this thread.
            // If already initialized (e.g., by another library like Tauri), this does nothing.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let all_devices: IMMDeviceEnumerator =
                unwrap_or_return!(CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL));
            let default_device =
                unwrap_or_return!(all_devices.GetDefaultAudioEndpoint(eRender, eMultimedia));
            let volume_interface = unwrap_or_return!(
                default_device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            );

            let _ = volume_interface.SetMute(mute, std::ptr::null());
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        let mute_val = if mute { "1" } else { "0" };
        let amixer_state = if mute { "mute" } else { "unmute" };

        // Try multiple backends to increase compatibility
        // 1. PipeWire (wpctl)
        if Command::new("wpctl")
            .args(["set-mute", "@DEFAULT_AUDIO_SINK@", mute_val])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return;
        }

        // 2. PulseAudio (pactl)
        if Command::new("pactl")
            .args(["set-sink-mute", "@DEFAULT_SINK@", mute_val])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return;
        }

        // 3. ALSA (amixer)
        let _ = Command::new("amixer")
            .args(["set", "Master", amixer_state])
            .output();
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let script = format!(
            "set volume output muted {}",
            if mute { "true" } else { "false" }
        );
        let _ = Command::new("osascript").args(["-e", &script]).output();
    }
}

const WHISPER_SAMPLE_RATE: usize = 16000;

/* ──────────────────────────────────────────────────────────────── */

pub struct StreamingTranscriptionSession {
    segment_tx: mpsc::Sender<SpeechSegment>,
    result_rx: mpsc::Receiver<(u64, anyhow::Result<String>)>,
    worker_handle: Option<JoinHandle<()>>,
    segments_text: BTreeMap<u64, String>,
}

impl StreamingTranscriptionSession {
    pub fn new(transcription_manager: Arc<TranscriptionManager>) -> Self {
        let (segment_tx, segment_rx) = mpsc::channel::<SpeechSegment>();
        let (result_tx, result_rx) = mpsc::channel::<(u64, anyhow::Result<String>)>();

        let worker_handle = thread::spawn(move || {
            while let Ok(segment) = segment_rx.recv() {
                debug!(
                    "Streaming transcription: processing segment {} ({} samples)",
                    segment.index,
                    segment.samples.len()
                );
                let result = transcription_manager.transcribe(segment.samples);
                if result_tx.send((segment.index, result)).is_err() {
                    break;
                }
            }
            debug!("Streaming transcription worker exiting");
        });

        Self {
            segment_tx,
            result_rx,
            worker_handle: Some(worker_handle),
            segments_text: BTreeMap::new(),
        }
    }

    pub fn get_segment_sender(&self) -> mpsc::Sender<SpeechSegment> {
        self.segment_tx.clone()
    }

    pub fn collect_pending_results(&mut self) {
        while let Ok((index, result)) = self.result_rx.try_recv() {
            match result {
                Ok(text) => {
                    if !text.is_empty() {
                        debug!("Streaming transcription: segment {} = '{}'", index, text);
                        self.segments_text.insert(index, text);
                    }
                }
                Err(e) => {
                    warn!("Streaming transcription: segment {} failed: {}", index, e);
                }
            }
        }
    }

    pub fn finish(mut self) -> String {
        drop(self.segment_tx);

        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }

        while let Ok((index, result)) = self.result_rx.try_recv() {
            if let Ok(text) = result {
                if !text.is_empty() {
                    self.segments_text.insert(index, text);
                }
            }
        }

        let combined: Vec<&str> = self.segments_text.values().map(|s| s.as_str()).collect();
        combined.join(" ")
    }

    pub fn segment_count(&self) -> usize {
        self.segments_text.len()
    }
}

/* ──────────────────────────────────────────────────────────────── */

#[derive(Clone, Debug)]
pub enum RecordingState {
    Idle,
    Recording { binding_id: String },
    Paused { binding_id: String },
}

#[derive(Clone, Debug)]
pub enum MicrophoneMode {
    AlwaysOn,
    OnDemand,
}

/* ──────────────────────────────────────────────────────────────── */

fn create_audio_recorder(
    vad_path: &str,
    app_handle: &tauri::AppHandle,
) -> Result<AudioRecorder, anyhow::Error> {
    let silero = SileroVad::new(vad_path, 0.3)
        .map_err(|e| anyhow::anyhow!("Failed to create SileroVad: {}", e))?;
    let smoothed_vad = SmoothedVad::new(Box::new(silero), 15, 15, 2);

    // Recorder with VAD plus a spectrum-level callback that forwards updates to
    // the frontend.
    let recorder = AudioRecorder::new()
        .map_err(|e| anyhow::anyhow!("Failed to create AudioRecorder: {}", e))?
        .with_vad(Box::new(smoothed_vad))
        .with_level_callback({
            let app_handle = app_handle.clone();
            move |levels| {
                utils::emit_levels(&app_handle, &levels);
            }
        });

    Ok(recorder)
}

/* ──────────────────────────────────────────────────────────────── */

#[derive(Clone)]
pub struct AudioRecordingManager {
    state: Arc<Mutex<RecordingState>>,
    mode: Arc<Mutex<MicrophoneMode>>,
    app_handle: tauri::AppHandle,

    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    is_open: Arc<Mutex<bool>>,
    is_recording: Arc<Mutex<bool>>,
    is_paused: Arc<Mutex<bool>>,
    did_mute: Arc<Mutex<bool>>,
    /// Buffer to store samples recorded before pause
    paused_samples: Arc<Mutex<Vec<f32>>>,
    /// Stores text selected by the user when the "Ramble to Coherent" action starts.
    /// This context is passed to the LLM to allow for "refining" existing text.
    selection_context: Arc<Mutex<Option<String>>>,
    /// When true, the current recording will be processed through LLM refinement on stop.
    /// Set by quick-press (toggle mode) to enable coherent mode for unified hotkey UX.
    coherent_mode: Arc<Mutex<bool>>,
    /// Stores the Base64 representation of screenshots captured during the session.
    vision_context: Arc<Mutex<Vec<String>>>,
    /// Active streaming transcription session (transcribes segments while recording)
    streaming_session: Arc<Mutex<Option<StreamingTranscriptionSession>>>,
}

impl AudioRecordingManager {
    /* ---------- construction ------------------------------------------------ */

    pub fn new(app: &tauri::AppHandle) -> Result<Self, anyhow::Error> {
        let settings = get_settings(app);
        let mode = if settings.always_on_microphone {
            MicrophoneMode::AlwaysOn
        } else {
            MicrophoneMode::OnDemand
        };

        let manager = Self {
            state: Arc::new(Mutex::new(RecordingState::Idle)),
            mode: Arc::new(Mutex::new(mode.clone())),
            app_handle: app.clone(),

            recorder: Arc::new(Mutex::new(None)),
            is_open: Arc::new(Mutex::new(false)),
            is_recording: Arc::new(Mutex::new(false)),
            is_paused: Arc::new(Mutex::new(false)),
            did_mute: Arc::new(Mutex::new(false)),
            paused_samples: Arc::new(Mutex::new(Vec::new())),
            selection_context: Arc::new(Mutex::new(None)),
            coherent_mode: Arc::new(Mutex::new(false)),
            vision_context: Arc::new(Mutex::new(Vec::new())),
            streaming_session: Arc::new(Mutex::new(None)),
        };

        // Always-on?  Open immediately.
        if matches!(mode, MicrophoneMode::AlwaysOn) {
            manager.start_microphone_stream()?;
        }

        Ok(manager)
    }

    /* ---------- helper methods --------------------------------------------- */

    fn get_effective_microphone_device(&self, settings: &AppSettings) -> Option<cpal::Device> {
        // Check if we're in clamshell mode and have a clamshell microphone configured
        let use_clamshell_mic = if let Ok(is_clamshell) = clamshell::is_clamshell() {
            is_clamshell && settings.clamshell_microphone.is_some()
        } else {
            false
        };

        let device_name = if use_clamshell_mic {
            settings.clamshell_microphone.as_ref().unwrap()
        } else {
            settings.selected_microphone.as_ref()?
        };

        // Find the device by name
        match list_input_devices() {
            Ok(devices) => devices
                .into_iter()
                .find(|d| d.name == *device_name)
                .map(|d| d.device),
            Err(e) => {
                debug!("Failed to list devices, using default: {}", e);
                None
            }
        }
    }

    /* ---------- microphone life-cycle -------------------------------------- */

    /// Applies mute if mute_while_recording is enabled and stream is open
    pub fn apply_mute(&self) {
        let settings = get_settings(&self.app_handle);
        let mut did_mute_guard = self.did_mute.lock().unwrap();

        if settings.mute_while_recording && *self.is_open.lock().unwrap() {
            set_mute(true);
            *did_mute_guard = true;
            debug!("Mute applied");
        }
    }

    /// Removes mute if it was applied
    pub fn remove_mute(&self) {
        let mut did_mute_guard = self.did_mute.lock().unwrap();
        if *did_mute_guard {
            set_mute(false);
            *did_mute_guard = false;
            debug!("Mute removed");
        }
    }

    pub fn start_microphone_stream(&self) -> Result<(), anyhow::Error> {
        let mut open_flag = self.is_open.lock().unwrap();
        if *open_flag {
            debug!("Microphone stream already active");
            return Ok(());
        }

        let start_time = Instant::now();

        // Don't mute immediately - caller will handle muting after audio feedback
        let mut did_mute_guard = self.did_mute.lock().unwrap();
        *did_mute_guard = false;

        let vad_path = self
            .app_handle
            .path()
            .resolve(
                "resources/models/silero_vad_v4.onnx",
                tauri::path::BaseDirectory::Resource,
            )
            .map_err(|e| anyhow::anyhow!("Failed to resolve VAD path: {}", e))?;
        let mut recorder_opt = self.recorder.lock().unwrap();

        if recorder_opt.is_none() {
            *recorder_opt = Some(create_audio_recorder(
                vad_path.to_str().unwrap(),
                &self.app_handle,
            )?);
        }

        // Get the selected device from settings, considering clamshell mode
        let settings = get_settings(&self.app_handle);
        let selected_device = self.get_effective_microphone_device(&settings);

        if let Some(rec) = recorder_opt.as_mut() {
            rec.open(selected_device)
                .map_err(|e| anyhow::anyhow!("Failed to open recorder: {}", e))?;
        }

        *open_flag = true;
        info!(
            "Microphone stream initialized in {:?}",
            start_time.elapsed()
        );
        Ok(())
    }

    pub fn stop_microphone_stream(&self) {
        let mut open_flag = self.is_open.lock().unwrap();
        if !*open_flag {
            return;
        }

        let mut did_mute_guard = self.did_mute.lock().unwrap();
        if *did_mute_guard {
            set_mute(false);
        }
        *did_mute_guard = false;

        if let Some(rec) = self.recorder.lock().unwrap().as_mut() {
            // If still recording, stop first.
            if *self.is_recording.lock().unwrap() {
                let _ = rec.stop();
                *self.is_recording.lock().unwrap() = false;
            }
            let _ = rec.close();
        }

        *open_flag = false;
        debug!("Microphone stream stopped");
    }

    /* ---------- mode switching --------------------------------------------- */

    pub fn update_mode(&self, new_mode: MicrophoneMode) -> Result<(), anyhow::Error> {
        let mode_guard = self.mode.lock().unwrap();
        let cur_mode = mode_guard.clone();

        match (cur_mode, &new_mode) {
            (MicrophoneMode::AlwaysOn, MicrophoneMode::OnDemand) => {
                if matches!(*self.state.lock().unwrap(), RecordingState::Idle) {
                    drop(mode_guard);
                    self.stop_microphone_stream();
                }
            }
            (MicrophoneMode::OnDemand, MicrophoneMode::AlwaysOn) => {
                drop(mode_guard);
                self.start_microphone_stream()?;
            }
            _ => {}
        }

        *self.mode.lock().unwrap() = new_mode;
        Ok(())
    }

    /* ---------- recording --------------------------------------------------- */

    pub fn try_start_recording(&self, binding_id: &str) -> bool {
        let max_retries = 10;
        let retry_delay = Duration::from_millis(100);

        for attempt in 0..max_retries {
            let mut state = self.state.lock().unwrap();

            debug!(
                "[AUDIO] try_start_recording (attempt {}/{}) called for binding '{}', current state: {:?}",
                attempt + 1,
                max_retries,
                binding_id,
                *state
            );

            if let RecordingState::Idle = *state {
                // Clear any leftover paused samples from previous session
                self.paused_samples.lock().unwrap().clear();
                // Reset coherent mode for new session
                *self.coherent_mode.lock().unwrap() = false;
                // Clear any previous selection context
                *self.selection_context.lock().unwrap() = None;
                // Clear any previous vision context
                self.vision_context.lock().unwrap().clear();

                // Ensure microphone is open in on-demand mode
                if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                    if let Err(e) = self.start_microphone_stream() {
                        error!("Failed to open microphone stream: {e}");
                        return false;
                    }
                }

                if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                    if rec.start().is_ok() {
                        *self.is_recording.lock().unwrap() = true;
                        *state = RecordingState::Recording {
                            binding_id: binding_id.to_string(),
                        };
                        debug!("[AUDIO] Recording started successfully for binding {binding_id}");
                        return true;
                    }
                }
                error!("[AUDIO] Recorder not available");
                return false;
            } else {
                debug!(
                    "[AUDIO] Cannot start recording - not in Idle state (current: {:?}). Waiting...", 
                    *state
                );
                drop(state); // Drop the lock before sleeping!
                if attempt < max_retries - 1 {
                    thread::sleep(retry_delay);
                }
            }
        }

        error!(
            "[AUDIO] Failed to start recording after {} retries ({}ms total wait). State was never Idle.",
            max_retries,
            max_retries * retry_delay.as_millis()
        );
        false
    }

    pub fn update_selected_device(&self) -> Result<(), anyhow::Error> {
        // If currently open, restart the microphone stream to use the new device
        if *self.is_open.lock().unwrap() {
            self.stop_microphone_stream();
            self.start_microphone_stream()?;
        }
        Ok(())
    }

    pub fn stop_recording(&self, binding_id: &str) -> Option<Vec<f32>> {
        let mut state = self.state.lock().unwrap();

        match *state {
            RecordingState::Recording {
                binding_id: ref active,
            } if active == binding_id => {
                debug!(
                    "[AUDIO-BUG] stop_recording: Matched Recording state for binding '{}'",
                    binding_id
                );
                *state = RecordingState::Idle;
                drop(state);

                // Get current samples from recorder
                let current_samples = if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                    match rec.stop() {
                        Ok(result) => result.raw_full,
                        Err(e) => {
                            error!("stop() failed: {e}");
                            Vec::new()
                        }
                    }
                } else {
                    error!("Recorder not available");
                    Vec::new()
                };

                // Prepend any samples from before pause
                let mut paused = self.paused_samples.lock().unwrap();
                let samples = if paused.is_empty() {
                    current_samples
                } else {
                    debug!(
                        "Prepending {} paused samples to {} current samples",
                        paused.len(),
                        current_samples.len()
                    );
                    let mut combined = std::mem::take(&mut *paused);
                    combined.extend(current_samples);
                    combined
                };

                *self.is_recording.lock().unwrap() = false;

                // In on-demand mode turn the mic off again
                if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                    self.stop_microphone_stream();
                }

                // Pad if very short
                let s_len = samples.len();
                // debug!("Got {} samples", s_len);
                if s_len < WHISPER_SAMPLE_RATE && s_len > 0 {
                    let mut padded = samples;
                    padded.resize(WHISPER_SAMPLE_RATE * 5 / 4, 0.0);
                    Some(padded)
                } else {
                    Some(samples)
                }
            }
            _ => None,
        }
    }
    pub fn is_recording(&self) -> bool {
        matches!(
            *self.state.lock().unwrap(),
            RecordingState::Recording { .. }
        )
    }

    /// Pause any ongoing recording, preserving samples recorded so far
    /// Returns the binding_id if pausing was successful
    pub fn pause_recording(&self) -> Option<String> {
        let mut state = self.state.lock().unwrap();

        if let RecordingState::Recording { binding_id } = state.clone() {
            // Stop capturing and save samples to the buffer
            if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                match rec.stop() {
                    Ok(result) => {
                        // Append to paused samples buffer
                        let mut paused = self.paused_samples.lock().unwrap();
                        debug!(
                            "Pausing: saving {} samples (had {} previously)",
                            result.raw_full.len(),
                            paused.len()
                        );
                        paused.extend(result.raw_full);
                    }
                    Err(e) => {
                        error!("Failed to stop recorder during pause: {e}");
                    }
                }
            }

            *self.is_recording.lock().unwrap() = false;
            *self.is_paused.lock().unwrap() = true;
            *state = RecordingState::Paused {
                binding_id: binding_id.clone(),
            };
            debug!("Recording paused for binding {binding_id}");
            Some(binding_id)
        } else {
            None
        }
    }

    /// Resume a paused recording
    /// Returns the binding_id if resuming was successful
    pub fn resume_recording(&self) -> Option<String> {
        let mut state = self.state.lock().unwrap();

        if let RecordingState::Paused { binding_id } = state.clone() {
            // Start recording again
            if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                if rec.start().is_ok() {
                    *self.is_recording.lock().unwrap() = true;
                    *self.is_paused.lock().unwrap() = false;
                    *state = RecordingState::Recording {
                        binding_id: binding_id.clone(),
                    };
                    debug!("Recording resumed for binding {binding_id}");
                    return Some(binding_id);
                }
            }
            error!("Failed to resume recording");
            None
        } else {
            None
        }
    }

    /// Get the binding_id if currently paused
    pub fn get_paused_binding_id(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        if let RecordingState::Paused { binding_id } = &*state {
            Some(binding_id.clone())
        } else {
            None
        }
    }

    /// Cancel any ongoing recording without returning audio samples
    pub fn cancel_recording(&self) {
        let mut state = self.state.lock().unwrap();

        match *state {
            RecordingState::Recording { .. } | RecordingState::Paused { .. } => {
                *state = RecordingState::Idle;
                drop(state);

                // Stop segment emission and discard streaming session
                if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
                    rec.set_segment_sender(None);
                    let _ = rec.stop(); // Discard the result
                }
                let _ = self.streaming_session.lock().unwrap().take();

                // Clear the paused samples buffer
                self.paused_samples.lock().unwrap().clear();

                *self.is_recording.lock().unwrap() = false;
                *self.is_paused.lock().unwrap() = false;

                // In on-demand mode turn the mic off again
                if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                    self.stop_microphone_stream();
                }
            }
            _ => {}
        }
    }

    /// Sets the selection context for the current recording session.
    pub fn set_selection_context(&self, text: String) {
        *self.selection_context.lock().unwrap() = Some(text);
    }

    /// Clears the selection context.
    pub fn clear_selection_context(&self) {
        *self.selection_context.lock().unwrap() = None;
    }

    /// Retrieves the selection context, if any.
    pub fn get_selection_context(&self) -> Option<String> {
        self.selection_context.lock().unwrap().clone()
    }

    /// Sets coherent mode for the current recording session.
    /// When true, stop will process through LLM refinement.
    pub fn set_coherent_mode(&self, enabled: bool) {
        *self.coherent_mode.lock().unwrap() = enabled;
    }

    /// Gets whether coherent mode is enabled for the current session.
    pub fn get_coherent_mode(&self) -> bool {
        *self.coherent_mode.lock().unwrap()
    }

    /// Sets the vision context for the current recording session.
    /// Adds a vision context (screenshot) for the current recording session.
    pub fn add_vision_context(&self, base64_image: String) {
        debug!(
            "Adding vision context (image size: {} chars)",
            base64_image.len()
        );
        self.vision_context.lock().unwrap().push(base64_image);
    }

    /// Retrieves the vision context (list of images), if any.
    pub fn get_vision_context(&self) -> Vec<String> {
        let ctx = self.vision_context.lock().unwrap().clone();
        debug!("Retrieved vision context ({} images)", ctx.len());
        ctx
    }

    /// Starts a streaming transcription session that will transcribe audio segments
    /// as they are detected during recording.
    pub fn start_streaming_transcription(&self, transcription_manager: Arc<TranscriptionManager>) {
        let session = StreamingTranscriptionSession::new(transcription_manager);
        let segment_sender = session.get_segment_sender();

        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            rec.set_segment_sender(Some(segment_sender));
        }

        *self.streaming_session.lock().unwrap() = Some(session);
        debug!("Streaming transcription session started");
    }

    /// Stops the streaming transcription session and returns the accumulated transcription.
    /// This should be called after stop_recording() to get the pre-transcribed text.
    pub fn finish_streaming_transcription(&self) -> Option<String> {
        if let Some(rec) = self.recorder.lock().unwrap().as_ref() {
            rec.set_segment_sender(None);
        }

        let session = self.streaming_session.lock().unwrap().take();
        if let Some(session) = session {
            let text = session.finish();
            debug!(
                "Streaming transcription session finished: {} chars",
                text.len()
            );
            Some(text)
        } else {
            None
        }
    }

    /// Returns true if there's an active streaming transcription session
    pub fn has_streaming_session(&self) -> bool {
        self.streaming_session.lock().unwrap().is_some()
    }
}
