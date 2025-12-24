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

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PostProcessProvider {
    pub id: String,
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub allow_base_url_edit: bool,
    #[serde(default)]
    pub models_endpoint: Option<String>,
    #[serde(default)]
    pub supports_vision: bool,
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

/// Prompt mode selection - Dynamic auto-detects based on app, others are explicit overrides
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum PromptMode {
    #[default]
    Dynamic,
    Development,
    Conversation,
    Writing,
    Email,
}

impl PromptMode {
    /// Get the icon for this mode (used in overlay and tray menu)
    pub fn icon(&self) -> &'static str {
        match self {
            PromptMode::Dynamic => "🔄",
            PromptMode::Development => "💻",
            PromptMode::Conversation => "💬",
            PromptMode::Writing => "✍️",
            PromptMode::Email => "📧",
        }
    }

    /// Get the display name for this mode
    pub fn display_name(&self) -> &'static str {
        match self {
            PromptMode::Dynamic => "Dynamic",
            PromptMode::Development => "Development",
            PromptMode::Conversation => "Conversation",
            PromptMode::Writing => "Writing",
            PromptMode::Email => "Email",
        }
    }

    /// Get the category ID for this mode (used for prompt lookup)
    pub fn category_id(&self) -> Option<&'static str> {
        match self {
            PromptMode::Dynamic => None, // Will be determined by app detection
            PromptMode::Development => Some("development"),
            PromptMode::Conversation => Some("conversation"),
            PromptMode::Writing => Some("writing"),
            PromptMode::Email => Some("email"),
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
    #[serde(default)]
    pub paste_method: PasteMethod,
    #[serde(default)]
    pub clipboard_handling: ClipboardHandling,
    #[serde(default = "default_post_process_enabled")]
    pub post_process_enabled: bool,
    #[serde(default = "default_post_process_provider_id")]
    pub post_process_provider_id: String,
    #[serde(default = "default_post_process_providers")]
    pub post_process_providers: Vec<PostProcessProvider>,
    #[serde(default = "default_post_process_api_keys")]
    pub post_process_api_keys: HashMap<String, String>,
    #[serde(default = "default_post_process_models")]
    pub post_process_models: HashMap<String, String>,
    #[serde(default = "default_post_process_prompts")]
    pub post_process_prompts: Vec<LLMPrompt>,
    #[serde(default)]
    pub post_process_selected_prompt_id: Option<String>,
    #[serde(default)]
    pub mute_while_recording: bool,
    #[serde(default)]
    pub append_trailing_space: bool,
    #[serde(default = "default_app_language")]
    pub app_language: String,
    // Ramble to Coherent settings (separate from post-processing)
    #[serde(default = "default_ramble_enabled")]
    pub ramble_enabled: bool,
    #[serde(default = "default_ramble_provider_id")]
    pub ramble_provider_id: String,
    #[serde(default)]
    pub ramble_model: String,
    #[serde(default = "default_ramble_prompt")]
    pub ramble_prompt: String,
    #[serde(default = "default_ramble_use_vision_model")]
    pub ramble_use_vision_model: bool,
    #[serde(default = "default_ramble_vision_model")]
    pub ramble_vision_model: String,
    /// Threshold in milliseconds for tap vs hold detection (smart PTT)
    #[serde(default = "default_hold_threshold_ms")]
    pub hold_threshold_ms: u64,
    // App-aware prompt settings
    /// Current prompt mode (Dynamic, Development, Conversation, Writing, Email)
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

fn default_post_process_enabled() -> bool {
    false
}

fn default_app_language() -> String {
    tauri_plugin_os::locale()
        .and_then(|l| l.split(['-', '_']).next().map(String::from))
        .unwrap_or_else(|| "en".to_string())
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
    "gemini-2.5-flash-lite".to_string()
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

IMPORTANT: You are the user's proxy. Speak AS the user, not TO the user. Formulate the response as if the user is typing it.

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

Selected text (may be empty):
${selection}

Input transcript:
${output}".to_string()
}

fn default_hold_threshold_ms() -> u64 {
    500 // 500ms feels more natural - fast enough for PTT, slow enough for accidental taps
}

fn default_category_id() -> String {
    "development".to_string()
}

fn default_prompt_categories() -> Vec<PromptCategory> {
    vec![
        PromptCategory {
            id: "development".to_string(),
            name: "Development".to_string(),
            icon: "💻".to_string(),
            is_builtin: true,
            prompt: "You are transforming rambling speech into clean, well-structured text.

**Context:** The user is in ${application} (${category} mode). The output will be used in developer tools or sent to AI assistants.

The input is unfiltered speech-to-text. Your job is to make it readable while preserving all meaning.

IMPORTANT: You are the user's proxy. Speak AS the user, not TO the user. Formulate the response as if the user is typing it.

---

INLINE INSTRUCTIONS - The speaker may give you direct commands during dictation:

Explicit commands (always obey these):
- \"Hey Ramble, ...\" or \"Ramble: ...\" signals a direct instruction to you
- Example: \"Hey Ramble, ignore the last sentence\" → delete the preceding sentence
- Example: \"Ramble: expand on that idea\" → elaborate on the previous point

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
2. Fix run-on sentences. Break them into clear, punctuated sentences
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

Selected text (may be empty):
${selection}

Input transcript:
${output}".to_string(),
        },
        PromptCategory {
            id: "conversation".to_string(),
            name: "Conversation".to_string(),
            icon: "💬".to_string(),
            is_builtin: true,
            prompt: "You are cleaning up speech-to-text for a casual chat message.

**Context:** The user is in ${application} (${category} mode). The output is a message to another human.

IMPORTANT: You are the user's proxy. The message should sound exactly like the user would type it.

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

Selected text (may be empty):
${selection}

Input transcript:
${output}".to_string(),
        },
        PromptCategory {
            id: "writing".to_string(),
            name: "Writing".to_string(),
            icon: "✍️".to_string(),
            is_builtin: true,
            prompt: "You are transforming rambling speech into polished written prose.

**Context:** The user is in ${application} (${category} mode). The output is written content for human readers.

IMPORTANT: You are the user's proxy. Write AS the user, not TO the user.

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

Selected text (may be empty):
${selection}

Input transcript:
${output}".to_string(),
        },
        PromptCategory {
            id: "email".to_string(),
            name: "Email".to_string(),
            icon: "📧".to_string(),
            is_builtin: true,
            prompt: "You are transforming rambling speech into a clear email message.

**Context:** The user is in ${application} (${category} mode). The output is an email to another human.

IMPORTANT: You are the user's proxy. Write AS the user, not TO the user.

CRITICAL RULES:
1. NO em dashes (—) ever. Use commas or periods instead.
2. Be concise but don't remove information unless explicitly asked
3. Use professional but not overly formal tone
4. Only remove content if the user explicitly instructs (\"scratch that\", \"delete that\")

YOUR JOB:
1. Get to the point quickly
2. Remove filler words and self-corrections
3. Structure clearly: greeting, main content, closing (if user mentions them)
4. Preserve action items and deadlines exactly as stated
5. If the user rambles about multiple topics, keep them but organize clearly

FORMAT:
- Keep paragraphs short (2-3 sentences max)
- Use bullet points ONLY if user explicitly lists items or says \"bullet point\"
- Don't add greetings or closings unless the user mentions them

INLINE COMMANDS:
- \"hey Ramble, ...\" = direct instruction
- \"scratch that\", \"delete that\", \"never mind\" = remove preceding content

Return ONLY the email body text. No preamble.

---

Selected text (may be empty):
${selection}

Input transcript:
${output}".to_string(),
        },
    ]
}

fn default_post_process_providers() -> Vec<PostProcessProvider> {
    let mut providers = vec![
        PostProcessProvider {
            id: "openai".to_string(),
            label: "OpenAI".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
            supports_vision: true,
        },
        PostProcessProvider {
            id: "openrouter".to_string(),
            label: "OpenRouter".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
            supports_vision: true,
        },
        PostProcessProvider {
            id: "anthropic".to_string(),
            label: "Anthropic".to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
            supports_vision: true,
        },
        PostProcessProvider {
            id: "gemini".to_string(),
            label: "Google Gemini".to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
            supports_vision: true,
        },
        PostProcessProvider {
            id: "custom".to_string(),
            label: "Custom".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            allow_base_url_edit: true,
            models_endpoint: Some("/models".to_string()),
            supports_vision: true,
        },
    ];

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        if crate::apple_intelligence::check_apple_intelligence_availability() {
            providers.push(PostProcessProvider {
                id: APPLE_INTELLIGENCE_PROVIDER_ID.to_string(),
                label: "Apple Intelligence".to_string(),
                base_url: "apple-intelligence://local".to_string(),
                allow_base_url_edit: false,
                models_endpoint: None,
                supports_vision: false,
            });
        }
    }

    providers
}

fn default_post_process_api_keys() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for provider in default_post_process_providers() {
        map.insert(provider.id, String::new());
    }
    map
}

fn default_model_for_provider(provider_id: &str) -> String {
    if provider_id == APPLE_INTELLIGENCE_PROVIDER_ID {
        return APPLE_INTELLIGENCE_DEFAULT_MODEL_ID.to_string();
    }
    String::new()
}

fn default_post_process_models() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for provider in default_post_process_providers() {
        map.insert(
            provider.id.clone(),
            default_model_for_provider(&provider.id),
        );
    }
    map
}

fn default_post_process_prompts() -> Vec<LLMPrompt> {
    vec![
        LLMPrompt {
            id: "default_improve_transcriptions".to_string(),
            name: "Improve Transcriptions".to_string(),
            prompt: "Clean this transcript:\n1. Fix spelling, capitalization, and punctuation errors\n2. Convert number words to digits (twenty-five → 25, ten percent → 10%, five dollars → $5)\n3. Replace spoken punctuation with symbols (period → ., comma → ,, question mark → ?)\n4. Remove filler words (um, uh, like as filler)\n5. Keep the language in the original version (if it was french, keep it in french for example)\n\nPreserve exact meaning and word order. Do not paraphrase or reorder content.\n\nReturn only the cleaned transcript.\n\nTranscript:\n${output}".to_string(),
        },
        LLMPrompt {
            id: "ramble_to_coherent".to_string(),
            name: "Ramble to Coherent".to_string(),
            prompt: "You are transforming raw speech into clean, coherent text.\n\nThe input is unfiltered speech-to-text that contains:\n- Filler words (um, uh, like, you know, basically, so, I mean)\n- Thinking out loud and self-corrections\n- Backtracking (no wait, actually, I mean)\n- Repeated ideas phrased multiple ways\n- Run-on sentences and stream of consciousness\n\nYour task:\n1. Extract the core intent and requirements\n2. Remove ALL filler words and verbal tics\n3. When the speaker changes their mind, keep ONLY the final decision\n4. Consolidate repeated ideas into single clear statements\n5. Structure as clear, actionable points if appropriate\n6. Preserve technical terms and specific details exactly\n7. Keep the same language as input\n\nReturn ONLY the cleaned, structured text. No preamble or explanation.\n\nInput transcript:\n${output}".to_string(),
        },
    ]
}

fn ensure_post_process_defaults(settings: &mut AppSettings) -> bool {
    let mut changed = false;
    for provider in default_post_process_providers() {
        // 1. Add missing providers
        if settings
            .post_process_providers
            .iter()
            .all(|existing| existing.id != provider.id)
        {
            settings.post_process_providers.push(provider.clone());
            changed = true;
        }

        // 2. Ensure API key entry exists
        if !settings.post_process_api_keys.contains_key(&provider.id) {
            settings
                .post_process_api_keys
                .insert(provider.id.clone(), String::new());
            changed = true;
        }

        // 3. Ensure Model entry exists
        let default_model = default_model_for_provider(&provider.id);
        match settings.post_process_models.get_mut(&provider.id) {
            Some(existing) => {
                if existing.is_empty() && !default_model.is_empty() {
                    *existing = default_model.clone();
                    changed = true;
                }
            }
            None => {
                settings
                    .post_process_models
                    .insert(provider.id.clone(), default_model);
                changed = true;
            }
        }

        // 4. Sync capability flags (supports_vision) for default providers
        // This ensures existing users get the new capability enabled automatically
        if let Some(existing) = settings
            .post_process_providers
            .iter_mut()
            .find(|p| p.id == provider.id)
        {
            if existing.supports_vision != provider.supports_vision {
                debug!(
                    "Updating supports_vision for provider '{}': {} -> {}",
                    existing.id, existing.supports_vision, provider.supports_vision
                );
                existing.supports_vision = provider.supports_vision;
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
        "vision_capture".to_string(),
        ShortcutBinding {
            id: "vision_capture".to_string(),
            name: "Vision Capture".to_string(),
            description: "Captures screenshot during recording.".to_string(),
            default_binding: "Option+Shift+S".to_string(),
            current_binding: "Option+Shift+S".to_string(),
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
        paste_method: PasteMethod::default(),
        clipboard_handling: ClipboardHandling::default(),
        post_process_enabled: default_post_process_enabled(),
        post_process_provider_id: default_post_process_provider_id(),
        post_process_providers: default_post_process_providers(),
        post_process_api_keys: default_post_process_api_keys(),
        post_process_models: default_post_process_models(),
        post_process_prompts: default_post_process_prompts(),
        post_process_selected_prompt_id: None,
        mute_while_recording: false,
        append_trailing_space: false,
        app_language: default_app_language(),
        ramble_enabled: default_ramble_enabled(),
        ramble_provider_id: default_ramble_provider_id(),
        ramble_model: default_ramble_model(),
        ramble_prompt: default_ramble_prompt(),
        ramble_use_vision_model: default_ramble_use_vision_model(),
        ramble_vision_model: default_ramble_vision_model(),
        hold_threshold_ms: default_hold_threshold_ms(),
        // App-aware prompt settings
        prompt_mode: PromptMode::default(),
        prompt_categories: default_prompt_categories(),
        app_category_mappings: Vec::new(),
        detected_apps_history: Vec::new(),
        default_category_id: default_category_id(),
    }
}

impl AppSettings {
    pub fn active_post_process_provider(&self) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == self.post_process_provider_id)
    }

    pub fn post_process_provider(&self, provider_id: &str) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == provider_id)
    }

    pub fn post_process_provider_mut(
        &mut self,
        provider_id: &str,
    ) -> Option<&mut PostProcessProvider> {
        self.post_process_providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
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

    if ensure_post_process_defaults(&mut settings) {
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

    if ensure_post_process_defaults(&mut settings) {
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

pub fn get_recording_retention_period(app: &AppHandle) -> RecordingRetentionPeriod {
    let settings = get_settings(app);
    settings.recording_retention_period
}
