use crate::oauth::{google, openai as openai_oauth, tokens::load_tokens, OAuthProvider};
use crate::settings::{AuthMethod, LLMProvider};
use async_openai::{config::OpenAIConfig, Client};

/// Get the API key to use for a provider (sync version, no auto-refresh)
///
/// For OAuth providers, this retrieves the access token from the local token store.
/// For API key providers, this returns the stored API key.
///
/// Returns an error if OAuth is selected but no valid token is available.
/// Note: This version does NOT auto-refresh expired tokens. Use `get_api_key_for_provider_async` for auto-refresh.
pub fn get_api_key_for_provider(provider: &LLMProvider) -> Result<String, String> {
    log::info!(
        "get_api_key_for_provider: id={}, auth_method={:?}, supports_oauth={}",
        provider.id,
        provider.auth_method,
        provider.supports_oauth
    );
    match provider.auth_method {
        AuthMethod::OAuth => {
            log::info!(
                "get_api_key_for_provider: using OAuth flow for {}",
                provider.id
            );

            // Determine which OAuth provider this is
            let oauth_provider = match OAuthProvider::from_str(&provider.id) {
                Some(p) => {
                    log::info!("get_api_key_for_provider: mapped to OAuth provider {:?}", p);
                    p
                }
                None => {
                    log::error!(
                        "get_api_key_for_provider: OAuth not supported for provider: {}",
                        provider.id
                    );
                    return Err(format!("OAuth not supported for provider: {}", provider.id));
                }
            };

            // Load tokens from local token store
            log::info!("get_api_key_for_provider: loading tokens from local token store...");
            let tokens = match load_tokens(oauth_provider) {
                Ok(t) => {
                    log::info!(
                        "get_api_key_for_provider: loaded tokens successfully (email={:?}, expires_at={}, token_length={})",
                        t.email,
                        t.expires_at,
                        t.access_token.len()
                    );
                    t
                }
                Err(e) => {
                    log::error!(
                        "get_api_key_for_provider: failed to load OAuth tokens: {}",
                        e
                    );
                    return Err(format!("Failed to load OAuth tokens: {}", e));
                }
            };

            // Check if token is expired - caller should use async version for auto-refresh
            if tokens.is_expired() {
                log::warn!(
                    "get_api_key_for_provider: OAuth token expired for {} (expires_at={}), use async version for auto-refresh",
                    provider.name,
                    tokens.expires_at
                );
                return Err(format!(
                    "OAuth token expired for {}. Please sign in again.",
                    provider.name
                ));
            }

            log::info!(
                "get_api_key_for_provider: returning valid OAuth token for {}",
                provider.id
            );
            Ok(tokens.access_token)
        }
        AuthMethod::ApiKey => {
            log::info!(
                "get_api_key_for_provider: using API key flow for {}",
                provider.id
            );
            if provider.api_key.is_empty() {
                log::error!(
                    "get_api_key_for_provider: no API key configured for {}",
                    provider.name
                );
                return Err(format!("No API key configured for {}", provider.name));
            }
            log::info!(
                "get_api_key_for_provider: returning API key for {} (length={})",
                provider.id,
                provider.api_key.len()
            );
            Ok(provider.api_key.clone())
        }
    }
}

/// Get the API key to use for a provider (async version with auto-refresh)
///
/// For OAuth providers, this retrieves the access token from the local token store
/// and automatically refreshes it if expired.
/// For API key providers, this returns the stored API key.
///
/// Returns an error if OAuth is selected but no valid token is available and refresh fails.
pub async fn get_api_key_for_provider_async(provider: &LLMProvider) -> Result<String, String> {
    log::info!(
        "get_api_key_for_provider_async: id={}, auth_method={:?}, supports_oauth={}",
        provider.id,
        provider.auth_method,
        provider.supports_oauth
    );
    match provider.auth_method {
        AuthMethod::OAuth => {
            log::info!(
                "get_api_key_for_provider_async: using OAuth flow for {}",
                provider.id
            );

            // Determine which OAuth provider this is
            let oauth_provider = match OAuthProvider::from_str(&provider.id) {
                Some(p) => {
                    log::info!(
                        "get_api_key_for_provider_async: mapped to OAuth provider {:?}",
                        p
                    );
                    p
                }
                None => {
                    log::error!(
                        "get_api_key_for_provider_async: OAuth not supported for provider: {}",
                        provider.id
                    );
                    return Err(format!("OAuth not supported for provider: {}", provider.id));
                }
            };

            // Load tokens from local token store
            log::info!("get_api_key_for_provider_async: loading tokens from local token store...");
            let mut tokens = match load_tokens(oauth_provider) {
                Ok(t) => {
                    log::info!(
                        "get_api_key_for_provider_async: loaded tokens successfully (email={:?}, expires_at={}, is_expired={})",
                        t.email,
                        t.expires_at,
                        t.is_expired()
                    );
                    t
                }
                Err(e) => {
                    log::error!(
                        "get_api_key_for_provider_async: failed to load OAuth tokens: {}",
                        e
                    );
                    return Err(format!("Failed to load OAuth tokens: {}", e));
                }
            };

            // Check if token is expired and try to refresh
            if tokens.is_expired() {
                log::info!(
                    "get_api_key_for_provider_async: OAuth token expired for {}, attempting refresh...",
                    provider.name
                );

                // Try to refresh the token
                let refresh_result = match oauth_provider {
                    OAuthProvider::Google => google::refresh_token(&tokens.refresh_token).await,
                    OAuthProvider::OpenAI => {
                        openai_oauth::refresh_token(&tokens.refresh_token).await
                    }
                };

                match refresh_result {
                    Ok(new_tokens) => {
                        log::info!(
                            "get_api_key_for_provider_async: successfully refreshed OAuth token for {} (new expires_at={})",
                            provider.name,
                            new_tokens.expires_at
                        );
                        tokens = new_tokens;
                    }
                    Err(e) => {
                        log::error!(
                            "get_api_key_for_provider_async: failed to refresh OAuth token for {}: {}",
                            provider.name,
                            e
                        );
                        return Err(format!(
                            "OAuth token expired for {} and refresh failed: {}. Please sign in again.",
                            provider.name, e
                        ));
                    }
                }
            }

            log::info!(
                "get_api_key_for_provider_async: returning valid OAuth token for {}",
                provider.id
            );
            Ok(tokens.access_token)
        }
        AuthMethod::ApiKey => {
            log::info!(
                "get_api_key_for_provider_async: using API key flow for {}",
                provider.id
            );
            if provider.api_key.is_empty() {
                log::error!(
                    "get_api_key_for_provider_async: no API key configured for {}",
                    provider.name
                );
                return Err(format!("No API key configured for {}", provider.name));
            }
            log::info!(
                "get_api_key_for_provider_async: returning API key for {} (length={})",
                provider.id,
                provider.api_key.len()
            );
            Ok(provider.api_key.clone())
        }
    }
}

/// Create an OpenAI-compatible client configured for the given provider
pub fn create_client(
    provider: &LLMProvider,
    api_key: String,
) -> Result<Client<OpenAIConfig>, String> {
    let base_url = provider.base_url.trim_end_matches('/');
    let config = OpenAIConfig::new()
        .with_api_base(base_url)
        .with_api_key(api_key.clone());

    // Create client with provider-specific headers
    let client = if provider.id == "anthropic" {
        // Anthropic requires a version header
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "anthropic-version",
            reqwest::header::HeaderValue::from_static("2023-06-01"),
        );

        let http_client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

        Client::with_config(config).with_http_client(http_client)
    } else if provider.auth_method == AuthMethod::OAuth {
        // OAuth providers need specific headers
        create_oauth_client(provider, &config, &api_key)?
    } else {
        Client::with_config(config)
    };

    Ok(client)
}

/// Create an OpenAI client with OAuth-specific headers
fn create_oauth_client(
    provider: &LLMProvider,
    config: &OpenAIConfig,
    access_token: &str,
) -> Result<Client<OpenAIConfig>, String> {
    let oauth_provider = OAuthProvider::from_str(&provider.id).ok_or_else(|| {
        format!(
            "OAuth not supported for provider: {} (this should not happen)",
            provider.id
        )
    })?;

    // Get provider-specific headers
    let headers_map = match oauth_provider {
        OAuthProvider::Google => google::get_request_headers(access_token),
        OAuthProvider::OpenAI => {
            // For OpenAI, we need to load the full tokens to get the account ID
            let tokens = load_tokens(oauth_provider)
                .map_err(|e| format!("Failed to load OAuth tokens for headers: {}", e))?;
            openai_oauth::get_request_headers(&tokens)
        }
    };

    // Convert HashMap to reqwest HeaderMap
    let mut headers = reqwest::header::HeaderMap::new();
    for (key, value) in headers_map {
        let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
            .map_err(|e| format!("Invalid header name '{}': {}", key, e))?;
        let header_value = reqwest::header::HeaderValue::from_str(&value)
            .map_err(|e| format!("Invalid header value for '{}': {}", key, e))?;
        headers.insert(header_name, header_value);
    }

    let http_client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    Ok(Client::with_config(config.clone()).with_http_client(http_client))
}
