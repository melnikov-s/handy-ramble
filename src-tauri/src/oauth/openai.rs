//! OpenAI OAuth implementation for ChatGPT API
//!
//! Uses the Codex CLI OAuth credentials for ChatGPT Plus/Pro accounts.

use serde::Deserialize;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use super::config::get_openai_client_id;
use super::pkce::PkceChallenge;
use super::tokens::{extract_chatgpt_account_id, store_tokens, StoredTokens, TokenError};
use super::OAuthProvider;

/// OpenAI OAuth configuration (Codex CLI credentials)
pub const DEFAULT_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
// OpenAI uses a public client (no client secret)
pub const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
pub const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub const SCOPES: &str = "openid profile email offline_access";

fn client_id() -> String {
    match get_openai_client_id() {
        Ok(Some(value)) => value,
        Ok(None) => DEFAULT_CLIENT_ID.to_string(),
        Err(_) => DEFAULT_CLIENT_ID.to_string(),
    }
}

/// Codex API endpoint for ChatGPT OAuth (NOT the standard OpenAI API)
/// ChatGPT Plus/Pro subscriptions use the Codex backend, not api.openai.com
pub const API_ENDPOINT: &str = "https://chatgpt.com/backend-api";

/// Token response from OpenAI
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    scope: Option<String>,
    #[allow(dead_code)]
    id_token: Option<String>,
}

/// Error response from OpenAI
#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: String,
    error_description: Option<String>,
}

/// Build the OpenAI OAuth authorization URL
pub fn build_auth_url(pkce: &PkceChallenge, state: &str) -> String {
    let redirect_uri = OAuthProvider::OpenAI.redirect_uri();
    let client_id = client_id();

    let params = [
        ("response_type", "code"),
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("scope", SCOPES),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", "codex_cli_rs"),
    ];

    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    format!("{}?{}", AUTHORIZE_URL, query)
}

/// Exchange authorization code for tokens
pub async fn exchange_code(code: &str, code_verifier: &str) -> Result<StoredTokens, TokenError> {
    let redirect_uri = OAuthProvider::OpenAI.redirect_uri();
    let client_id = client_id();

    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", client_id.as_str()),
        ("code", code),
        ("code_verifier", code_verifier),
        ("redirect_uri", &redirect_uri),
    ];

    let client = reqwest::Client::new();
    let response = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| TokenError::RefreshFailed(e.to_string()))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| TokenError::RefreshFailed(e.to_string()))?;

    if !status.is_success() {
        let error: ErrorResponse = serde_json::from_str(&text).unwrap_or_else(|_| ErrorResponse {
            error: "unknown".to_string(),
            error_description: Some(text.clone()),
        });
        return Err(TokenError::RefreshFailed(
            error.error_description.unwrap_or(error.error),
        ));
    }

    let token_response: TokenResponse =
        serde_json::from_str(&text).map_err(|e| TokenError::SerializationError(e.to_string()))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Extract ChatGPT account ID from JWT
    let chatgpt_account_id = extract_chatgpt_account_id(&token_response.access_token);

    // Extract email from ID token if available
    let email = token_response
        .id_token
        .as_ref()
        .and_then(|id_token| super::tokens::parse_jwt_claims(id_token))
        .and_then(|claims| {
            claims
                .get("email")
                .and_then(|e| e.as_str())
                .map(String::from)
        });

    let tokens = StoredTokens {
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token.unwrap_or_default(),
        expires_at: now + token_response.expires_in,
        email,
        chatgpt_account_id,
    };

    // Store tokens
    store_tokens(OAuthProvider::OpenAI, &tokens)?;

    Ok(tokens)
}

/// Refresh the access token using the refresh token
pub async fn refresh_token(refresh_token: &str) -> Result<StoredTokens, TokenError> {
    let client_id = client_id();
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id.as_str()),
    ];

    let client = reqwest::Client::new();
    let response = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| TokenError::RefreshFailed(e.to_string()))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| TokenError::RefreshFailed(e.to_string()))?;

    if !status.is_success() {
        let error: ErrorResponse = serde_json::from_str(&text).unwrap_or_else(|_| ErrorResponse {
            error: "unknown".to_string(),
            error_description: Some(text.clone()),
        });
        return Err(TokenError::RefreshFailed(
            error.error_description.unwrap_or(error.error),
        ));
    }

    let token_response: TokenResponse =
        serde_json::from_str(&text).map_err(|e| TokenError::SerializationError(e.to_string()))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Extract ChatGPT account ID from new JWT
    let chatgpt_account_id = extract_chatgpt_account_id(&token_response.access_token);

    // Extract email from ID token if available
    let email = token_response
        .id_token
        .as_ref()
        .and_then(|id_token| super::tokens::parse_jwt_claims(id_token))
        .and_then(|claims| {
            claims
                .get("email")
                .and_then(|e| e.as_str())
                .map(String::from)
        });

    let tokens = StoredTokens {
        access_token: token_response.access_token,
        // Keep the original refresh token if not provided in response
        refresh_token: token_response
            .refresh_token
            .unwrap_or_else(|| refresh_token.to_string()),
        expires_at: now + token_response.expires_in,
        email,
        chatgpt_account_id,
    };

    // Store updated tokens
    store_tokens(OAuthProvider::OpenAI, &tokens)?;

    Ok(tokens)
}

/// Get request headers for OpenAI API calls
pub fn get_request_headers(tokens: &StoredTokens) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        format!("Bearer {}", tokens.access_token),
    );
    headers.insert("originator".to_string(), "codex_cli_rs".to_string());
    headers.insert(
        "OpenAI-Beta".to_string(),
        "responses=experimental".to_string(),
    );

    // Add ChatGPT account ID if available
    if let Some(ref account_id) = tokens.chatgpt_account_id {
        headers.insert("chatgpt-account-id".to_string(), account_id.clone());
    }

    headers
}
