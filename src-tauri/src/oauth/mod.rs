//! OAuth 2.0 authentication module for Handy
//!
//! This module provides OAuth authentication support for AI providers:
//! - Google Gemini (via Gemini CLI OAuth)
//! - OpenAI ChatGPT (via Codex OAuth)
//!
//! Anthropic does not support OAuth and continues to use API keys only.

pub mod google;
pub mod openai;
pub mod pkce;
pub mod server;
pub mod tokens;

use serde::{Deserialize, Serialize};
use specta::Type;

/// Supported OAuth providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    Google,
    OpenAI,
}

impl OAuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::OpenAI => "openai",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "google" | "gemini" | "gemini_oauth" => Some(OAuthProvider::Google),
            "openai" | "chatgpt" | "openai_oauth" => Some(OAuthProvider::OpenAI),
            _ => None,
        }
    }

    /// Get the callback port for this provider
    pub fn callback_port(&self) -> u16 {
        match self {
            OAuthProvider::Google => 8085,
            OAuthProvider::OpenAI => 1455,
        }
    }

    /// Get the callback path for this provider
    pub fn callback_path(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "/oauth2callback",
            OAuthProvider::OpenAI => "/auth/callback",
        }
    }

    /// Get the full redirect URI for this provider
    pub fn redirect_uri(&self) -> String {
        format!(
            "http://localhost:{}{}",
            self.callback_port(),
            self.callback_path()
        )
    }
}

/// Result of starting the OAuth flow
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AuthStartResult {
    /// The URL to open in the browser for user authentication
    pub auth_url: String,
    /// The state parameter for CSRF protection
    pub state: String,
}

/// Result of completing the OAuth flow
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AuthResult {
    /// Whether authentication was successful
    pub success: bool,
    /// User's email (if available)
    pub email: Option<String>,
    /// Error message (if authentication failed)
    pub error: Option<String>,
}

/// OAuth status for a provider
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OAuthStatus {
    /// Whether the user is authenticated
    pub authenticated: bool,
    /// User's email (if available and authenticated)
    pub email: Option<String>,
    /// Token expiration timestamp (Unix seconds)
    pub expires_at: Option<i64>,
}

/// Request headers for making authenticated API calls
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OAuthHeaders {
    /// Headers to include in API requests
    pub headers: std::collections::HashMap<String, String>,
}
