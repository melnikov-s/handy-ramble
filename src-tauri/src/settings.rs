use log::{debug, warn};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use specta::Type;
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub const APPLE_INTELLIGENCE_PROVIDER_ID: &str = "apple_intelligence";
pub const APPLE_INTELLIGENCE_DEFAULT_MODEL_ID: &str = "Apple Intelligence";

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

// Custom deserializer to handle both old numeric format (1-5) and new string format ("trace", "debug", etc.)
impl<'de> Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LogLevelVisitor;

        impl<'de> Visitor<'de> for LogLevelVisitor {
            type Value = LogLevel;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or integer representing log level")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<LogLevel, E> {
                match value.to_lowercase().as_str() {
                    "trace" => Ok(LogLevel::Trace),
                    "debug" => Ok(LogLevel::Debug),
                    "info" => Ok(LogLevel::Info),
                    "warn" => Ok(LogLevel::Warn),
                    "error" => Ok(LogLevel::Error),
                    _ => Err(E::unknown_variant(
                        value,
                        &["trace", "debug", "info", "warn", "error"],
                    )),
                }
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<LogLevel, E> {
                match value {
                    1 => Ok(LogLevel::Trace),
                    2 => Ok(LogLevel::Debug),
                    3 => Ok(LogLevel::Info),
                    4 => Ok(LogLevel::Warn),
                    5 => Ok(LogLevel::Error),
                    _ => Err(E::invalid_value(de::Unexpected::Unsigned(value), &"1-5")),
                }
            }
        }

        deserializer.deserialize_any(LogLevelVisitor)
    }
}

impl From<LogLevel> for tauri_plugin_log::LogLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => tauri_plugin_log::LogLevel::Trace,
            LogLevel::Debug => tauri_plugin_log::LogLevel::Debug,
            LogLevel::Info => tauri_plugin_log::LogLevel::Info,
            LogLevel::Warn => tauri_plugin_log::LogLevel::Warn,
            LogLevel::Error => tauri_plugin_log::LogLevel::Error,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct ShortcutBinding {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default_binding: String,
    pub current_binding: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct LLMPrompt {
    pub id: String,
    pub name: String,
    pub prompt: String,
}

/// Authentication method for LLM providers
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// API key authentication (default)
    #[default]
    ApiKey,
    /// OAuth 2.0 authentication (supported by Google and OpenAI)
    #[serde(rename = "oauth")]
    OAuth,
}

/// Unified LLM provider configuration
/// Supports OpenAI, Anthropic, Gemini, OpenRouter, and custom enterprise proxies
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct LLMProvider {
    /// Unique identifier (UUID string)
    pub id: String,
    /// Display name (e.g., "OpenAI", "OpenRouter", "My Proxy")
    pub name: String,
    /// API base URL (editable for custom endpoints)
    pub base_url: String,
    /// User's API key for this provider
    #[serde(default)]
    pub api_key: String,
    /// Whether this provider supports vision/image inputs
    #[serde(default)]
    pub supports_vision: bool,
    /// Whether this is a user-added custom provider vs preset
    #[serde(default)]
    pub is_custom: bool,
    /// Authentication method (API key or OAuth)
    #[serde(default)]
    pub auth_method: AuthMethod,
    /// Whether this provider supports OAuth authentication
    #[serde(default)]
    pub supports_oauth: bool,
}

/// Model configuration for a specific provider
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct LLMModel {
    /// Unique identifier (UUID string)
    pub id: String,
    /// Provider ID this model belongs to
    pub provider_id: String,
    /// Model identifier sent to API (e.g., "gpt-4o", "anthropic/claude-3-opus")
    pub model_id: String,
    /// User-friendly display name
    pub display_name: String,
    /// Whether this model supports vision/image inputs
    #[serde(default)]
    pub supports_vision: bool,
    /// Whether this model is enabled and should appear in model selectors
    #[serde(default = "default_model_enabled")]
    pub enabled: bool,
}

fn default_model_enabled() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum OverlayPosition {
    None,
    Top,
    Bottom,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ModelUnloadTimeout {
    Never,
    Immediately,
    Min2,
    Min5,
    Min10,
    Min15,
    Hour1,
    Sec5, // Debug mode only
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PasteMethod {
    CtrlV,
    Direct,
    None,
    ShiftInsert,
    CtrlShiftV,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardHandling {
    DontModify,
    CopyToClipboard,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum RecordingRetentionPeriod {
    Never,
    PreserveLimit,
    Days3,
    Weeks2,
    Months3,
}

/// Prompt mode selection - Dynamic auto-detects based on app, others are explicit processing levels
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum PromptMode {
    #[default]
    Dynamic,
    /// Low processing: minimal intervention, just grammar and filler removal
    Low,
    /// Medium processing: standard polish, formatting, and structure
    Medium,
    /// High processing: intent extraction, aggressive restructuring
    High,
}

// Custom deserialization to handle migration from old category names
impl<'de> serde::Deserialize<'de> for PromptMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "dynamic" => PromptMode::Dynamic,
            "low" => PromptMode::Low,
            "medium" => PromptMode::Medium,
            "high" => PromptMode::High,
            // Migration: old category names → new processing levels
            "development" => PromptMode::Medium,
            "conversation" => PromptMode::Low,
            "writing" => PromptMode::High,
            "email" => PromptMode::Medium,
            _ => PromptMode::Dynamic, // Fallback for any unknown values
        })
    }
}

impl PromptMode {
    /// Get the icon for this mode (used in overlay and tray menu)
    pub fn icon(&self) -> &'static str {
        match self {
            PromptMode::Dynamic => "",
            PromptMode::Low => "▁",
            PromptMode::Medium => "▃",
            PromptMode::High => "▅",
        }
    }

    /// Get the display name for this mode
    pub fn display_name(&self) -> &'static str {
        match self {
            PromptMode::Dynamic => "Dynamic",
            PromptMode::Low => "Low",
            PromptMode::Medium => "Medium",
            PromptMode::High => "High",
        }
    }

    /// Get the category ID for this mode (used for prompt lookup)
    pub fn category_id(&self) -> Option<&'static str> {
        match self {
            PromptMode::Dynamic => None, // Will be determined by app detection
            PromptMode::Low => Some("low"),
            PromptMode::Medium => Some("medium"),
            PromptMode::High => Some("high"),
        }
    }
}

/// A prompt category that groups applications and defines processing style
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PromptCategory {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub prompt: String,
    pub is_builtin: bool,
    /// Optional model override for this category (None = use default coherent model)
    #[serde(default)]
    pub model_override: Option<String>,
}

/// Maps an application to a category
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AppCategoryMapping {
    pub bundle_identifier: String,
    pub display_name: String,
    pub category_id: String,
}

/// Detected app info (for tracking history)
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct DetectedApp {
    pub bundle_identifier: String,
    pub display_name: String,
    pub last_seen: u64,
}

/// Type of voice command
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum VoiceCommandType {
    /// Built-in command with native handler (web_search, open_app, print, etc.)
    #[default]
    Builtin,
    /// User-defined script (shell or AppleScript)
    #[serde(alias = "bespoke")]
    Custom,
    /// Legacy: LLM-inferred command (treated as Builtin)
    #[serde(alias = "inferable")]
    #[serde(skip_serializing)]
    LegacyInferable,
}

/// Script type for bespoke commands
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScriptType {
    #[default]
    Shell,
    AppleScript,
}

/// A voice command definition
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct VoiceCommand {
    /// Unique identifier for the command
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Trigger phrases that activate this command
    pub phrases: Vec<String>,
    /// Type of command (inferable or bespoke)
    pub command_type: VoiceCommandType,
    /// Description for LLM (inferable commands)
    #[serde(default)]
    pub description: Option<String>,
    /// Script type (bespoke commands)
    #[serde(default)]
    pub script_type: ScriptType,
    /// Script content (bespoke commands)
    #[serde(default)]
    pub script: Option<String>,
    /// Model override (uses default if None)
    #[serde(default)]
    pub model_override: Option<String>,
    /// Whether this is a built-in command
    #[serde(default)]
    pub is_builtin: bool,
}

impl Default for ModelUnloadTimeout {
    fn default() -> Self {
        ModelUnloadTimeout::Never
    }
}

impl Default for PasteMethod {
    fn default() -> Self {
        // Default to CtrlV for macOS and Windows, Direct for Linux
        #[cfg(target_os = "linux")]
        return PasteMethod::Direct;
        #[cfg(not(target_os = "linux"))]
        return PasteMethod::CtrlV;
    }
}

impl Default for ClipboardHandling {
    fn default() -> Self {
        ClipboardHandling::DontModify
    }
}

impl ModelUnloadTimeout {
    pub fn to_minutes(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Min2 => Some(2),
            ModelUnloadTimeout::Min5 => Some(5),
            ModelUnloadTimeout::Min10 => Some(10),
            ModelUnloadTimeout::Min15 => Some(15),
            ModelUnloadTimeout::Hour1 => Some(60),
            ModelUnloadTimeout::Sec5 => Some(0), // Special case for debug - handled separately
        }
    }

    pub fn to_seconds(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Sec5 => Some(5),
            _ => self.to_minutes().map(|m| m * 60),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum SoundTheme {
    Marimba,
    Pop,
    Custom,
}

impl SoundTheme {
    fn as_str(&self) -> &'static str {
        match self {
            SoundTheme::Marimba => "marimba",
            SoundTheme::Pop => "pop",
            SoundTheme::Custom => "custom",
        }
    }

    pub fn to_start_path(&self) -> String {
        format!("resources/{}_start.wav", self.as_str())
    }

    pub fn to_stop_path(&self) -> String {
        format!("resources/{}_stop.wav", self.as_str())
    }
}

/* still handy for composing the initial JSON in the store ------------- */
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AppSettings {
    pub bindings: HashMap<String, ShortcutBinding>,
    pub push_to_talk: bool,
    pub audio_feedback: bool,
    #[serde(default = "default_audio_feedback_volume")]
    pub audio_feedback_volume: f32,
    #[serde(default = "default_sound_theme")]
    pub sound_theme: SoundTheme,
    #[serde(default = "default_start_hidden")]
    pub start_hidden: bool,
    #[serde(default = "default_autostart_enabled")]
    pub autostart_enabled: bool,
    #[serde(default = "default_update_checks_enabled")]
    pub update_checks_enabled: bool,
    #[serde(default = "default_model")]
    pub selected_model: String,
    #[serde(default = "default_always_on_microphone")]
    pub always_on_microphone: bool,
    #[serde(default)]
    pub selected_microphone: Option<String>,
    #[serde(default)]
    pub clamshell_microphone: Option<String>,
    #[serde(default)]
    pub selected_output_device: Option<String>,
    #[serde(default = "default_translate_to_english")]
    pub translate_to_english: bool,
    #[serde(default = "default_selected_language")]
    pub selected_language: String,
    #[serde(default = "default_overlay_position")]
    pub overlay_position: OverlayPosition,
    #[serde(default = "default_debug_mode")]
    pub debug_mode: bool,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default)]
    pub custom_words: Vec<String>,
    #[serde(default)]
    pub model_unload_timeout: ModelUnloadTimeout,
    #[serde(default = "default_word_correction_threshold")]
    pub word_correction_threshold: f64,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default = "default_recording_retention_period")]
    pub recording_retention_period: RecordingRetentionPeriod,

    // === Unified LLM Provider Configuration ===
    /// All configured LLM providers (OpenAI, Anthropic, OpenRouter, custom)
    #[serde(default = "default_llm_providers")]
    pub llm_providers: Vec<LLMProvider>,
    /// All configured models across all providers
    #[serde(default = "default_llm_models")]
    pub llm_models: Vec<LLMModel>,
    /// Default model ID for chat windows
    #[serde(default)]
    pub default_chat_model_id: Option<String>,
    /// Default model ID for coherent/ramble mode
    #[serde(default)]
    pub default_coherent_model_id: Option<String>,
    /// Default model ID for voice commands
    #[serde(default)]
    pub default_voice_model_id: Option<String>,

    // === Other settings ===
    #[serde(default)]
    pub paste_method: PasteMethod,
    #[serde(default)]
    pub clipboard_handling: ClipboardHandling,
    /// Prompts for coherent mode (transforms rambling speech to clean text)
    #[serde(default = "default_coherent_prompts")]
    pub coherent_prompts: Vec<LLMPrompt>,
    #[serde(default)]
    pub coherent_selected_prompt_id: Option<String>,
    #[serde(default)]
    pub mute_while_recording: bool,
    #[serde(default)]
    pub append_trailing_space: bool,
    #[serde(default = "default_app_language")]
    pub app_language: String,
    /// Whether coherent mode (LLM refinement) is enabled
    #[serde(default = "default_coherent_enabled")]
    pub coherent_enabled: bool,
    /// Whether to use vision model when screenshots are available
    #[serde(default)]
    pub coherent_use_vision: bool,
    /// Threshold in milliseconds for tap vs hold detection (smart PTT)
    #[serde(default = "default_hold_threshold_ms")]
    pub hold_threshold_ms: u64,
    // App-aware prompt settings
    /// Current prompt mode (Dynamic, Low, Medium, High)
    #[serde(default)]
    pub prompt_mode: PromptMode,
    /// Prompt categories (built-in + user-defined)
    #[serde(default = "default_prompt_categories")]
    pub prompt_categories: Vec<PromptCategory>,
    /// Application to category mappings
    #[serde(default)]
    pub app_category_mappings: Vec<AppCategoryMapping>,
    /// History of detected applications (for dropdown suggestions)
    #[serde(default)]
    pub detected_apps_history: Vec<DetectedApp>,
    /// Default category for apps not in known_apps or user mappings
    #[serde(default = "default_category_id")]
    pub default_category_id: String,
    // Voice command settings
    /// Whether voice commands are enabled
    #[serde(default)]
    pub voice_commands_enabled: bool,
    /// Default model for voice command execution
    #[serde(default = "default_voice_command_model")]
    pub voice_command_default_model: String,
    /// User-defined voice commands
    #[serde(default = "default_voice_commands")]
    pub voice_commands: Vec<VoiceCommand>,
    // TTS Settings
    #[serde(default = "default_tts_enabled")]
    pub tts_enabled: bool,
    #[serde(default)]
    pub tts_selected_model: Option<String>,
    #[serde(default = "default_tts_speed")]
    pub tts_speed: f32,
    #[serde(default = "default_tts_volume")]
    pub tts_volume: f32,
    #[serde(default)]
    pub filler_word_filter: Option<String>,
    /// Whether to collapse repeated words (e.g., "I I I am" → "I am")
    #[serde(default = "default_collapse_repeated_words")]
    pub collapse_repeated_words: bool,
    /// Customizable initial prompt for the quick chat
    #[serde(default = "default_quick_chat_initial_prompt")]
    pub quick_chat_initial_prompt: String,
    // Unknown command agent settings
    /// Whether to launch CLI agent for unknown commands (instead of showing error)
    #[serde(default)]
    pub unknown_command_agent_enabled: bool,
    /// Template for the CLI command to run. Supports ${prompt} placeholder.
    /// Example: `claude -p "${prompt}"` or `gemini "${prompt}"`
    #[serde(default = "default_unknown_command_template")]
    pub unknown_command_template: String,
    /// Terminal application to use (iTerm, Terminal, Warp)
    #[serde(default = "default_unknown_command_terminal")]
    pub unknown_command_terminal: String,
    /// Maximum characters to include from clipboard content in ${clipboard} variable
    /// 0 = no cutoff (include all), other values = limit to N characters
    #[serde(default)]
    pub clipboard_content_cutoff: u32,
    /// Prompt for the context chat mode
    #[serde(default = "default_context_chat_prompt")]
    pub context_chat_prompt: String,
    /// The last response from a voice interaction (Context Chat)
    #[serde(default)]
    pub last_voice_interaction: Option<String>,
    /// Default model ID for context chat mode
    #[serde(default)]
    pub default_context_chat_model_id: Option<String>,
    /// Path to a system prompt file that will be injected into all LLM calls
    #[serde(default)]
    pub system_prompt_file: Option<String>,
}

fn default_model() -> String {
    "".to_string()
}

fn default_always_on_microphone() -> bool {
    false
}

fn default_translate_to_english() -> bool {
    false
}

fn default_start_hidden() -> bool {
    false
}

fn default_autostart_enabled() -> bool {
    false
}

fn default_update_checks_enabled() -> bool {
    true
}

fn default_selected_language() -> String {
    "auto".to_string()
}

fn default_overlay_position() -> OverlayPosition {
    #[cfg(target_os = "linux")]
    return OverlayPosition::None;
    #[cfg(not(target_os = "linux"))]
    return OverlayPosition::Bottom;
}

fn default_debug_mode() -> bool {
    false
}

fn default_log_level() -> LogLevel {
    LogLevel::Debug
}

fn default_word_correction_threshold() -> f64 {
    0.18
}

fn default_history_limit() -> usize {
    5
}

fn default_recording_retention_period() -> RecordingRetentionPeriod {
    RecordingRetentionPeriod::PreserveLimit
}

fn default_audio_feedback_volume() -> f32 {
    1.0
}

fn default_sound_theme() -> SoundTheme {
    SoundTheme::Marimba
}

fn default_tts_enabled() -> bool {
    true
}

fn default_tts_speed() -> f32 {
    1.0
}

fn default_tts_volume() -> f32 {
    1.0
}

fn default_post_process_enabled() -> bool {
    false
}

fn default_app_language() -> String {
    tauri_plugin_os::locale()
        .and_then(|l| l.split(['-', '_']).next().map(String::from))
        .unwrap_or_else(|| "en".to_string())
}

fn default_llm_provider_id() -> String {
    "gemini".to_string()
}

fn default_post_process_provider_id() -> String {
    "openai".to_string()
}

fn default_ramble_enabled() -> bool {
    false
}

fn default_ramble_provider_id() -> String {
    "gemini".to_string()
}

fn default_ramble_model() -> String {
    "gemini-2-0-flash-lite".to_string()
}

fn default_ramble_use_vision_model() -> bool {
    false
}

fn default_ramble_vision_model() -> String {
    "gemini-2.0-flash".to_string()
}

fn default_ramble_prompt() -> String {
    "You are transforming rambling speech into clean, well-structured text.

The input is unfiltered speech-to-text. Your job is to make it readable while preserving all meaning.

IMPORTANT: You are the user's proxy. Speak AS the user, not TO the user. Formulate the response as if the user is typing it. Preserve the user's perspective: do not change pronouns or perspective. If the user addresses \"you\", keep it as \"you\".

---

INLINE INSTRUCTIONS - The speaker may give you direct commands during dictation:

Explicit commands (always obey these):
- \"Hey refiner, ...\" or \"Refiner: ...\" signals a direct instruction to you
- Example: \"Hey refiner, ignore the last sentence\" → delete the preceding sentence
- Example: \"Refiner: expand on that idea\" → elaborate on the previous point

Natural correction patterns (interpret these as editing commands, not content):
- \"scratch that\", \"delete that\", \"never mind\" → remove the immediately preceding content
- \"ignore the last [X seconds/sentence/paragraph]\" → remove that content
- \"go back and [change/fix/remove] ...\" → apply the edit retroactively
- \"actually, make that ...\" → replace the previous statement with the correction
- \"fill in the details here\", \"expand on this\" → elaborate on the topic
- \"placeholder for [X]\" → insert a clear [TODO: X] marker

These instructions are commands TO YOU—they should NOT appear in the output.
When in doubt about whether something is an instruction vs. content, prefer treating it as an instruction if it clearly references editing the transcription itself.

---

ACTIVELY DO:
1. Remove filler words (um, uh, like, you know, basically, so, I mean)
2. Fix run-on sentences—break them into clear, punctuated sentences
3. Remove verbal repetition and redundancy
4. Restructure for clarity and readability
5. When the speaker corrects themselves, keep only the final version

CODE DICTATION - Convert spoken code to actual syntax:
- \"camel case foo bar\" → fooBar
- \"pascal case foo bar\" → FooBar
- \"snake case foo bar\" → foo_bar
- \"open paren\", \"close bracket\" → (, ]
- Natural descriptions like \"if A greater than B\" → if (a > b)

FORMATTING - Use markdown for readability:
- Break up large paragraphs into shorter ones
- Use bullet points for lists of items or requirements
- Use numbered lists for sequential steps or instructions
- Use line breaks between distinct topics or ideas
- Use code blocks or backticks for code/technical terms when appropriate
The output should be easy to scan and reference later.

PRESERVE THE MEANING OF (but rewrite for clarity):
- Instructions and directives (first do X, start by checking Y)
- Context and reasoning (why something matters)
- Specific examples given
- Sequence of steps or operations

The output should be noticeably cleaner and more readable than the input while conveying the same information.

Return ONLY the cleaned, formatted text. No preamble.

---

<selection>
${selection}
</selection>

<transcript>
${output}
</transcript>".to_string()
}

fn default_hold_threshold_ms() -> u64 {
    500 // 500ms feels more natural - fast enough for PTT, slow enough for accidental taps
}

fn default_category_id() -> String {
    "medium".to_string()
}

fn default_voice_command_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_filler_word_filter() -> Option<String> {
    // Default pattern to filter common English filler words
    // Matches: um, uh, hmm, mhm, mm, ah, er, erm (with variations like umm, uhh, etc.)
    Some(r"\b(u+[hm]+|a+h+|e+r+m?|m+h?m+|h+m+)\b[,\s]*".to_string())
}

fn default_collapse_repeated_words() -> bool {
    true
}

fn default_quick_chat_initial_prompt() -> String {
    "You are a helpful assistant. You are given some context from the user's screen or selection to help you answer their questions.\n\nCONTEXT FROM USER SELECTION:\n${selection}".to_string()
}

fn default_unknown_command_template() -> String {
    "claude -p \"${prompt}\"".to_string()
}

fn default_unknown_command_terminal() -> String {
    "Terminal".to_string()
}

fn default_context_chat_prompt() -> String {
    "You are a helpful voice assistant. Your response will be read aloud using text-to-speech.

THIS IS A ONE-SHOT COMMAND, NOT A CONVERSATION:
- The user cannot respond or follow up - give a complete, final answer
- Do NOT ask clarifying questions or say things like \"Would you like me to...\"
- Do NOT offer to help with anything else or ask if they need more information
- Just directly answer or perform what was asked, nothing more

FORMATTING FOR SPEECH:
- No markdown, bullet points, lists, or special formatting
- No asterisks, code blocks, or headers
- Write in natural, conversational sentences
- Keep responses concise - aim for 2-4 sentences
- Use punctuation for natural speech rhythm

YOUTUBE VIDEO SUMMARIZATION:
If you see a YouTube screenshot or YouTube link and the user asks to summarize it:
1. For screenshots: look at the URL bar or video title visible in the image
2. For links: extract the video ID from the URL
3. Use web search to find the transcript of that video
4. Summarize based on the transcript content, not just the title
5. Provide a thorough summary with a decent amount of detail - roughly one paragraph per 5-10 minutes of video content
6. Cover the main points, key arguments, and important takeaways
7. This overrides the \"2-4 sentences\" guideline - video summaries should be comprehensive
8. Of course, follow any specific instructions the user gives about length or detail

CONTEXT:
${selection}

USER COMMAND: ${prompt}

Provide a direct, complete answer."
        .to_string()
}

fn default_voice_commands() -> Vec<VoiceCommand> {
    vec![
        VoiceCommand {
            id: "open_app".to_string(),
            name: "Open Application".to_string(),
            phrases: vec![
                "open".to_string(),
                "launch".to_string(),
                "start".to_string(),
            ],
            command_type: VoiceCommandType::Builtin,
            description: Some(
                "Opens an application by name. The user will specify which app to open."
                    .to_string(),
            ),
            script_type: ScriptType::Shell,
            script: None,
            model_override: None,
            is_builtin: true,
        },
        VoiceCommand {
            id: "web_search".to_string(),
            name: "Web Search".to_string(),
            phrases: vec![
                "search for".to_string(),
                "look up".to_string(),
                "google".to_string(),
            ],
            command_type: VoiceCommandType::Builtin,
            description: Some("Opens a web browser with a search query.".to_string()),
            script_type: ScriptType::Shell,
            script: None,
            model_override: None,
            is_builtin: true,
        },
        VoiceCommand {
            id: "refactor_code".to_string(),
            name: "Refactor Code".to_string(),
            phrases: vec![
                "refactor".to_string(),
                "rewrite".to_string(),
                "improve this".to_string(),
            ],
            command_type: VoiceCommandType::Builtin,
            description: Some(
                "Refactors or rewrites the selected code based on the user's instruction."
                    .to_string(),
            ),
            script_type: ScriptType::Shell,
            script: None,
            model_override: Some("gpt-4o".to_string()), // Needs reasoning capability
            is_builtin: true,
        },
        VoiceCommand {
            id: "print".to_string(),
            name: "Print / Echo".to_string(),
            phrases: vec![
                "print".to_string(),
                "echo".to_string(),
                "say".to_string(),
                "type".to_string(),
            ],
            command_type: VoiceCommandType::Builtin,
            description: Some(
                "Echoes back the text that follows the trigger word. Returns the text verbatim without any modifications. For example, 'print hello world' returns 'hello world'."
                    .to_string(),
            ),
            script_type: ScriptType::Shell,
            script: None,
            model_override: None,
            is_builtin: true,
        },
        VoiceCommand {
            id: "lucky_search".to_string(),
            name: "Lucky Search".to_string(),
            phrases: vec![
                "beads".to_string(),
                "beach".to_string(),
                "go to".to_string(),
                "lucky search".to_string(),
                "i'm feeling lucky".to_string(),
            ],
            command_type: VoiceCommandType::Custom,
            description: Some(
                "Searches Google and automatically clicks the first result.".to_string(),
            ),
            script_type: ScriptType::AppleScript,
            script: Some(r#"tell application "Google Chrome"
    activate
    if (count of windows) is 0 then
        make new window
    end if
    set newTab to make new tab at front window with properties {URL:"https://www.google.com/search?q=${transcription}"}
    repeat until (loading of newTab is false)
        delay 0.3
    end repeat
    delay 0.5
    execute newTab javascript "var firstResult = document.querySelector('h3'); if (firstResult) { firstResult.click(); } else { var anchor = document.querySelector('a.zReHs'); if (anchor) anchor.click(); }"
end tell"#.to_string()),
            model_override: None,
            is_builtin: true,
        },
    ]
}

fn default_prompt_categories() -> Vec<PromptCategory> {
    vec![
        PromptCategory {
            id: "low".to_string(),
            name: "Low".to_string(),
            icon: "▁".to_string(),
            is_builtin: true,
            model_override: None,
            prompt: "You are cleaning up speech-to-text for a casual chat message.

**Context:** The user is in ${application} (${category} mode). The output is a message to another human.

IMPORTANT: You are the user's proxy. The message should sound exactly like the user would type it. Preserve the user's perspective: do not change pronouns or perspective. If the user addresses \"you\", keep it as \"you\".

CRITICAL RULES:
1. NEVER remove content unless the user explicitly instructs (\"hey Ramble, delete that\", \"scratch that\", \"never mind\")
2. DO NOT start sentences with capital letters (like mobile autocorrect does) unless it's a proper noun or name
3. NO em dashes (—). Use simple punctuation only: periods, commas, question marks, exclamation points
4. NO formatting: no lists, no bullet points, no bold, no italics, no headers
5. If the user wants formatting, they will say it explicitly (\"bold tomorrow\", \"emphasis on skills\")

YOUR ONLY JOB:
- Add appropriate punctuation where needed
- Fix obvious typos or grammar issues
- Convert spoken punctuation (\"period\", \"comma\", \"question mark\") to symbols
- Keep emoji references if mentioned (\"smiley face\" → 😊, \"thumbs up\" → 👍)

PRESERVE:
- The user's casual speaking style
- All content and meaning, do not condense or summarize
- Humor, sarcasm, and informal language
- Short, punchy message style

INLINE COMMANDS (only these remove content):
- \"hey Ramble, ...\" or \"Ramble: ...\" = direct instruction to you
- \"scratch that\", \"delete that\", \"never mind\" = remove preceding content
- \"actually\" followed by correction = keep only the correction

Return ONLY the cleaned text. No preamble.

---

<selection>
${selection}
</selection>

<transcript>
${output}
</transcript>".to_string(),
        },
        PromptCategory {
            id: "medium".to_string(),
            name: "Medium".to_string(),
            icon: "▃".to_string(),
            is_builtin: true,
            model_override: None,
            prompt: "You are transforming rambling speech into polished written prose.

**Context:** The user is in ${application} (${category} mode). The output is written content for human readers.

IMPORTANT: You are the user's proxy. Write AS the user, not TO the user. Preserve the user's perspective: do not change pronouns or perspective. If the user addresses \"you\", keep it as \"you\".

CRITICAL RULES:
1. NO em dashes (—) ever. Use commas, periods, or restructure the sentence instead.
2. Keep the author's voice while improving clarity
3. Only remove content if the user explicitly instructs (\"scratch that\", \"delete that\", \"hey Ramble, remove...\")

YOUR JOB:
1. Create well-structured paragraphs with proper flow
2. Fix grammar, punctuation, and sentence structure
3. Remove filler words and verbal tics (um, uh, like, you know)
4. Consolidate repeated ideas into single, clear statements
5. Use appropriate paragraph breaks between distinct ideas

FORMATTING:
- Use markdown sparingly: headers, lists when explicitly requested
- If user says \"bullet point\" or \"list this\", use bullets
- If user says \"bold X\" or \"emphasis on X\", apply formatting
- Otherwise, keep it as flowing prose

INLINE COMMANDS:
- \"hey Ramble, ...\" = direct instruction
- \"scratch that\", \"delete that\", \"never mind\" = remove preceding content
- \"actually\" followed by correction = keep only the correction

Return ONLY the cleaned text. No preamble.

---

<selection>
${selection}
</selection>

<transcript>
${output}
</transcript>".to_string(),
        },
        PromptCategory {
            id: "high".to_string(),
            name: "High".to_string(),
            icon: "▅".to_string(),
            is_builtin: true,
            model_override: None,
            prompt: "You are an aggressive editor transforming rambling speech into clean, focused text.

**Context:** The user is in ${application} (${category} mode). The output will be used in developer tools or sent to AI assistants.

The input is unfiltered speech-to-text. Your job is to extract ONLY the relevant, final intent—aggressively pruning everything else.

IMPORTANT: You are the user's proxy. Speak AS the user, not TO the user. Preserve the user's perspective.

---

CRITICAL: RETRACTION AND CORRECTION HANDLING

When the speaker changes their mind, backtracks, or says \"nevermind\", you MUST remove the retracted content entirely. Examples:

INPUT: \"I want to add a button, nevermind that, I want to add a link instead\"
OUTPUT: \"I want to add a link.\"

INPUT: \"Let's meet at 5 PM, actually scratch that, make it 6 PM\"
OUTPUT: \"Let's meet at 6 PM.\"

INPUT: \"We should use React, no wait, actually let's use Vue, hmm, on second thought React is better\"
OUTPUT: \"We should use React.\"

Retraction triggers (ALWAYS remove preceding content when you see these):
- \"nevermind that\", \"never mind\", \"nevermind\"
- \"scratch that\", \"delete that\", \"forget that\"
- \"actually no\", \"wait no\", \"no wait\"
- \"on second thought\", \"let me rethink\"
- \"ignore that\", \"disregard that\"
- \"that's wrong\", \"that's not right\"

---

AGGRESSIVELY REMOVE (do NOT include in output):
1. Filler words: um, uh, like, you know, basically, so, I mean, right, okay
2. Thinking out loud: \"let me think\", \"what was I saying\", \"where was I\"
3. Meta-commentary about the recording: \"is this thing on\", \"let me start over\"
4. Repeated attempts to say the same thing—keep only the clearest version
5. False starts and abandoned sentences
6. Self-corrections—keep ONLY the final corrected version
7. Hedging when the speaker later commits: \"maybe we should... actually yes, let's definitely...\" → keep only the commitment

---

INLINE COMMANDS (these are instructions TO YOU, never include them in output):
- \"Hey Ramble, ...\" or \"Ramble: ...\" = direct instruction
- \"scratch that\", \"delete that\", \"never mind\" = remove preceding content
- \"actually\" followed by correction = keep only the correction
- \"ignore the last [X]\" = remove that content
- \"placeholder for [X]\" → insert [TODO: X]

---

PRESERVE (but rewrite for clarity):
- The speaker's actual intent and requirements
- Technical details, names, numbers
- Sequence of steps when relevant
- Context that matters for understanding

CODE DICTATION:
- \"camel case foo bar\" → fooBar
- \"snake case foo bar\" → foo_bar
- \"open paren\", \"close bracket\" → (, ]

FORMATTING:
- Use markdown: bullet points, numbered lists, code blocks
- Break into short paragraphs
- Make it scannable

The output should be MUCH shorter and cleaner than the input. If you're not removing significant content, you're not being aggressive enough.

Return ONLY the cleaned text. No preamble.

---

<selection>
${selection}
</selection>

<transcript>
${output}
</transcript>".to_string(),
        },
    ]
}

fn default_llm_providers() -> Vec<LLMProvider> {
    let mut providers = vec![
        // API Key providers (original)
        LLMProvider {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            supports_vision: true,
            is_custom: false,
            auth_method: AuthMethod::ApiKey,
            supports_oauth: false,
        },
        LLMProvider {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            api_key: String::new(),
            supports_vision: true,
            is_custom: false,
            auth_method: AuthMethod::ApiKey,
            supports_oauth: false,
        },
        LLMProvider {
            id: "gemini".to_string(),
            name: "Google Gemini".to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            api_key: String::new(),
            supports_vision: true,
            is_custom: false,
            auth_method: AuthMethod::ApiKey,
            supports_oauth: false,
        },
        // OAuth providers (new - separate from API key providers)
        LLMProvider {
            id: "openai_oauth".to_string(),
            name: "OpenAI (OAuth)".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            supports_vision: true,
            is_custom: false,
            auth_method: AuthMethod::OAuth,
            supports_oauth: true,
        },
        LLMProvider {
            id: "gemini_oauth".to_string(),
            name: "Google Gemini (OAuth)".to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            api_key: String::new(),
            supports_vision: true,
            is_custom: false,
            auth_method: AuthMethod::OAuth,
            supports_oauth: true,
        },
    ];

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        if crate::apple_intelligence::check_apple_intelligence_availability() {
            providers.push(LLMProvider {
                id: APPLE_INTELLIGENCE_PROVIDER_ID.to_string(),
                name: "Apple Intelligence".to_string(),
                base_url: "apple-intelligence://local".to_string(),
                api_key: String::new(),
                supports_vision: false,
                is_custom: false,
                auth_method: AuthMethod::ApiKey,
                supports_oauth: false,
            });
        }
    }

    providers
}

fn default_llm_models() -> Vec<LLMModel> {
    // Models are now fetched dynamically from provider APIs
    // Use the "Refresh Models" button in the UI to populate this list
    vec![]
}

fn default_coherent_enabled() -> bool {
    true
}

fn default_coherent_prompts() -> Vec<LLMPrompt> {
    vec![
        LLMPrompt {
            id: "default_improve_transcriptions".to_string(),
            name: "Improve Transcriptions".to_string(),
            prompt: "Clean this transcript:\n1. Fix spelling, capitalization, and punctuation errors\n2. Convert number words to digits (twenty-five → 25, ten percent → 10%, five dollars → $5)\n3. Replace spoken punctuation with symbols (period → ., comma → ,, question mark → ?)\n4. Remove filler words (um, uh, like as filler)\n5. Keep the language in the original version (if it was french, keep it in french for example)\n\nPreserve exact meaning, pronouns, perspective, and word order. Do not paraphrase or reorder content.\n\nReturn only the cleaned transcript.\n\n<transcript>\n${output}\n</transcript>".to_string(),
        },
        LLMPrompt {
            id: "ramble_to_coherent".to_string(),
            name: "Ramble to Coherent".to_string(),
            prompt: "You are transforming raw speech into clean, coherent text.\n\nThe input is unfiltered speech-to-text that contains:\n- Filler words (um, uh, like, you know, basically, so, I mean)\n- Thinking out loud and self-corrections\n- Backtracking (no wait, actually, I mean)\n- Repeated ideas phrased multiple ways\n- Run-on sentences and stream of consciousness\n\nYour task:\n1. Extract the core intent and requirements\n2. Remove ALL filler words and verbal tics\n3. When the speaker changes their mind, keep ONLY the final decision\n4. Consolidate repeated ideas into single clear statements\n5. Structure as clear, actionable points if appropriate\n6. Preserve technical terms and specific details exactly\n7. Keep the same language as input\n8. Preserve the user's perspective: do not change pronouns or perspective. If the user addresses \"you\", keep it as \"you\".\n\nReturn ONLY the cleaned, structured text. No preamble or explanation.\n\n<transcript>\n${output}\n</transcript>".to_string(),
        },
    ]
}

/// Migrate old prompt categories (development/conversation/writing/email) to new system (low/medium/high).
/// Also migrates app_category_mappings and default_category_id.
fn migrate_prompt_categories(settings: &mut AppSettings) -> bool {
    let old_category_ids = ["development", "conversation", "writing", "email"];
    let mut migrated = false;

    // Check if we need to migrate by looking for old category IDs
    let has_old_categories = settings
        .prompt_categories
        .iter()
        .any(|c| old_category_ids.contains(&c.id.as_str()));

    if has_old_categories {
        debug!("Migrating old prompt categories to new Low/Medium/High system");
        // Replace all categories with the new defaults (preserves user's custom categories if any)
        let defaults = default_prompt_categories();
        let mut new_categories = defaults;

        // Keep any user-defined (non-builtin) categories that aren't old ones
        for cat in &settings.prompt_categories {
            if !cat.is_builtin && !old_category_ids.contains(&cat.id.as_str()) {
                new_categories.push(cat.clone());
            }
        }

        settings.prompt_categories = new_categories;
        migrated = true;
    }

    // Migrate app_category_mappings to use new category IDs
    for mapping in &mut settings.app_category_mappings {
        let new_id = match mapping.category_id.as_str() {
            "development" => Some("medium"),
            "conversation" => Some("low"),
            "writing" => Some("high"),
            "email" => Some("medium"),
            _ => None,
        };
        if let Some(new) = new_id {
            debug!(
                "Migrating app mapping {} from {} to {}",
                mapping.display_name, mapping.category_id, new
            );
            mapping.category_id = new.to_string();
            migrated = true;
        }
    }

    // Migrate default_category_id
    let new_default = match settings.default_category_id.as_str() {
        "development" => Some("medium"),
        "conversation" => Some("low"),
        "writing" => Some("high"),
        "email" => Some("medium"),
        _ => None,
    };
    if let Some(new) = new_default {
        debug!(
            "Migrating default_category_id from {} to {}",
            settings.default_category_id, new
        );
        settings.default_category_id = new.to_string();
        migrated = true;
    }

    // Always ensure builtin categories are in correct order (Low, Medium, High)
    // and have the latest prompt content
    let defaults = default_prompt_categories();
    let expected_order = ["low", "medium", "high"];

    // Check if builtin categories need reordering or updating
    let builtin_ids: Vec<&str> = settings
        .prompt_categories
        .iter()
        .filter(|c| c.is_builtin)
        .map(|c| c.id.as_str())
        .collect();

    let needs_reorder = builtin_ids != expected_order;

    if needs_reorder {
        debug!("Reordering prompt categories to Low, Medium, High");
        // Keep user-defined categories
        let user_categories: Vec<_> = settings
            .prompt_categories
            .iter()
            .filter(|c| !c.is_builtin)
            .cloned()
            .collect();

        // Replace with defaults + user categories
        settings.prompt_categories = defaults;
        settings.prompt_categories.extend(user_categories);
        migrated = true;
    }

    migrated
}

/// Previously ensured default providers/models were present.
/// Now disabled - users add providers via the UI dialog.
fn ensure_llm_defaults(_settings: &mut AppSettings) -> bool {
    // No longer auto-populate providers and models
    // Users add them via Settings > AI Providers > Add Provider
    false
}

/// Ensures that all built-in default voice commands are present in settings.
/// This adds new built-in commands without overwriting user-defined commands.
fn ensure_voice_command_defaults(settings: &mut AppSettings) -> bool {
    let mut changed = false;
    for default_cmd in default_voice_commands() {
        if default_cmd.is_builtin {
            // Check if this built-in command already exists
            let exists = settings
                .voice_commands
                .iter()
                .any(|c| c.id == default_cmd.id);
            if !exists {
                log::debug!("Adding missing built-in voice command: {}", default_cmd.id);
                settings.voice_commands.push(default_cmd);
                changed = true;
            }
        }
    }
    changed
}

pub const SETTINGS_STORE_PATH: &str = "settings_store.json";

pub fn get_default_settings() -> AppSettings {
    #[cfg(target_os = "windows")]
    let default_shortcut = "ctrl+space";
    #[cfg(target_os = "macos")]
    let default_shortcut = "option+space";
    #[cfg(target_os = "linux")]
    let default_shortcut = "ctrl+space";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let default_shortcut = "alt+space";

    let mut bindings = HashMap::new();
    bindings.insert(
        "transcribe".to_string(),
        ShortcutBinding {
            id: "transcribe".to_string(),
            name: "Transcribe".to_string(),
            description: "Converts your speech into text.".to_string(),
            default_binding: default_shortcut.to_string(),
            current_binding: default_shortcut.to_string(),
        },
    );
    bindings.insert(
        "cancel".to_string(),
        ShortcutBinding {
            id: "cancel".to_string(),
            name: "Cancel".to_string(),
            description: "Cancels the current recording.".to_string(),
            default_binding: "escape".to_string(),
            current_binding: "escape".to_string(),
        },
    );
    bindings.insert(
        "pause_toggle".to_string(),
        ShortcutBinding {
            id: "pause_toggle".to_string(),
            name: "Toggle Pause".to_string(),
            description: "Pauses/Resumes recording.".to_string(),
            default_binding: "Option+Shift+P".to_string(),
            current_binding: "Option+Shift+P".to_string(),
        },
    );
    bindings.insert(
        "voice_command".to_string(),
        ShortcutBinding {
            id: "voice_command".to_string(),
            name: "Voice Command".to_string(),
            description: "Activates voice command mode to control your computer.".to_string(),
            default_binding: "right_command".to_string(),
            current_binding: "right_command".to_string(),
        },
    );
    bindings.insert(
        "quick_chat".to_string(),
        ShortcutBinding {
            id: "quick_chat".to_string(),
            name: "Quick Chat".to_string(),
            description: "Opens a new AI chat window.".to_string(),
            default_binding: "".to_string(),
            current_binding: "".to_string(),
        },
    );
    bindings.insert(
        "speak_selection".to_string(),
        ShortcutBinding {
            id: "speak_selection".to_string(),
            name: "Speak Selection".to_string(),
            description: "Reads the currently selected text aloud using AI.".to_string(),
            default_binding: "Option+S".to_string(),
            current_binding: "Option+S".to_string(),
        },
    );
    bindings.insert(
        "context_chat".to_string(),
        ShortcutBinding {
            id: "context_chat".to_string(),
            name: "Voice Interaction".to_string(),
            description: "Talk to an AI about your current context (selection, clipboard, screen)."
                .to_string(),
            default_binding: "left_shift+right_command".to_string(),
            current_binding: "left_shift+right_command".to_string(),
        },
    );

    // Note: ramble_to_coherent is no longer a separate binding.
    // Unified hotkey: hold transcribe key = raw, quick tap = coherent.

    AppSettings {
        bindings,
        push_to_talk: true,
        audio_feedback: false,
        audio_feedback_volume: default_audio_feedback_volume(),
        sound_theme: default_sound_theme(),
        start_hidden: default_start_hidden(),
        autostart_enabled: default_autostart_enabled(),
        update_checks_enabled: default_update_checks_enabled(),
        selected_model: "".to_string(),
        always_on_microphone: false,
        selected_microphone: None,
        clamshell_microphone: None,
        selected_output_device: None,
        translate_to_english: false,
        selected_language: "auto".to_string(),
        overlay_position: default_overlay_position(),
        debug_mode: false,
        log_level: default_log_level(),
        custom_words: Vec::new(),
        model_unload_timeout: ModelUnloadTimeout::Never,
        word_correction_threshold: default_word_correction_threshold(),
        history_limit: default_history_limit(),
        recording_retention_period: default_recording_retention_period(),
        // Unified LLM Provider Configuration
        llm_providers: default_llm_providers(),
        llm_models: default_llm_models(),
        default_chat_model_id: Some("gemini-flash".to_string()),
        default_coherent_model_id: Some("gemini-flash".to_string()),
        default_voice_model_id: Some("gemini-flash".to_string()),
        default_context_chat_model_id: None,
        // Other settings
        paste_method: PasteMethod::default(),
        clipboard_handling: ClipboardHandling::default(),
        coherent_prompts: default_coherent_prompts(),
        coherent_selected_prompt_id: Some("ramble_to_coherent".to_string()),
        mute_while_recording: false,
        append_trailing_space: false,
        app_language: default_app_language(),
        coherent_enabled: default_coherent_enabled(),
        coherent_use_vision: false,
        hold_threshold_ms: default_hold_threshold_ms(),
        // App-aware prompt settings
        prompt_mode: PromptMode::default(),
        prompt_categories: default_prompt_categories(),
        app_category_mappings: Vec::new(),
        detected_apps_history: Vec::new(),
        default_category_id: default_category_id(),
        // Voice command settings
        voice_commands_enabled: false,
        voice_command_default_model: default_voice_command_model(),
        voice_commands: default_voice_commands(),
        // TTS Settings
        tts_enabled: default_tts_enabled(),
        tts_selected_model: None,
        tts_speed: default_tts_speed(),
        tts_volume: default_tts_volume(),
        filler_word_filter: default_filler_word_filter(),
        collapse_repeated_words: default_collapse_repeated_words(),
        quick_chat_initial_prompt: default_quick_chat_initial_prompt(),
        // Unknown command agent settings
        unknown_command_agent_enabled: false,
        unknown_command_template: default_unknown_command_template(),
        unknown_command_terminal: default_unknown_command_terminal(),
        // Clipboard settings
        clipboard_content_cutoff: 0,
        context_chat_prompt: default_context_chat_prompt(),
        last_voice_interaction: None,
        // System prompt file
        system_prompt_file: None,
    }
}

impl AppSettings {
    /// Get a provider by ID
    pub fn get_provider(&self, provider_id: &str) -> Option<&LLMProvider> {
        self.llm_providers
            .iter()
            .find(|provider| provider.id == provider_id)
    }

    /// Get a model by ID
    pub fn get_model(&self, model_id: &str) -> Option<&LLMModel> {
        self.llm_models.iter().find(|model| model.id == model_id)
    }

    /// Get provider for a model
    pub fn get_provider_for_model(&self, model_id: &str) -> Option<&LLMProvider> {
        self.get_model(model_id)
            .and_then(|model| self.get_provider(&model.provider_id))
    }
}

pub fn load_or_create_app_settings(app: &AppHandle) -> AppSettings {
    // Initialize store
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    let mut settings = if let Some(settings_value) = store.get("settings") {
        // Parse the entire settings object
        match serde_json::from_value::<AppSettings>(settings_value) {
            Ok(mut settings) => {
                debug!("Found existing settings: {:?}", settings);
                let default_settings = get_default_settings();
                let mut updated = false;

                // Merge default bindings into existing settings
                for (key, value) in default_settings.bindings {
                    if !settings.bindings.contains_key(&key) {
                        debug!("Adding missing binding: {}", key);
                        settings.bindings.insert(key, value);
                        updated = true;
                    }
                }

                // Migration: Remove deprecated ramble_to_coherent binding
                // This binding is now merged into the transcribe key (hold=raw, quick press=coherent)
                if settings.bindings.remove("ramble_to_coherent").is_some() {
                    debug!("Removed deprecated ramble_to_coherent binding");
                    updated = true;
                }

                if updated {
                    debug!("Settings updated with new bindings");
                    store.set("settings", serde_json::to_value(&settings).unwrap());
                }

                settings
            }
            Err(e) => {
                warn!("Failed to parse settings: {}", e);
                // Fall back to default settings if parsing fails
                let default_settings = get_default_settings();
                store.set("settings", serde_json::to_value(&default_settings).unwrap());
                default_settings
            }
        }
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    // Migration: Convert LegacyInferable command types to Builtin
    // LegacyInferable was the old "inferable" type, now treated as Builtin
    let mut legacy_migrated = false;
    for cmd in &mut settings.voice_commands {
        if cmd.command_type == VoiceCommandType::LegacyInferable {
            debug!(
                "Migrating voice command '{}' from LegacyInferable to Builtin",
                cmd.name
            );
            cmd.command_type = VoiceCommandType::Builtin;
            legacy_migrated = true;
        }
    }
    if legacy_migrated {
        // Save immediately to prevent serialization errors later
        if let Ok(value) = serde_json::to_value(&settings) {
            store.set("settings", value);
        }
    }

    // Migration: Fix invalid model IDs (e.g. gemini-2.5-flash-lite -> gemini-2-0-flash-lite)
    let invalid_models = ["gemini-2.5-flash-lite", "gemini-2.5-flash"];
    let replacement_model = "gemini-2-0-flash-lite";
    let mut changed = false;

    if invalid_models.contains(&settings.selected_model.as_str()) {
        debug!(
            "Migrating selected_model from {} to {}",
            settings.selected_model, replacement_model
        );
        settings.selected_model = replacement_model.to_string();
        changed = true;
    }

    if let Some(voice_model) = &settings.default_voice_model_id {
        if invalid_models.contains(&voice_model.as_str()) {
            debug!(
                "Migrating default_voice_model_id from {} to {}",
                voice_model, replacement_model
            );
            settings.default_voice_model_id = Some(replacement_model.to_string());
            changed = true;
        }
    }

    if let Some(coherent_model) = &settings.default_coherent_model_id {
        if invalid_models.contains(&coherent_model.as_str()) {
            debug!(
                "Migrating default_coherent_model_id from {} to {}",
                coherent_model, replacement_model
            );
            settings.default_coherent_model_id = Some(replacement_model.to_string());
            changed = true;
        }
    }

    if changed {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    // Migration: Replace old prompt categories with new Low/Medium/High defaults
    // and migrate app_category_mappings to use new category IDs
    if migrate_prompt_categories(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    if ensure_llm_defaults(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    if ensure_voice_command_defaults(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    settings
}

pub fn get_settings(app: &AppHandle) -> AppSettings {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    let mut settings = if let Some(settings_value) = store.get("settings") {
        serde_json::from_value::<AppSettings>(settings_value).unwrap_or_else(|_| {
            let default_settings = get_default_settings();
            store.set("settings", serde_json::to_value(&default_settings).unwrap());
            default_settings
        })
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    if ensure_llm_defaults(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    if ensure_voice_command_defaults(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    settings
}

pub fn write_settings(app: &AppHandle, settings: AppSettings) {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    store.set("settings", serde_json::to_value(&settings).unwrap());
}

pub fn get_bindings(app: &AppHandle) -> HashMap<String, ShortcutBinding> {
    let settings = get_settings(app);

    settings.bindings
}

pub fn get_stored_binding(app: &AppHandle, id: &str) -> ShortcutBinding {
    let bindings = get_bindings(app);

    let binding = bindings.get(id).unwrap().clone();

    binding
}

pub fn get_history_limit(app: &AppHandle) -> usize {
    let settings = get_settings(app);
    settings.history_limit
}

/// Read the content of the system prompt file if configured.
/// Returns None if no file is configured or if reading fails.
pub fn get_system_prompt_content(app: &AppHandle) -> Option<String> {
    let settings = get_settings(app);
    let path = settings.system_prompt_file.as_ref()?;

    if path.trim().is_empty() {
        return None;
    }

    match std::fs::read_to_string(path) {
        Ok(content) => {
            if content.trim().is_empty() {
                None
            } else {
                Some(content)
            }
        }
        Err(e) => {
            log::warn!("Failed to read system prompt file '{}': {}", path, e);
            None
        }
    }
}

/// Prepend system prompt content to a user prompt if configured.
/// If no system prompt is configured, returns the original prompt unchanged.
pub fn inject_system_prompt(app: &AppHandle, user_prompt: &str) -> String {
    match get_system_prompt_content(app) {
        Some(system_prompt) => format!("{}\n\n---\n\n{}", system_prompt.trim(), user_prompt),
        None => user_prompt.to_string(),
    }
}

pub fn get_recording_retention_period(app: &AppHandle) -> RecordingRetentionPeriod {
    let settings = get_settings(app);
    settings.recording_retention_period
}
