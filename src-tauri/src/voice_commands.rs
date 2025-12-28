//! Voice command execution and script running
//!
//! This module handles executing voice commands, including:
//! - Running shell commands
//! - Running AppleScript
//! - LLM-based command interpretation for inferable commands

use crate::settings::{ScriptType, VoiceCommand};
use log::{debug, error, info};
use std::process::Command;

/// Result of executing a voice command
#[derive(Debug)]
pub enum CommandResult {
    /// Command produced text output to paste
    PasteOutput(String),
    /// Command executed successfully with no output
    Success,
    /// Command failed with an error message
    Error(String),
    /// Internal command that should be handled by the caller
    InternalCommand(String),
}

/// Execute a bespoke (user-defined script) command
pub fn execute_bespoke_command(command: &VoiceCommand) -> CommandResult {
    let script = match &command.script {
        Some(s) if !s.trim().is_empty() => s,
        _ => {
            return CommandResult::Error(format!(
                "Command '{}' has no script defined",
                command.name
            ))
        }
    };

    debug!(
        "Executing bespoke command '{}' with script type {:?}",
        command.name, command.script_type
    );

    // Check for internal commands (not actual shell/applescript)
    if script == "open_chat_window" {
        // This is handled specially by the caller
        return CommandResult::InternalCommand("open_chat_window".to_string());
    }

    match command.script_type {
        ScriptType::Shell => execute_shell_script(script),
        ScriptType::AppleScript => execute_applescript(script),
    }
}

/// Execute a shell script
fn execute_shell_script(script: &str) -> CommandResult {
    debug!("Running shell script: {}", script);

    match Command::new("sh").arg("-c").arg(script).output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if stdout.is_empty() {
                    info!("Shell script executed successfully (no output)");
                    CommandResult::Success
                } else {
                    info!(
                        "Shell script executed successfully with output ({} chars)",
                        stdout.len()
                    );
                    CommandResult::PasteOutput(stdout)
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                error!("Shell script failed: {}", stderr);
                CommandResult::Error(format!("Script failed: {}", stderr))
            }
        }
        Err(e) => {
            error!("Failed to execute shell script: {}", e);
            CommandResult::Error(format!("Failed to run script: {}", e))
        }
    }
}

/// Execute an AppleScript (macOS only)
#[cfg(target_os = "macos")]
fn execute_applescript(script: &str) -> CommandResult {
    debug!("Running AppleScript: {}", script);

    match Command::new("osascript").arg("-e").arg(script).output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if stdout.is_empty() {
                    info!("AppleScript executed successfully (no output)");
                    CommandResult::Success
                } else {
                    info!(
                        "AppleScript executed successfully with output ({} chars)",
                        stdout.len()
                    );
                    CommandResult::PasteOutput(stdout)
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                error!("AppleScript failed: {}", stderr);
                CommandResult::Error(format!("AppleScript failed: {}", stderr))
            }
        }
        Err(e) => {
            error!("Failed to execute AppleScript: {}", e);
            CommandResult::Error(format!("Failed to run AppleScript: {}", e))
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn execute_applescript(_script: &str) -> CommandResult {
    CommandResult::Error("AppleScript is only supported on macOS".to_string())
}

/// Find the best matching command for the given spoken text
/// Prioritizes matches that appear earlier in the text
pub fn find_matching_command<'a>(
    spoken_text: &str,
    commands: &'a [VoiceCommand],
) -> Option<&'a VoiceCommand> {
    let spoken_lower = spoken_text.to_lowercase();

    // Find all matching commands with their earliest match position
    let mut matches: Vec<(&VoiceCommand, usize)> = Vec::new();

    for command in commands {
        let mut earliest_pos: Option<usize> = None;
        for phrase in &command.phrases {
            if let Some(pos) = spoken_lower.find(&phrase.to_lowercase()) {
                match earliest_pos {
                    None => earliest_pos = Some(pos),
                    Some(current) if pos < current => earliest_pos = Some(pos),
                    _ => {}
                }
            }
        }
        if let Some(pos) = earliest_pos {
            matches.push((command, pos));
        }
    }

    // Sort by position (earliest first) and return the best match
    if !matches.is_empty() {
        matches.sort_by_key(|(_, pos)| *pos);
        let (best_match, _) = matches[0];
        debug!(
            "Matched command '{}' (earliest position in text)",
            best_match.name
        );
        return Some(best_match);
    }

    None
}

/// Build the system prompt for LLM command interpretation
pub fn build_command_prompt(commands: &[VoiceCommand], selection: Option<&str>) -> String {
    let mut prompt = String::from(
        "You are Ramble's command interpreter. Given a user's spoken command and available actions, determine which action to execute.\n\n",
    );

    prompt.push_str("Available commands:\n");
    for cmd in commands {
        let type_label = match cmd.command_type {
            crate::settings::VoiceCommandType::Inferable => "inferable",
            crate::settings::VoiceCommandType::Bespoke => "bespoke",
            crate::settings::VoiceCommandType::ComputerUse => "computer_use",
        };
        prompt.push_str(&format!(
            "- {} ({}) [type: {}]: ",
            cmd.id, cmd.name, type_label
        ));
        if let Some(desc) = &cmd.description {
            prompt.push_str(desc);
        }
        prompt.push_str(&format!(" [Trigger phrases: {}]\n", cmd.phrases.join(", ")));
    }

    prompt.push_str("\nCurrent context:\n");
    prompt.push_str(&format!("- Selection: {}\n", selection.unwrap_or("(none)")));

    prompt.push_str(
        r#"

VALID EXECUTION TYPES (use ONLY these exact values):
- "shell" - run a shell command
- "applescript" - run AppleScript (macOS only)  
- "bespoke" - execute a predefined script by command ID
- "paste" - paste text to the user
- "computer_use" - AI agent that sees screen and performs UI automation

DECISION LOGIC:
1. If user says "Computer, do X" or "Agent, do X" → execution_type = "computer_use"
2. If a configured command matches → use that command's type
3. If task needs web browsing, clicking, navigating apps, forms, or any visual interaction → execution_type = "computer_use"
4. If task is a simple shell/system command → execution_type = "shell"
5. If task is just a text response or information → execution_type = "paste"

CRITICAL: execution_type MUST be one of: "shell", "applescript", "bespoke", "paste", "computer_use"
Do NOT invent other types like "web_search", "browse", etc. Use "computer_use" for anything needing screen interaction.

Respond with JSON:
{
  "matched_command": "command_id" or null,
  "execution_type": "shell" | "applescript" | "bespoke" | "paste" | "computer_use",
  "command": "shell/applescript command" (for shell/applescript),
  "output": "text to paste" (for paste),
  "task_description": "full task for computer use agent" (for computer_use),
  "explanation": "brief explanation"
}

If nothing can be done:
{
  "matched_command": null,
  "message": "explanation to show user"
}

IMPORTANT: Return ONLY raw JSON. No markdown code blocks."#,
    );

    prompt
}
