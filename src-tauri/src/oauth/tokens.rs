//! OAuth token storage and management
//!
//! Handles secure storage of OAuth tokens using the OS keychain.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::OAuthProvider;

/// Service name for keyring storage
const KEYRING_SERVICE: &str = "com.handy.oauth";

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

    /// Check if the access token will expire within the given seconds
    pub fn expires_within(&self, seconds: i64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        self.expires_at - seconds <= now
    }
}

/// Token storage error
#[derive(Debug)]
pub enum TokenError {
    /// Failed to access keyring
    KeyringError(String),
    /// Failed to serialize/deserialize tokens
    SerializationError(String),
    /// Tokens not found
    NotFound,
    /// Token refresh failed
    RefreshFailed(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::KeyringError(msg) => write!(f, "Keyring error: {}", msg),
            TokenError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            TokenError::NotFound => write!(f, "Tokens not found"),
            TokenError::RefreshFailed(msg) => write!(f, "Token refresh failed: {}", msg),
        }
    }
}

impl std::error::Error for TokenError {}

/// Get the keyring entry for a provider
fn get_entry(provider: OAuthProvider) -> Result<Entry, TokenError> {
    Entry::new(KEYRING_SERVICE, provider.as_str())
        .map_err(|e| TokenError::KeyringError(e.to_string()))
}

/// Store tokens for a provider
pub fn store_tokens(provider: OAuthProvider, tokens: &StoredTokens) -> Result<(), TokenError> {
    let entry = get_entry(provider)?;
    let json =
        serde_json::to_string(tokens).map_err(|e| TokenError::SerializationError(e.to_string()))?;

    entry
        .set_password(&json)
        .map_err(|e| TokenError::KeyringError(e.to_string()))?;

    log::info!("Stored OAuth tokens for {}", provider.as_str());
    Ok(())
}

/// Load tokens for a provider
pub fn load_tokens(provider: OAuthProvider) -> Result<StoredTokens, TokenError> {
    log::info!("load_tokens: loading tokens for provider {:?}", provider);

    let entry = match get_entry(provider) {
        Ok(e) => {
            log::info!("load_tokens: got keyring entry for {}", provider.as_str());
            e
        }
        Err(e) => {
            log::error!("load_tokens: failed to get keyring entry: {}", e);
            return Err(e);
        }
    };

    let json = match entry.get_password() {
        Ok(j) => {
            log::info!(
                "load_tokens: retrieved password from keyring (length={})",
                j.len()
            );
            j
        }
        Err(e) => {
            let err = match e {
                keyring::Error::NoEntry => {
                    log::warn!("load_tokens: no tokens found for {}", provider.as_str());
                    TokenError::NotFound
                }
                _ => {
                    log::error!("load_tokens: keyring error: {}", e);
                    TokenError::KeyringError(e.to_string())
                }
            };
            return Err(err);
        }
    };

    let tokens: StoredTokens = match serde_json::from_str::<StoredTokens>(&json) {
        Ok(t) => {
            log::info!(
                "load_tokens: successfully parsed tokens (email={:?}, expires_at={}, is_expired={})",
                t.email,
                t.expires_at,
                t.is_expired()
            );
            t
        }
        Err(e) => {
            log::error!("load_tokens: failed to parse tokens JSON: {}", e);
            return Err(TokenError::SerializationError(e.to_string()));
        }
    };

    Ok(tokens)
}

/// Delete tokens for a provider
pub fn delete_tokens(provider: OAuthProvider) -> Result<(), TokenError> {
    let entry = get_entry(provider)?;

    entry.delete_credential().map_err(|e| match e {
        keyring::Error::NoEntry => TokenError::NotFound,
        _ => TokenError::KeyringError(e.to_string()),
    })?;

    log::info!("Deleted OAuth tokens for {}", provider.as_str());
    Ok(())
}

/// Check if tokens exist for a provider
pub fn has_tokens(provider: OAuthProvider) -> bool {
    load_tokens(provider).is_ok()
}

/// Get a valid access token for a provider, refreshing if necessary
///
/// Returns None if not authenticated or refresh fails.
pub fn get_valid_access_token(provider: OAuthProvider) -> Option<String> {
    match load_tokens(provider) {
        Ok(tokens) => {
            if tokens.is_expired() {
                log::info!("Access token expired for {}", provider.as_str());
                None // Caller should trigger refresh
            } else {
                Some(tokens.access_token)
            }
        }
        Err(TokenError::NotFound) => None,
        Err(e) => {
            log::error!("Error loading tokens for {}: {}", provider.as_str(), e);
            None
        }
    }
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

    #[test]
    fn test_expires_within() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let tokens = StoredTokens {
            access_token: "test".to_string(),
            refresh_token: "test".to_string(),
            expires_at: now + 300, // Expires in 5 minutes
            email: None,
            chatgpt_account_id: None,
        };

        assert!(tokens.expires_within(600)); // Within 10 minutes
        assert!(!tokens.expires_within(60)); // Not within 1 minute (has 5 min left)
    }
}
