//! Gemini 2.5 Computer Use agent implementation
//!
//! This module provides an agentic loop that:
//! 1. Captures screenshots of the current screen
//! 2. Sends them to Gemini Computer Use model with a task description
//! 3. Parses UI actions from the response
//! 4. Executes actions using native input methods
//! 5. Repeats until task is complete or stopped

use crate::input::EnigoState;
use crate::settings::get_settings;
use crate::vision::capture_screen_for_computer_use;
use enigo::{Axis, Button, Coordinate, Direction, Keyboard, Mouse};
use log::{debug, error, info, warn};
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use xcap::Monitor;

/// Result of a computer use agent run
#[derive(Debug)]
pub struct AgentResult {
    pub success: bool,
    pub steps_taken: usize,
    pub final_output: Option<String>,
    pub error: Option<String>,
}

/// Scroll direction for scroll actions
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Actions that can be executed by the computer use agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum ComputerAction {
    /// Open web browser
    OpenWebBrowser,
    /// Wait for 5 seconds
    Wait5Seconds,
    /// Go back in browser
    GoBack,
    /// Go forward in browser
    GoForward,
    /// Search (opens default search)
    Search,
    /// Navigate to URL
    Navigate { url: String },
    /// Click at coordinates
    ClickAt {
        x: i32,
        y: i32,
        #[serde(default)]
        safety_decision: Option<SafetyDecision>,
    },
    /// Hover at coordinates
    HoverAt { x: i32, y: i32 },
    /// Type text at coordinates
    TypeTextAt {
        x: i32,
        y: i32,
        text: String,
        #[serde(default)]
        press_enter: bool,
        #[serde(default)]
        clear_before_typing: bool,
        #[serde(default)]
        safety_decision: Option<SafetyDecision>,
    },
    /// Key combination (e.g., "Control+A")
    KeyCombination { keys: String },
    /// Scroll document in a direction
    ScrollDocument { direction: ScrollDirection },
    /// Scroll at coordinates
    ScrollAt {
        x: i32,
        y: i32,
        direction: ScrollDirection,
        #[serde(default)]
        magnitude: i32,
    },
    /// Drag and drop
    DragAndDrop {
        x: i32,
        y: i32,
        destination_x: i32,
        destination_y: i32,
    },
}

/// Safety decision from Gemini's internal safety system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyDecision {
    pub explanation: String,
    pub decision: String,
}

impl SafetyDecision {
    /// Returns true if user confirmation is required
    pub fn requires_confirmation(&self) -> bool {
        self.decision == "require_confirmation"
    }
}

/// The Computer Use agent that orchestrates the agent loop
pub struct ComputerUseAgent {
    app: AppHandle,
    model: String,
    max_steps: usize,
    stop_signal: Arc<AtomicBool>,
    screen_width: i32,
    screen_height: i32,
}

impl ComputerUseAgent {
    /// Create a new Computer Use agent
    pub fn new(app: AppHandle, stop_signal: Arc<AtomicBool>) -> Result<Self, String> {
        let settings = get_settings(&app);

        // Get screen dimensions
        let monitors = Monitor::all().map_err(|e| format!("Failed to get monitors: {}", e))?;
        let monitor = monitors.into_iter().next().ok_or("No monitors found")?;
        let screen_width = monitor
            .width()
            .map_err(|e| format!("Failed to get width: {}", e))? as i32;
        let screen_height = monitor
            .height()
            .map_err(|e| format!("Failed to get height: {}", e))?
            as i32;

        Ok(Self {
            app,
            model: settings.computer_use_model.clone(),
            max_steps: settings.computer_use_max_steps,
            stop_signal,
            screen_width,
            screen_height,
        })
    }

    /// Check if the agent should stop
    fn should_stop(&self) -> bool {
        self.stop_signal.load(Ordering::SeqCst)
    }

    /// Denormalize X coordinate from 0-1000 range to actual pixels
    fn denormalize_x(&self, x: i32) -> i32 {
        (x as f64 / 1000.0 * self.screen_width as f64) as i32
    }

    /// Denormalize Y coordinate from 0-1000 range to actual pixels
    fn denormalize_y(&self, y: i32) -> i32 {
        (y as f64 / 1000.0 * self.screen_height as f64) as i32
    }

    /// Execute a single action
    pub fn execute_action(&self, action: &ComputerAction) -> Result<(), String> {
        let enigo_state = self
            .app
            .try_state::<EnigoState>()
            .ok_or("Enigo state not available")?;

        let mut enigo = enigo_state
            .0
            .lock()
            .map_err(|_| "Failed to lock Enigo state")?;

        match action {
            ComputerAction::OpenWebBrowser => {
                debug!("Opening default web browser to Google");
                // On macOS, open URL with default browser
                #[cfg(target_os = "macos")]
                {
                    std::process::Command::new("open")
                        .arg("https://www.google.com")
                        .spawn()
                        .map_err(|e| format!("Failed to open browser: {}", e))?;
                }
                // Wait for browser to load
                std::thread::sleep(std::time::Duration::from_millis(1500));
                Ok(())
            }

            ComputerAction::Wait5Seconds => {
                debug!("Waiting 5 seconds");
                std::thread::sleep(std::time::Duration::from_secs(5));
                Ok(())
            }

            ComputerAction::GoBack => {
                debug!("Going back");
                // Cmd+[ on macOS
                #[cfg(target_os = "macos")]
                {
                    enigo
                        .key(enigo::Key::Meta, Direction::Press)
                        .map_err(|e| format!("Failed to press Meta: {}", e))?;
                    enigo
                        .key(enigo::Key::Unicode('['), Direction::Click)
                        .map_err(|e| format!("Failed to click [: {}", e))?;
                    enigo
                        .key(enigo::Key::Meta, Direction::Release)
                        .map_err(|e| format!("Failed to release Meta: {}", e))?;
                }
                Ok(())
            }

            ComputerAction::GoForward => {
                debug!("Going forward");
                #[cfg(target_os = "macos")]
                {
                    enigo
                        .key(enigo::Key::Meta, Direction::Press)
                        .map_err(|e| format!("Failed to press Meta: {}", e))?;
                    enigo
                        .key(enigo::Key::Unicode(']'), Direction::Click)
                        .map_err(|e| format!("Failed to click ]: {}", e))?;
                    enigo
                        .key(enigo::Key::Meta, Direction::Release)
                        .map_err(|e| format!("Failed to release Meta: {}", e))?;
                }
                Ok(())
            }

            ComputerAction::Search => {
                debug!("Opening search");
                // Cmd+Space for Spotlight on macOS
                #[cfg(target_os = "macos")]
                {
                    enigo
                        .key(enigo::Key::Meta, Direction::Press)
                        .map_err(|e| format!("Failed to press Meta: {}", e))?;
                    enigo
                        .key(enigo::Key::Space, Direction::Click)
                        .map_err(|e| format!("Failed to click Space: {}", e))?;
                    enigo
                        .key(enigo::Key::Meta, Direction::Release)
                        .map_err(|e| format!("Failed to release Meta: {}", e))?;
                }
                Ok(())
            }

            ComputerAction::Navigate { url } => {
                debug!("Navigating to: {}", url);
                #[cfg(target_os = "macos")]
                {
                    // Use system default browser
                    std::process::Command::new("open")
                        .arg(url)
                        .spawn()
                        .map_err(|e| format!("Failed to open URL: {}", e))?;
                }
                // Wait longer for page to load
                std::thread::sleep(std::time::Duration::from_millis(2000));
                Ok(())
            }

            ComputerAction::ClickAt { x, y, .. } => {
                let actual_x = self.denormalize_x(*x);
                let actual_y = self.denormalize_y(*y);
                debug!(
                    "Clicking at ({}, {}) -> pixel ({}, {})",
                    x, y, actual_x, actual_y
                );

                enigo
                    .move_mouse(actual_x, actual_y, Coordinate::Abs)
                    .map_err(|e| format!("Failed to move mouse: {}", e))?;
                std::thread::sleep(std::time::Duration::from_millis(50));
                enigo
                    .button(Button::Left, Direction::Click)
                    .map_err(|e| format!("Failed to click: {}", e))?;
                Ok(())
            }

            ComputerAction::HoverAt { x, y } => {
                let actual_x = self.denormalize_x(*x);
                let actual_y = self.denormalize_y(*y);
                debug!(
                    "Hovering at ({}, {}) -> pixel ({}, {})",
                    x, y, actual_x, actual_y
                );

                enigo
                    .move_mouse(actual_x, actual_y, Coordinate::Abs)
                    .map_err(|e| format!("Failed to move mouse: {}", e))?;
                Ok(())
            }

            ComputerAction::TypeTextAt {
                x,
                y,
                text,
                press_enter,
                clear_before_typing,
                ..
            } => {
                let actual_x = self.denormalize_x(*x);
                let actual_y = self.denormalize_y(*y);
                debug!(
                    "Typing '{}' at ({}, {}) -> pixel ({}, {})",
                    text, x, y, actual_x, actual_y
                );

                // Click at position
                enigo
                    .move_mouse(actual_x, actual_y, Coordinate::Abs)
                    .map_err(|e| format!("Failed to move mouse: {}", e))?;
                std::thread::sleep(std::time::Duration::from_millis(50));
                enigo
                    .button(Button::Left, Direction::Click)
                    .map_err(|e| format!("Failed to click: {}", e))?;
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Clear if requested (Cmd+A, Backspace on macOS)
                if *clear_before_typing {
                    #[cfg(target_os = "macos")]
                    {
                        enigo
                            .key(enigo::Key::Meta, Direction::Press)
                            .map_err(|e| format!("Failed to press Meta: {}", e))?;
                        enigo
                            .key(enigo::Key::Unicode('a'), Direction::Click)
                            .map_err(|e| format!("Failed to click A: {}", e))?;
                        enigo
                            .key(enigo::Key::Meta, Direction::Release)
                            .map_err(|e| format!("Failed to release Meta: {}", e))?;
                        enigo
                            .key(enigo::Key::Backspace, Direction::Click)
                            .map_err(|e| format!("Failed to click Backspace: {}", e))?;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }

                // Type the text
                enigo
                    .text(text)
                    .map_err(|e| format!("Failed to type text: {}", e))?;

                // Press enter if requested
                if *press_enter {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    enigo
                        .key(enigo::Key::Return, Direction::Click)
                        .map_err(|e| format!("Failed to press Enter: {}", e))?;
                }

                Ok(())
            }

            ComputerAction::KeyCombination { keys } => {
                debug!("Key combination: {}", keys);
                let parts: Vec<&str> = keys.split('+').collect();

                // Press all modifier keys
                for part in &parts[..parts.len().saturating_sub(1)] {
                    let key = parse_key(part)?;
                    enigo
                        .key(key, Direction::Press)
                        .map_err(|e| format!("Failed to press {}: {}", part, e))?;
                }

                // Click the final key
                if let Some(final_key) = parts.last() {
                    let key = parse_key(final_key)?;
                    enigo
                        .key(key, Direction::Click)
                        .map_err(|e| format!("Failed to click {}: {}", final_key, e))?;
                }

                // Release all modifier keys in reverse order
                for part in parts[..parts.len().saturating_sub(1)].iter().rev() {
                    let key = parse_key(part)?;
                    enigo
                        .key(key, Direction::Release)
                        .map_err(|e| format!("Failed to release {}: {}", part, e))?;
                }

                Ok(())
            }

            ComputerAction::ScrollDocument { direction } => {
                debug!("Scrolling document: {:?}", direction);
                let lines = match direction {
                    ScrollDirection::Up => -3,
                    ScrollDirection::Down => 3,
                    ScrollDirection::Left | ScrollDirection::Right => 0,
                };
                let axis = match direction {
                    ScrollDirection::Up | ScrollDirection::Down => Axis::Vertical,
                    ScrollDirection::Left | ScrollDirection::Right => Axis::Horizontal,
                };
                enigo
                    .scroll(lines, axis)
                    .map_err(|e| format!("Failed to scroll: {}", e))?;
                Ok(())
            }

            ComputerAction::ScrollAt {
                x,
                y,
                direction,
                magnitude,
            } => {
                let actual_x = self.denormalize_x(*x);
                let actual_y = self.denormalize_y(*y);
                debug!(
                    "Scrolling {:?} at ({}, {}) with magnitude {}",
                    direction, actual_x, actual_y, magnitude
                );

                // Move to position first
                enigo
                    .move_mouse(actual_x, actual_y, Coordinate::Abs)
                    .map_err(|e| format!("Failed to move mouse: {}", e))?;
                std::thread::sleep(std::time::Duration::from_millis(50));

                // Calculate scroll amount (magnitude is in pixels, convert to scroll units)
                let scroll_amount = (*magnitude / 100).max(1) as i32;
                let lines = match direction {
                    ScrollDirection::Up => -scroll_amount,
                    ScrollDirection::Down => scroll_amount,
                    ScrollDirection::Left => -scroll_amount,
                    ScrollDirection::Right => scroll_amount,
                };
                let axis = match direction {
                    ScrollDirection::Up | ScrollDirection::Down => Axis::Vertical,
                    ScrollDirection::Left | ScrollDirection::Right => Axis::Horizontal,
                };
                enigo
                    .scroll(lines, axis)
                    .map_err(|e| format!("Failed to scroll: {}", e))?;
                Ok(())
            }

            ComputerAction::DragAndDrop {
                x,
                y,
                destination_x,
                destination_y,
            } => {
                let start_x = self.denormalize_x(*x);
                let start_y = self.denormalize_y(*y);
                let end_x = self.denormalize_x(*destination_x);
                let end_y = self.denormalize_y(*destination_y);
                debug!(
                    "Dragging from ({}, {}) to ({}, {})",
                    start_x, start_y, end_x, end_y
                );

                // Move to start position
                enigo
                    .move_mouse(start_x, start_y, Coordinate::Abs)
                    .map_err(|e| format!("Failed to move to start: {}", e))?;
                std::thread::sleep(std::time::Duration::from_millis(50));

                // Press mouse button
                enigo
                    .button(Button::Left, Direction::Press)
                    .map_err(|e| format!("Failed to press mouse: {}", e))?;
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Move to end position
                enigo
                    .move_mouse(end_x, end_y, Coordinate::Abs)
                    .map_err(|e| format!("Failed to move to end: {}", e))?;
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Release mouse button
                enigo
                    .button(Button::Left, Direction::Release)
                    .map_err(|e| format!("Failed to release mouse: {}", e))?;
                Ok(())
            }
        }
    }

    /// Emit agent step event to frontend with human-readable description
    fn emit_step(&self, step: usize, action_name: &str) {
        let description = match action_name {
            "open_web_browser" => "Opening browser...",
            "navigate" => "Navigating...",
            "click_at" => "Clicking...",
            "hover_at" => "Hovering...",
            "type_text_at" => "Typing...",
            "scroll_at" | "scroll_document" => "Scrolling...",
            "search" => "Searching...",
            "wait_5_seconds" => "Waiting...",
            "key_combination" => "Pressing keys...",
            "drag_and_drop" => "Dragging...",
            "go_back" => "Going back...",
            "go_forward" => "Going forward...",
            _ => action_name,
        };

        let _ = self.app.emit(
            "computer-use-step",
            serde_json::json!({
                "step": step,
                "action": action_name,
                "description": description,
            }),
        );
    }

    /// Emit agent start event with task description
    fn emit_start(&self, task: &str) {
        let _ = self.app.emit(
            "computer-use-start",
            serde_json::json!({
                "task": task,
            }),
        );
    }

    /// Emit agent end event
    /// Emit agent end event with completion message
    fn emit_end(&self, success: bool, message: Option<&str>) {
        let _ = self.app.emit(
            "computer-use-end",
            serde_json::json!({
                "success": success,
                "message": message.unwrap_or(""),
            }),
        );
    }

    /// Run the computer use agent loop
    ///
    /// This implements the "see, think, act" loop:
    /// 1. Capture screenshot
    /// 2. Send to Gemini with task and screenshot
    /// 3. Parse function calls from response
    /// 4. Execute actions
    /// 5. Repeat until done or stopped
    pub async fn run(&self, task: &str, api_key: &str) -> AgentResult {
        info!("Starting computer use agent with task: {}", task);
        self.emit_start(task);

        // Delay between actions for visibility
        let action_delay = std::time::Duration::from_millis(200);

        let mut conversation_history: Vec<serde_json::Value> = Vec::new();
        let mut steps_taken = 0;

        // Initial screenshot and user message
        let screenshot = match capture_screen_for_computer_use() {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to capture initial screenshot: {}", e);
                self.emit_end(false, Some("Failed to capture screenshot"));
                return AgentResult {
                    success: false,
                    steps_taken: 0,
                    final_output: None,
                    error: Some(format!("Failed to capture screenshot: {}", e)),
                };
            }
        };

        // Build initial user content with task and screenshot
        conversation_history.push(serde_json::json!({
            "role": "user",
            "parts": [
                { "text": task },
                {
                    "inline_data": {
                        "mime_type": "image/png",
                        "data": screenshot
                    }
                }
            ]
        }));

        // Main agent loop
        while steps_taken < self.max_steps {
            // Check for stop signal (user pressed Escape or Cancel)
            if self.should_stop() {
                warn!("Agent stopped by user");
                self.emit_end(false, Some("Stopped by user"));
                return AgentResult {
                    success: false,
                    steps_taken,
                    final_output: None,
                    error: Some("Stopped by user".to_string()),
                };
            }

            // Send request to Gemini
            let response = match self.call_gemini_api(&conversation_history, api_key).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Gemini API call failed: {}", e);
                    self.emit_end(false, Some("API error"));
                    return AgentResult {
                        success: false,
                        steps_taken,
                        final_output: None,
                        error: Some(e),
                    };
                }
            };

            // Parse the response
            let candidates = response.get("candidates").and_then(|c| c.as_array());
            let candidate = match candidates.and_then(|c| c.first()) {
                Some(c) => c,
                None => {
                    error!("No candidates in Gemini response");
                    self.emit_end(false, Some("No response from model"));
                    return AgentResult {
                        success: false,
                        steps_taken,
                        final_output: None,
                        error: Some("No candidates in response".to_string()),
                    };
                }
            };

            let content = candidate.get("content").cloned().unwrap_or_default();
            let parts = content.get("parts").and_then(|p| p.as_array());

            // Debug: Log what Gemini returned
            debug!("Gemini response content: {:?}", content);
            if let Some(p) = parts {
                debug!("Response has {} parts", p.len());
                for (i, part) in p.iter().enumerate() {
                    if part.get("text").is_some() {
                        debug!("Part {}: text response", i);
                    }
                    if part.get("functionCall").is_some() {
                        debug!("Part {}: functionCall", i);
                    }
                }
            } else {
                warn!("Response has no parts!");
            }

            // Add model response to history
            conversation_history.push(serde_json::json!({
                "role": "model",
                "parts": content.get("parts").cloned().unwrap_or(serde_json::json!([]))
            }));

            // Check for function calls
            let mut has_function_calls = false;
            let mut function_responses: Vec<serde_json::Value> = Vec::new();
            let mut text_output: Option<String> = None;

            if let Some(parts) = parts {
                for part in parts {
                    // Check for text response (final answer)
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        text_output = Some(text.to_string());
                    }

                    // Check for function call (Gemini uses camelCase: "functionCall")
                    if let Some(function_call) = part.get("functionCall") {
                        has_function_calls = true;
                        let name = function_call
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("");
                        let args = function_call.get("args").cloned().unwrap_or_default();

                        debug!("Function call: {} with args: {:?}", name, args);
                        steps_taken += 1;
                        self.emit_step(steps_taken, name);

                        // Check for safety decision that requires confirmation
                        if let Some(safety_decision) = args.get("safety_decision") {
                            let decision = safety_decision.get("decision").and_then(|d| d.as_str());
                            if decision == Some("require_confirmation") {
                                let explanation = safety_decision
                                    .get("explanation")
                                    .and_then(|e| e.as_str())
                                    .unwrap_or("Action requires confirmation");
                                warn!("Action requires confirmation: {}", explanation);

                                // TODO: Emit event to frontend for user confirmation
                                // For now, we'll auto-confirm (this should be changed)
                                info!("Auto-confirming action (TODO: implement UI confirmation)");
                            }
                        }

                        // Parse and execute the action
                        match parse_action_from_function_call(name, &args) {
                            Ok(action) => {
                                // Add delay between actions for visibility
                                std::thread::sleep(action_delay);

                                if let Err(e) = self.execute_action(&action) {
                                    warn!("Action execution failed: {}", e);
                                    function_responses.push(serde_json::json!({
                                        "functionResponse": {
                                            "name": name,
                                            "response": { "error": e }
                                        }
                                    }));
                                } else {
                                    // Capture new screenshot after action
                                    let new_screenshot =
                                        capture_screen_for_computer_use().unwrap_or_default();

                                    // Get current URL if in browser context
                                    let current_url = get_browser_url()
                                        .unwrap_or_else(|| "about:blank".to_string());

                                    debug!(
                                        "Function response - URL: {}, screenshot: {} bytes",
                                        current_url,
                                        new_screenshot.len()
                                    );

                                    function_responses.push(serde_json::json!({
                                        "functionResponse": {
                                            "name": name,
                                            "response": {
                                                "url": current_url
                                            },
                                            "parts": [{
                                                "inlineData": {
                                                    "mimeType": "image/png",
                                                    "data": new_screenshot
                                                }
                                            }]
                                        }
                                    }));
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse action '{}': {}", name, e);
                                function_responses.push(serde_json::json!({
                                    "functionResponse": {
                                        "name": name,
                                        "response": { "error": e }
                                    }
                                }));
                            }
                        }
                    }
                }
            }

            if !has_function_calls {
                // Model is done - return the text output
                info!("Agent completed after {} steps", steps_taken);
                self.emit_end(true, text_output.as_deref());
                return AgentResult {
                    success: true,
                    steps_taken,
                    final_output: text_output,
                    error: None,
                };
            }

            // Add function responses to history
            if !function_responses.is_empty() {
                conversation_history.push(serde_json::json!({
                    "role": "user",
                    "parts": function_responses
                }));
            }
        }

        warn!("Agent reached max steps limit ({})", self.max_steps);
        self.emit_end(false, Some("Reached max steps limit"));
        AgentResult {
            success: false,
            steps_taken,
            final_output: None,
            error: Some(format!("Reached max steps limit ({})", self.max_steps)),
        }
    }

    /// Call the Gemini API with the conversation history (with retry for rate limits)
    async fn call_gemini_api(
        &self,
        contents: &[serde_json::Value],
        api_key: &str,
    ) -> Result<serde_json::Value, String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, api_key
        );

        let request_body = serde_json::json!({
            "contents": contents,
            "tools": [{
                "computer_use": {
                    "environment": "ENVIRONMENT_BROWSER"
                }
            }],
            "generationConfig": {
                "temperature": 0.0
            }
        });

        let client = reqwest::Client::new();
        let max_retries = 3;
        let mut retry_delay = std::time::Duration::from_secs(2);

        for attempt in 0..=max_retries {
            // Check stop signal before each attempt
            if self.should_stop() {
                return Err("Stopped by user".to_string());
            }

            let response = client
                .post(&url)
                .header(CONTENT_TYPE, "application/json")
                .json(&request_body)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            let status = response.status();

            if status.is_success() {
                return response
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|e| format!("Failed to parse response: {}", e));
            }

            // Handle rate limiting with retry
            if status.as_u16() == 429 && attempt < max_retries {
                warn!(
                    "Rate limited (429), retrying in {:?} (attempt {}/{})",
                    retry_delay,
                    attempt + 1,
                    max_retries
                );
                tokio::time::sleep(retry_delay).await;
                retry_delay *= 2; // Exponential backoff
                continue;
            }

            // Non-retryable error
            let body = response.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status, body));
        }

        Err("Max retries exceeded".to_string())
    }
}

/// Get the current URL from the browser (Safari, Chrome, or Arc)
/// Uses AppleScript on macOS to query the browser directly (doesn't require frontmost)
#[cfg(target_os = "macos")]
fn get_browser_url() -> Option<String> {
    use std::process::Command;

    // Try Safari first (doesn't need to be frontmost)
    let safari_script = r#"
        tell application "Safari"
            if (count of windows) > 0 then
                return URL of current tab of front window
            end if
        end tell
        return ""
    "#;

    if let Ok(output) = Command::new("osascript")
        .arg("-e")
        .arg(safari_script)
        .output()
    {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !url.is_empty() && url != "missing value" {
            return Some(url);
        }
    }

    // Try Chrome
    let chrome_script = r#"
        tell application "Google Chrome"
            if (count of windows) > 0 then
                return URL of active tab of front window
            end if
        end tell
        return ""
    "#;

    if let Ok(output) = Command::new("osascript")
        .arg("-e")
        .arg(chrome_script)
        .output()
    {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !url.is_empty() && url != "missing value" {
            return Some(url);
        }
    }

    // Try Arc
    let arc_script = r#"
        tell application "Arc"
            if (count of windows) > 0 then
                return URL of active tab of front window
            end if
        end tell
        return ""
    "#;

    if let Ok(output) = Command::new("osascript").arg("-e").arg(arc_script).output() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !url.is_empty() && url != "missing value" {
            return Some(url);
        }
    }

    None
}

#[cfg(not(target_os = "macos"))]
fn get_browser_url() -> Option<String> {
    None
}

/// Parse a key string into an enigo Key
fn parse_key(key_str: &str) -> Result<enigo::Key, String> {
    match key_str.to_lowercase().as_str() {
        "control" | "ctrl" => Ok(enigo::Key::Control),
        "shift" => Ok(enigo::Key::Shift),
        "alt" | "option" => Ok(enigo::Key::Alt),
        "meta" | "command" | "cmd" | "super" => Ok(enigo::Key::Meta),
        "enter" | "return" => Ok(enigo::Key::Return),
        "tab" => Ok(enigo::Key::Tab),
        "space" => Ok(enigo::Key::Space),
        "backspace" => Ok(enigo::Key::Backspace),
        "delete" => Ok(enigo::Key::Delete),
        "escape" | "esc" => Ok(enigo::Key::Escape),
        "up" => Ok(enigo::Key::UpArrow),
        "down" => Ok(enigo::Key::DownArrow),
        "left" => Ok(enigo::Key::LeftArrow),
        "right" => Ok(enigo::Key::RightArrow),
        "home" => Ok(enigo::Key::Home),
        "end" => Ok(enigo::Key::End),
        "pageup" => Ok(enigo::Key::PageUp),
        "pagedown" => Ok(enigo::Key::PageDown),
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap();
            Ok(enigo::Key::Unicode(c))
        }
        s if s.starts_with("f") && s.len() <= 3 => {
            // F1-F12
            match s {
                "f1" => Ok(enigo::Key::F1),
                "f2" => Ok(enigo::Key::F2),
                "f3" => Ok(enigo::Key::F3),
                "f4" => Ok(enigo::Key::F4),
                "f5" => Ok(enigo::Key::F5),
                "f6" => Ok(enigo::Key::F6),
                "f7" => Ok(enigo::Key::F7),
                "f8" => Ok(enigo::Key::F8),
                "f9" => Ok(enigo::Key::F9),
                "f10" => Ok(enigo::Key::F10),
                "f11" => Ok(enigo::Key::F11),
                "f12" => Ok(enigo::Key::F12),
                _ => Err(format!("Invalid F-key: {}", s)),
            }
        }
        _ => Err(format!("Unknown key: {}", key_str)),
    }
}

/// Parse action from Gemini function call response
pub fn parse_action_from_function_call(
    name: &str,
    args: &serde_json::Value,
) -> Result<ComputerAction, String> {
    match name {
        "open_web_browser" => Ok(ComputerAction::OpenWebBrowser),
        "wait_5_seconds" => Ok(ComputerAction::Wait5Seconds),
        "go_back" => Ok(ComputerAction::GoBack),
        "go_forward" => Ok(ComputerAction::GoForward),
        "search" => Ok(ComputerAction::Search),
        "navigate" => {
            let url = args
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or("navigate requires 'url' argument")?;
            Ok(ComputerAction::Navigate {
                url: url.to_string(),
            })
        }
        "click_at" => {
            let x = args
                .get("x")
                .and_then(|v| v.as_i64())
                .ok_or("click_at requires 'x' argument")? as i32;
            let y = args
                .get("y")
                .and_then(|v| v.as_i64())
                .ok_or("click_at requires 'y' argument")? as i32;
            let safety_decision = args
                .get("safety_decision")
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()
                .map_err(|e| format!("Failed to parse safety_decision: {}", e))?;
            Ok(ComputerAction::ClickAt {
                x,
                y,
                safety_decision,
            })
        }
        "hover_at" => {
            let x = args
                .get("x")
                .and_then(|v| v.as_i64())
                .ok_or("hover_at requires 'x' argument")? as i32;
            let y = args
                .get("y")
                .and_then(|v| v.as_i64())
                .ok_or("hover_at requires 'y' argument")? as i32;
            Ok(ComputerAction::HoverAt { x, y })
        }
        "type_text_at" => {
            let x = args
                .get("x")
                .and_then(|v| v.as_i64())
                .ok_or("type_text_at requires 'x' argument")? as i32;
            let y = args
                .get("y")
                .and_then(|v| v.as_i64())
                .ok_or("type_text_at requires 'y' argument")? as i32;
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("type_text_at requires 'text' argument")?
                .to_string();
            let press_enter = args
                .get("press_enter")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let clear_before_typing = args
                .get("clear_before_typing")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let safety_decision = args
                .get("safety_decision")
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()
                .map_err(|e| format!("Failed to parse safety_decision: {}", e))?;
            Ok(ComputerAction::TypeTextAt {
                x,
                y,
                text,
                press_enter,
                clear_before_typing,
                safety_decision,
            })
        }
        "key_combination" => {
            let keys = args
                .get("keys")
                .and_then(|v| v.as_str())
                .ok_or("key_combination requires 'keys' argument")?
                .to_string();
            Ok(ComputerAction::KeyCombination { keys })
        }
        "scroll_document" => {
            let direction_str = args
                .get("direction")
                .and_then(|v| v.as_str())
                .ok_or("scroll_document requires 'direction' argument")?;
            let direction = match direction_str.to_lowercase().as_str() {
                "up" => ScrollDirection::Up,
                "down" => ScrollDirection::Down,
                "left" => ScrollDirection::Left,
                "right" => ScrollDirection::Right,
                _ => return Err(format!("Invalid scroll direction: {}", direction_str)),
            };
            Ok(ComputerAction::ScrollDocument { direction })
        }
        "scroll_at" => {
            let x = args
                .get("x")
                .and_then(|v| v.as_i64())
                .ok_or("scroll_at requires 'x' argument")? as i32;
            let y = args
                .get("y")
                .and_then(|v| v.as_i64())
                .ok_or("scroll_at requires 'y' argument")? as i32;
            let direction_str = args
                .get("direction")
                .and_then(|v| v.as_str())
                .ok_or("scroll_at requires 'direction' argument")?;
            let direction = match direction_str.to_lowercase().as_str() {
                "up" => ScrollDirection::Up,
                "down" => ScrollDirection::Down,
                "left" => ScrollDirection::Left,
                "right" => ScrollDirection::Right,
                _ => return Err(format!("Invalid scroll direction: {}", direction_str)),
            };
            let magnitude = args
                .get("magnitude")
                .and_then(|v| v.as_i64())
                .unwrap_or(100) as i32;
            Ok(ComputerAction::ScrollAt {
                x,
                y,
                direction,
                magnitude,
            })
        }
        "drag_and_drop" => {
            let x = args
                .get("x")
                .and_then(|v| v.as_i64())
                .ok_or("drag_and_drop requires 'x' argument")? as i32;
            let y = args
                .get("y")
                .and_then(|v| v.as_i64())
                .ok_or("drag_and_drop requires 'y' argument")? as i32;
            let destination_x = args
                .get("destination_x")
                .and_then(|v| v.as_i64())
                .ok_or("drag_and_drop requires 'destination_x' argument")?
                as i32;
            let destination_y = args
                .get("destination_y")
                .and_then(|v| v.as_i64())
                .ok_or("drag_and_drop requires 'destination_y' argument")?
                as i32;
            Ok(ComputerAction::DragAndDrop {
                x,
                y,
                destination_x,
                destination_y,
            })
        }
        _ => Err(format!("Unknown action: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key() {
        assert!(matches!(parse_key("control"), Ok(enigo::Key::Control)));
        assert!(matches!(parse_key("Shift"), Ok(enigo::Key::Shift)));
        assert!(matches!(parse_key("a"), Ok(enigo::Key::Unicode('a'))));
        assert!(matches!(parse_key("f1"), Ok(enigo::Key::F1)));
        assert!(matches!(parse_key("f12"), Ok(enigo::Key::F12)));
    }

    #[test]
    fn test_parse_action_from_function_call() {
        let args = serde_json::json!({"x": 500, "y": 300});
        let action = parse_action_from_function_call("click_at", &args).unwrap();
        assert!(matches!(
            action,
            ComputerAction::ClickAt { x: 500, y: 300, .. }
        ));
    }
}
