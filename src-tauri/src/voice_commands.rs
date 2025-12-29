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
///
/// If the script contains `${selection}`, it will be replaced with the provided selection text.
/// The selection is properly escaped for the script type (shell or AppleScript).
pub fn execute_bespoke_command(command: &VoiceCommand, selection: Option<&str>) -> CommandResult {
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

    // Substitute ${selection} with the actual selection text (escaped appropriately)
    let processed_script = if script.contains("${selection}") {
        let selection_text = selection.unwrap_or("");
        let escaped_selection = match command.script_type {
            ScriptType::Shell => escape_for_shell(selection_text),
            ScriptType::AppleScript => escape_for_applescript(selection_text),
        };
        debug!(
            "Substituting ${{selection}} with {} chars of text",
            selection_text.len()
        );
        script.replace("${selection}", &escaped_selection)
    } else {
        script.clone()
    };

    match command.script_type {
        ScriptType::Shell => execute_shell_script(&processed_script),
        ScriptType::AppleScript => execute_applescript(&processed_script),
    }
}

/// Escape a string for safe inclusion in a shell script (single-quoted context)
fn escape_for_shell(s: &str) -> String {
    // For shell, we escape single quotes by ending the single-quoted string,
    // adding an escaped single quote, and starting a new single-quoted string
    s.replace('\'', "'\\''")
}

/// Escape a string for safe inclusion in an AppleScript string (double-quoted context)
fn escape_for_applescript(s: &str) -> String {
    // AppleScript uses backslash escaping within double quotes
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
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
            crate::settings::VoiceCommandType::Builtin
            | crate::settings::VoiceCommandType::LegacyInferable => "builtin",
            crate::settings::VoiceCommandType::Custom => "custom",
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

COMMAND TYPES:
- "builtin" commands: Have native handlers. Just match and return the command ID.
- "custom" commands: Have user-defined scripts. Just match and return the command ID.

Respond with JSON:
{
  "matched_command": "command_id" or null,
  "execution_type": "builtin" | "custom" | "paste",
  "explanation": "brief explanation"
}

For text responses or information:
{
  "matched_command": null,
  "execution_type": "paste",
  "output": "text to paste"
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
