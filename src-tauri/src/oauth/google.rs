//! Google OAuth implementation for Gemini API
//!
//! Uses the Gemini CLI OAuth credentials for consumer Google accounts.
//! OAuth users access Gemini through the Cloud Code Assist API, not the
//! standard Generative Language API.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

use super::pkce::PkceChallenge;
use super::tokens::{store_tokens, StoredTokens, TokenError};
use super::OAuthProvider;

/// Google OAuth configuration (Gemini CLI credentials)
pub const CLIENT_ID: &str =
    "REDACTED_GOOGLE_OAUTH_CLIENT_ID";
pub const CLIENT_SECRET: &str = "REDACTED_GOOGLE_OAUTH_CLIENT_SECRET";
pub const AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
// Scopes for Gemini API access via OAuth (matching Gemini CLI)
// See: https://github.com/google-gemini/gemini-cli/blob/main/packages/core/src/code_assist/oauth2.ts
pub const SCOPES: &str = "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile";

/// Code Assist API endpoint for OAuth users (NOT generativelanguage.googleapis.com)
pub const CODE_ASSIST_ENDPOINT: &str = "https://cloudcode-pa.googleapis.com";

/// Code Assist API version
pub const CODE_ASSIST_API_VERSION: &str = "v1internal";

/// Cached project ID for the current session
static CACHED_PROJECT_ID: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn get_project_cache() -> &'static Mutex<Option<String>> {
    CACHED_PROJECT_ID.get_or_init(|| Mutex::new(None))
}

/// Token response from Google
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    token_type: String,
    #[allow(dead_code)]
    scope: Option<String>,
}

/// User info response from Google
#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    email: Option<String>,
    #[allow(dead_code)]
    name: Option<String>,
}

/// Error response from Google
#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: String,
    error_description: Option<String>,
}

/// Build the Google OAuth authorization URL
pub fn build_auth_url(pkce: &PkceChallenge, state: &str) -> String {
    let redirect_uri = OAuthProvider::Google.redirect_uri();

    // Encode state with verifier for token exchange
    let state_data = serde_json::json!({
        "state": state,
        "verifier": pkce.verifier
    });
    let encoded_state = URL_SAFE_NO_PAD.encode(state_data.to_string().as_bytes());

    let params = [
        ("client_id", CLIENT_ID),
        ("response_type", "code"),
        ("redirect_uri", &redirect_uri),
        ("scope", SCOPES),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("state", &encoded_state),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ];

    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    format!("{}?{}", AUTHORIZE_URL, query)
}

/// Decode the state parameter to extract the original state and verifier
pub fn decode_state(encoded_state: &str) -> Option<(String, String)> {
    let decoded = URL_SAFE_NO_PAD.decode(encoded_state).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    let state = json.get("state")?.as_str()?.to_string();
    let verifier = json.get("verifier")?.as_str()?.to_string();
    Some((state, verifier))
}

/// Exchange authorization code for tokens
pub async fn exchange_code(code: &str, code_verifier: &str) -> Result<StoredTokens, TokenError> {
    let redirect_uri = OAuthProvider::Google.redirect_uri();

    log::info!(
        "Google OAuth: exchanging code (length={}) with verifier (length={})",
        code.len(),
        code_verifier.len()
    );

    let params = [
        ("client_id", CLIENT_ID),
        ("client_secret", CLIENT_SECRET),
        ("code", code),
        ("grant_type", "authorization_code"),
        ("redirect_uri", &redirect_uri),
        ("code_verifier", code_verifier),
    ];

    let client = reqwest::Client::new();
    let response = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            log::error!("Google OAuth: token request failed: {}", e);
            TokenError::RefreshFailed(e.to_string())
        })?;

    let status = response.status();
    log::info!("Google OAuth: token response status: {}", status);

    let text = response
        .text()
        .await
        .map_err(|e| TokenError::RefreshFailed(e.to_string()))?;

    if !status.is_success() {
        log::error!(
            "Google OAuth: token exchange failed with status {}: {}",
            status,
            text
        );
        let error: ErrorResponse = serde_json::from_str(&text).unwrap_or_else(|_| ErrorResponse {
            error: "unknown".to_string(),
            error_description: Some(text.clone()),
        });
        return Err(TokenError::RefreshFailed(
            error.error_description.unwrap_or(error.error),
        ));
    }

    log::info!("Google OAuth: token exchange successful, parsing response");
    let token_response: TokenResponse =
        serde_json::from_str(&text).map_err(|e| TokenError::SerializationError(e.to_string()))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Fetch user email
    log::info!("Google OAuth: fetching user email");
    let email = fetch_user_email(&token_response.access_token).await.ok();
    log::info!("Google OAuth: user email fetched: {:?}", email);

    let tokens = StoredTokens {
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token.unwrap_or_default(),
        expires_at: now + token_response.expires_in,
        email,
        chatgpt_account_id: None, // Not applicable for Google
    };

    // Store tokens
    store_tokens(OAuthProvider::Google, &tokens)?;

    Ok(tokens)
}

/// Refresh the access token using the refresh token
pub async fn refresh_token(refresh_token: &str) -> Result<StoredTokens, TokenError> {
    let params = [
        ("client_id", CLIENT_ID),
        ("client_secret", CLIENT_SECRET),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
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

    // Fetch user email
    let email = fetch_user_email(&token_response.access_token).await.ok();

    let tokens = StoredTokens {
        access_token: token_response.access_token,
        // Keep the original refresh token if not provided in response
        refresh_token: token_response
            .refresh_token
            .unwrap_or_else(|| refresh_token.to_string()),
        expires_at: now + token_response.expires_in,
        email,
        chatgpt_account_id: None,
    };

    // Store updated tokens
    store_tokens(OAuthProvider::Google, &tokens)?;

    Ok(tokens)
}

/// Fetch user email from Google's userinfo endpoint
async fn fetch_user_email(access_token: &str) -> Result<String, TokenError> {
    let client = reqwest::Client::new();
    let response = client
        .get(USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| TokenError::RefreshFailed(e.to_string()))?;

    let user_info: UserInfoResponse = response
        .json()
        .await
        .map_err(|e| TokenError::SerializationError(e.to_string()))?;

    user_info
        .email
        .ok_or_else(|| TokenError::RefreshFailed("No email in user info".to_string()))
}

/// Get request headers for Google Code Assist API calls
pub fn get_request_headers(access_token: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        format!("Bearer {}", access_token),
    );
    headers.insert(
        "User-Agent".to_string(),
        "google-api-nodejs-client/9.15.1".to_string(),
    );
    headers.insert(
        "X-Goog-Api-Client".to_string(),
        "gl-node/22.17.0".to_string(),
    );
    headers.insert(
        "Client-Metadata".to_string(),
        "ideType=IDE_UNSPECIFIED,platform=PLATFORM_UNSPECIFIED,pluginType=GEMINI".to_string(),
    );
    headers
}

/// Response from loadCodeAssist endpoint
#[derive(Debug, Deserialize)]
struct LoadCodeAssistResponse {
    #[serde(rename = "cloudaicompanionProject")]
    cloudaicompanion_project: Option<String>,
    #[serde(rename = "currentTier")]
    current_tier: Option<TierInfo>,
    #[serde(rename = "allowedTiers")]
    allowed_tiers: Option<Vec<TierInfo>>,
}

#[derive(Debug, Deserialize)]
struct TierInfo {
    id: Option<String>,
    #[serde(rename = "isDefault")]
    is_default: Option<bool>,
}

/// Response from onboardUser endpoint
#[derive(Debug, Deserialize)]
struct OnboardUserResponse {
    done: Option<bool>,
    response: Option<OnboardResponseInner>,
}

#[derive(Debug, Deserialize)]
struct OnboardResponseInner {
    #[serde(rename = "cloudaicompanionProject")]
    cloudaicompanion_project: Option<ProjectInfo>,
}

#[derive(Debug, Deserialize)]
struct ProjectInfo {
    id: Option<String>,
}

/// Load or provision a Google Cloud project for Code Assist API access
/// Returns the project ID to use for API calls
pub async fn ensure_project_id(access_token: &str) -> Result<String, TokenError> {
    // Check cache first
    {
        let cache = get_project_cache().lock().await;
        if let Some(ref project_id) = *cache {
            log::debug!("Using cached project ID: {}", project_id);
            return Ok(project_id.clone());
        }
    }

    log::info!("Loading Code Assist project...");

    // Try to load existing project
    let project_id = match load_code_assist_project(access_token).await? {
        Some(id) => {
            log::info!("Found existing Code Assist project: {}", id);
            id
        }
        None => {
            // Need to onboard user to get a project
            log::info!("No existing project, onboarding user...");
            onboard_user(access_token).await?
        }
    };

    // Cache the project ID
    {
        let mut cache = get_project_cache().lock().await;
        *cache = Some(project_id.clone());
    }

    Ok(project_id)
}

/// Clear the cached project ID (e.g., on logout)
pub async fn clear_project_cache() {
    let mut cache = get_project_cache().lock().await;
    *cache = None;
    log::debug!("Cleared project cache");
}

/// Call loadCodeAssist endpoint to check for existing project
async fn load_code_assist_project(access_token: &str) -> Result<Option<String>, TokenError> {
    let url = format!(
        "{}/{}:loadCodeAssist",
        CODE_ASSIST_ENDPOINT, CODE_ASSIST_API_VERSION
    );

    let metadata = serde_json::json!({
        "ideType": "IDE_UNSPECIFIED",
        "platform": "PLATFORM_UNSPECIFIED",
        "pluginType": "GEMINI"
    });

    let request_body = serde_json::json!({
        "metadata": metadata
    });

    log::info!("loadCodeAssist request URL: {}", url);

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "google-api-nodejs-client/9.15.1")
        .header("X-Goog-Api-Client", "gl-node/22.17.0")
        .header(
            "Client-Metadata",
            "ideType=IDE_UNSPECIFIED,platform=PLATFORM_UNSPECIFIED,pluginType=GEMINI",
        )
        .json(&request_body)
        .send()
        .await
        .map_err(|e| TokenError::RefreshFailed(format!("loadCodeAssist request failed: {}", e)))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    log::info!("loadCodeAssist response: status={}, body={}", status, text);

    if !status.is_success() {
        log::warn!("loadCodeAssist failed with status {}: {}", status, text);
        // Return None to trigger onboarding instead of failing
        return Ok(None);
    }

    let payload: LoadCodeAssistResponse = serde_json::from_str(&text).map_err(|e| {
        TokenError::SerializationError(format!("Failed to parse loadCodeAssist response: {}", e))
    })?;

    log::info!(
        "loadCodeAssist parsed project: {:?}",
        payload.cloudaicompanion_project
    );

    Ok(payload.cloudaicompanion_project)
}

/// Onboard user to get a Code Assist project
async fn onboard_user(access_token: &str) -> Result<String, TokenError> {
    let url = format!(
        "{}/{}:onboardUser",
        CODE_ASSIST_ENDPOINT, CODE_ASSIST_API_VERSION
    );

    let metadata = serde_json::json!({
        "ideType": "IDE_UNSPECIFIED",
        "platform": "PLATFORM_UNSPECIFIED",
        "pluginType": "GEMINI"
    });

    // Use FREE tier for consumer accounts
    let request_body = serde_json::json!({
        "tierId": "FREE",
        "metadata": metadata
    });

    let client = reqwest::Client::new();

    // Onboarding can take multiple attempts as the project is being provisioned
    let max_attempts = 10;
    let delay_ms = 5000;

    for attempt in 0..max_attempts {
        log::info!("Onboarding attempt {} of {}", attempt + 1, max_attempts);

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "google-api-nodejs-client/9.15.1")
            .header("X-Goog-Api-Client", "gl-node/22.17.0")
            .header(
                "Client-Metadata",
                "ideType=IDE_UNSPECIFIED,platform=PLATFORM_UNSPECIFIED,pluginType=GEMINI",
            )
            .json(&request_body)
            .send()
            .await
            .map_err(|e| TokenError::RefreshFailed(format!("onboardUser request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            log::error!("onboardUser failed with status {}: {}", status, text);
            return Err(TokenError::RefreshFailed(format!(
                "onboardUser failed: {} - {}",
                status, text
            )));
        }

        let payload: OnboardUserResponse = response.json().await.map_err(|e| {
            TokenError::SerializationError(format!("Failed to parse onboardUser response: {}", e))
        })?;

        if payload.done == Some(true) {
            if let Some(project_id) = payload
                .response
                .and_then(|r| r.cloudaicompanion_project)
                .and_then(|p| p.id)
            {
                log::info!("User onboarding complete, project ID: {}", project_id);
                return Ok(project_id);
            }
        }

        // Wait before retrying
        if attempt < max_attempts - 1 {
            log::debug!("Onboarding not complete, waiting {}ms...", delay_ms);
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        }
    }

    Err(TokenError::RefreshFailed(
        "User onboarding timed out - please try again or manually enable Gemini for Google Cloud API in your Google Cloud Console".to_string(),
    ))
}

/// Build the Code Assist API URL for a given action
pub fn build_code_assist_url(action: &str, streaming: bool) -> String {
    let base = format!(
        "{}/{}:{}",
        CODE_ASSIST_ENDPOINT, CODE_ASSIST_API_VERSION, action
    );
    if streaming {
        format!("{}?alt=sse", base)
    } else {
        base
    }
}

/// Wrap a Gemini API request body for the Code Assist API
pub fn wrap_request_for_code_assist(
    project_id: &str,
    model: &str,
    request_body: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "project": project_id,
        "model": model,
        "request": request_body
    })
}

/// Unwrap a Code Assist API response to get the standard Gemini response
pub fn unwrap_code_assist_response(
    response: serde_json::Value,
) -> Result<serde_json::Value, String> {
    // Code Assist wraps the response in a "response" field
    if let Some(inner) = response.get("response") {
        Ok(inner.clone())
    } else {
        // If no wrapper, return as-is (might be an error response)
        Ok(response)
    }
}
