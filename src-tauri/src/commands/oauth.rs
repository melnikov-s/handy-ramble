//! OAuth Tauri commands
//!
//! Provides commands for the frontend to initiate and complete OAuth flows.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::oauth::pkce::{generate_state, PkceChallenge};
use crate::oauth::server::wait_for_callback;
use crate::oauth::tokens::{delete_tokens, load_tokens};
use crate::oauth::{google, openai, AuthResult, AuthStartResult, OAuthProvider, OAuthStatus};

/// In-flight OAuth state storage
/// Maps state -> (provider, verifier)
static OAUTH_STATE: LazyLock<Mutex<HashMap<String, (OAuthProvider, String)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Start the OAuth flow for a provider
///
/// Returns the authorization URL to open in the browser.
#[tauri::command]
#[specta::specta]
pub async fn oauth_start_auth(app: AppHandle, provider: String) -> Result<AuthStartResult, String> {
    let provider = OAuthProvider::from_str(&provider)
        .ok_or_else(|| format!("Unknown OAuth provider: {}", provider))?;

    // Generate PKCE challenge and state
    let pkce = PkceChallenge::new();
    let state = generate_state();

    // Build authorization URL based on provider
    let auth_url = match provider {
        OAuthProvider::Google => {
            google::build_auth_url(&pkce, &state).map_err(|e| e.to_string())?
        }
        OAuthProvider::OpenAI => openai::build_auth_url(&pkce, &state),
    };

    // Store state for verification
    {
        let mut oauth_state = OAUTH_STATE.lock().map_err(|e| e.to_string())?;
        oauth_state.insert(state.clone(), (provider, pkce.verifier.clone()));
    }

    // Open the authorization URL in the default browser
    app.opener()
        .open_url(&auth_url, None::<String>)
        .map_err(|e| format!("Failed to open browser: {}", e))?;

    log::info!("Started OAuth flow for {}", provider.as_str());

    Ok(AuthStartResult { auth_url, state })
}

/// Wait for and complete the OAuth callback
///
/// This should be called after oauth_start_auth. It waits for the callback,
/// exchanges the code for tokens, and stores them securely.
#[tauri::command]
#[specta::specta]
pub async fn oauth_await_callback(provider: String, state: String) -> Result<AuthResult, String> {
    let oauth_provider = OAuthProvider::from_str(&provider)
        .ok_or_else(|| format!("Unknown OAuth provider: {}", provider))?;

    // Get the stored verifier for this state
    let verifier = {
        let oauth_state = OAUTH_STATE.lock().map_err(|e| e.to_string())?;
        oauth_state
            .get(&state)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| "Invalid or expired OAuth state".to_string())?
    };

    let expected_state = state.clone();

    log::info!(
        "OAuth await_callback: starting for provider={}, state={}",
        provider,
        state
    );

    // Wait for callback (5 minute timeout)
    let timeout = Duration::from_secs(300);
    log::info!("OAuth await_callback: spawning blocking task to wait for callback");
    let callback_result = tokio::task::spawn_blocking(move || {
        wait_for_callback(oauth_provider, &expected_state, timeout)
    })
    .await
    .map_err(|e| format!("Callback task failed: {}", e))?;

    log::info!("OAuth await_callback: callback task completed");

    // Clean up stored state
    {
        let mut oauth_state = OAUTH_STATE.lock().map_err(|e| e.to_string())?;
        oauth_state.remove(&state);
    }

    let callback = match callback_result {
        Ok(cb) => {
            log::info!(
                "OAuth callback received for {}: code length={}, state length={}",
                provider,
                cb.code.len(),
                cb.state.len()
            );
            cb
        }
        Err(e) => {
            log::error!("OAuth callback failed for {}: {}", provider, e);
            return Ok(AuthResult {
                success: false,
                email: None,
                error: Some(e.to_string()),
            });
        }
    };

    // Get the verifier for code exchange
    // For Google, extract from encoded state; for OpenAI, use stored verifier
    let code_verifier = match oauth_provider {
        OAuthProvider::Google => {
            // Decode the state to get verifier
            google::decode_state(&callback.state)
                .map(|(_, v)| v)
                .unwrap_or(verifier)
        }
        OAuthProvider::OpenAI => verifier,
    };

    // Exchange code for tokens
    log::info!("Exchanging code for tokens for provider: {}", provider);
    let tokens_result = match oauth_provider {
        OAuthProvider::Google => google::exchange_code(&callback.code, &code_verifier).await,
        OAuthProvider::OpenAI => openai::exchange_code(&callback.code, &code_verifier).await,
    };

    match tokens_result {
        Ok(tokens) => {
            log::info!(
                "OAuth authentication successful for {} (email: {:?})",
                provider,
                tokens.email
            );
            Ok(AuthResult {
                success: true,
                email: tokens.email,
                error: None,
            })
        }
        Err(e) => {
            log::error!("Token exchange failed for {}: {}", provider, e);
            Ok(AuthResult {
                success: false,
                email: None,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Get OAuth status for a provider
#[tauri::command]
#[specta::specta]
pub fn oauth_get_status(provider: String) -> Result<OAuthStatus, String> {
    let provider = OAuthProvider::from_str(&provider)
        .ok_or_else(|| format!("Unknown OAuth provider: {}", provider))?;

    match load_tokens(provider) {
        Ok(tokens) => Ok(OAuthStatus {
            authenticated: true,
            email: tokens.email,
            expires_at: Some(tokens.expires_at),
        }),
        Err(crate::oauth::tokens::TokenError::NotFound) => Ok(OAuthStatus {
            authenticated: false,
            email: None,
            expires_at: None,
        }),
        Err(e) => {
            log::error!(
                "Error checking OAuth status for {}: {}",
                provider.as_str(),
                e
            );
            Err(e.to_string())
        }
    }
}

/// Log out from OAuth for a provider
#[tauri::command]
#[specta::specta]
pub fn oauth_logout(provider: String) -> Result<(), String> {
    let provider = OAuthProvider::from_str(&provider)
        .ok_or_else(|| format!("Unknown OAuth provider: {}", provider))?;

    match delete_tokens(provider) {
        Ok(()) => {
            log::info!("OAuth logout successful for {}", provider.as_str());
            Ok(())
        }
        Err(crate::oauth::tokens::TokenError::NotFound) => {
            // Already logged out
            Ok(())
        }
        Err(e) => {
            log::error!("OAuth logout failed for {}: {}", provider.as_str(), e);
            Err(e.to_string())
        }
    }
}

/// Refresh the OAuth token for a provider
#[tauri::command]
#[specta::specta]
pub async fn oauth_refresh_token(provider: String) -> Result<bool, String> {
    let oauth_provider = OAuthProvider::from_str(&provider)
        .ok_or_else(|| format!("Unknown OAuth provider: {}", provider))?;

    // Load existing tokens
    let tokens = load_tokens(oauth_provider).map_err(|e| e.to_string())?;

    // Refresh based on provider
    let result = match oauth_provider {
        OAuthProvider::Google => google::refresh_token(&tokens.refresh_token).await,
        OAuthProvider::OpenAI => openai::refresh_token(&tokens.refresh_token).await,
    };

    match result {
        Ok(_) => {
            log::info!("OAuth token refreshed for {}", provider);
            Ok(true)
        }
        Err(e) => {
            log::error!("OAuth token refresh failed for {}: {}", provider, e);
            // Don't delete tokens on refresh failure - user might want to try again
            Ok(false)
        }
    }
}

/// Get current access token for a provider (for making API calls)
#[tauri::command]
#[specta::specta]
pub fn oauth_get_access_token(provider: String) -> Result<Option<String>, String> {
    let provider = OAuthProvider::from_str(&provider)
        .ok_or_else(|| format!("Unknown OAuth provider: {}", provider))?;

    match load_tokens(provider) {
        Ok(tokens) => {
            if tokens.is_expired() {
                // Token is expired, caller should refresh
                Ok(None)
            } else {
                Ok(Some(tokens.access_token))
            }
        }
        Err(crate::oauth::tokens::TokenError::NotFound) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Get request headers for making authenticated API calls
#[tauri::command]
#[specta::specta]
pub fn oauth_get_request_headers(provider: String) -> Result<HashMap<String, String>, String> {
    let oauth_provider = OAuthProvider::from_str(&provider)
        .ok_or_else(|| format!("Unknown OAuth provider: {}", provider))?;

    let tokens = load_tokens(oauth_provider).map_err(|e| e.to_string())?;

    if tokens.is_expired() {
        return Err("Access token is expired. Please refresh first.".to_string());
    }

    let headers = match oauth_provider {
        OAuthProvider::Google => google::get_request_headers(&tokens.access_token),
        OAuthProvider::OpenAI => openai::get_request_headers(&tokens),
    };

    Ok(headers)
}

/// Check if OAuth is supported for a provider ID
#[tauri::command]
#[specta::specta]
pub fn oauth_supports_provider(provider_id: String) -> bool {
    OAuthProvider::from_str(&provider_id).is_some()
}
