//! OAuth token storage and management
//!
//! Handles local storage of OAuth tokens in the app data directory.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

use super::OAuthProvider;

/// Filename for on-disk OAuth token storage
const TOKEN_STORE_FILE: &str = "oauth_tokens.json";

static TOKEN_STORE_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Stored OAuth tokens for a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTokens {
    /// OAuth access token
    pub access_token: String,
    /// OAuth refresh token (for refreshing access)
    pub refresh_token: String,
    /// Token expiration timestamp (Unix seconds)
    pub expires_at: i64,
    /// User's email (if available)
    pub email: Option<String>,
    /// OpenAI-specific: ChatGPT account ID extracted from JWT
    pub chatgpt_account_id: Option<String>,
}

impl StoredTokens {
    /// Check if the access token is expired (with 5 minute buffer)
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        // Consider expired 5 minutes before actual expiry
        self.expires_at - 300 <= now
    }
}

/// Token storage error
#[derive(Debug)]
pub enum TokenError {
    /// Failed to access token storage
    StorageError(String),
    /// Failed to serialize/deserialize tokens
    SerializationError(String),
    /// Required configuration is missing
    ConfigMissing(String),
    /// Tokens not found
    NotFound,
    /// Token refresh failed
    RefreshFailed(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::StorageError(msg) => write!(f, "Token storage error: {}", msg),
            TokenError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            TokenError::ConfigMissing(msg) => write!(f, "Missing configuration: {}", msg),
            TokenError::NotFound => write!(f, "Tokens not found"),
            TokenError::RefreshFailed(msg) => write!(f, "Token refresh failed: {}", msg),
        }
    }
}

impl std::error::Error for TokenError {}

#[derive(Debug, Serialize, Deserialize, Default)]
struct TokenStoreFile {
    tokens: HashMap<String, StoredTokens>,
}

pub fn init_token_store(app: &AppHandle) -> Result<(), TokenError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| TokenError::StorageError(e.to_string()))?;

    if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
        return Err(TokenError::StorageError(e.to_string()));
    }

    let path = app_data_dir.join(TOKEN_STORE_FILE);
    let _ = TOKEN_STORE_PATH.set(path);
    Ok(())
}

fn token_store_path() -> Result<PathBuf, TokenError> {
    TOKEN_STORE_PATH
        .get()
        .cloned()
        .ok_or_else(|| TokenError::ConfigMissing("OAuth token store not initialized".to_string()))
}

fn read_store(path: &Path) -> Result<TokenStoreFile, TokenError> {
    let json = std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            TokenError::NotFound
        } else {
            TokenError::StorageError(e.to_string())
        }
    })?;

    serde_json::from_str::<TokenStoreFile>(&json)
        .map_err(|e| TokenError::SerializationError(e.to_string()))
}

fn read_store_or_default(path: &Path) -> Result<TokenStoreFile, TokenError> {
    match read_store(path) {
        Ok(store) => Ok(store),
        Err(TokenError::NotFound) => Ok(TokenStoreFile::default()),
        Err(e) => Err(e),
    }
}

fn write_store(path: &Path, store: &TokenStoreFile) -> Result<(), TokenError> {
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| TokenError::SerializationError(e.to_string()))?;

    std::fs::write(path, json).map_err(|e| TokenError::StorageError(e.to_string()))
}

/// Store tokens for a provider
pub fn store_tokens(provider: OAuthProvider, tokens: &StoredTokens) -> Result<(), TokenError> {
    let path = token_store_path()?;
    let mut store = read_store_or_default(&path)?;
    store
        .tokens
        .insert(provider.as_str().to_string(), tokens.clone());
    write_store(&path, &store)?;

    log::info!(
        "Stored OAuth tokens for {} in local token store",
        provider.as_str()
    );
    Ok(())
}

/// Load tokens for a provider
pub fn load_tokens(provider: OAuthProvider) -> Result<StoredTokens, TokenError> {
    log::info!(
        "load_tokens: loading tokens for provider {:?} from local token store",
        provider
    );

    let path = token_store_path()?;
    let store = read_store(&path)?;
    let tokens = store
        .tokens
        .get(provider.as_str())
        .cloned()
        .ok_or(TokenError::NotFound)?;

    log::info!(
        "load_tokens: loaded tokens (email={:?}, expires_at={}, is_expired={})",
        tokens.email,
        tokens.expires_at,
        tokens.is_expired()
    );
    Ok(tokens)
}

/// Delete tokens for a provider
pub fn delete_tokens(provider: OAuthProvider) -> Result<(), TokenError> {
    let path = token_store_path()?;
    let mut store = read_store(&path)?;
    let removed = store.tokens.remove(provider.as_str());

    if removed.is_none() {
        return Err(TokenError::NotFound);
    }

    if store.tokens.is_empty() {
        if let Err(e) = std::fs::remove_file(&path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(TokenError::StorageError(e.to_string()));
            }
        }
    } else {
        write_store(&path, &store)?;
    }

    log::info!(
        "Deleted OAuth tokens for {} from local token store",
        provider.as_str()
    );
    Ok(())
}

/// Parse a JWT token and extract claims
///
/// This is a simple base64 decode without signature verification,
/// suitable for extracting claims from tokens we received from OAuth providers.
pub fn parse_jwt_claims(token: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    // Decode the payload (middle part)
    let payload = URL_SAFE_NO_PAD.decode(parts[1]).ok()?;
    serde_json::from_slice(&payload).ok()
}

/// Extract the ChatGPT account ID from an OpenAI access token JWT
pub fn extract_chatgpt_account_id(access_token: &str) -> Option<String> {
    let claims = parse_jwt_claims(access_token)?;

    // The account ID is in the custom claim "https://api.openai.com/auth"
    claims
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_expiry_check() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Token that expires in 1 hour
        let tokens = StoredTokens {
            access_token: "test".to_string(),
            refresh_token: "test".to_string(),
            expires_at: now + 3600,
            email: None,
            chatgpt_account_id: None,
        };
        assert!(!tokens.is_expired());

        // Token that expired 1 hour ago
        let expired_tokens = StoredTokens {
            access_token: "test".to_string(),
            refresh_token: "test".to_string(),
            expires_at: now - 3600,
            email: None,
            chatgpt_account_id: None,
        };
        assert!(expired_tokens.is_expired());
    }
}
