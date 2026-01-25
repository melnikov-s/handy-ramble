// Dynamic Model Fetching Commands
//
// Fetches available models from provider APIs:
// - OpenAI: GET /v1/models
// - Gemini: GET /v1beta/models
// - Anthropic: Mock (no API available)

use crate::llm_client::get_api_key_for_provider;
use crate::settings::{self, AuthMethod, LLMModel, LLMProvider};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

/// Fetched model from an API (normalized format)
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct FetchedModel {
    pub model_id: String,
    pub display_name: String,
    pub supports_vision: bool,
}

// === OpenAI Response Types ===
#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
    #[allow(dead_code)]
    owned_by: String,
}

// === Gemini Response Types ===
#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiModel {
    name: String,
    display_name: Option<String>,
    #[serde(default)]
    supported_generation_methods: Vec<String>,
}

/// Refresh models for ALL configured providers with API keys or OAuth
/// Returns the complete updated list of models
#[tauri::command]
#[specta::specta]
pub async fn refresh_all_models(app: AppHandle) -> Result<Vec<LLMModel>, String> {
    let mut settings = settings::get_settings(&app);

    // Log all providers for debugging
    log::debug!(
        "refresh_all_models: checking {} providers",
        settings.llm_providers.len()
    );
    for p in &settings.llm_providers {
        log::debug!(
            "  Provider: id={}, name={}, auth_method={:?}, supports_oauth={}, has_api_key={}",
            p.id,
            p.name,
            p.auth_method,
            p.supports_oauth,
            !p.api_key.is_empty()
        );
    }

    // Collect providers that have API keys configured OR are authenticated via OAuth
    let providers_to_fetch: Vec<LLMProvider> = settings
        .llm_providers
        .iter()
        .filter(|p| {
            // Check if provider has API key OR is OAuth authenticated
            let should_fetch =
                !p.api_key.is_empty() || (p.auth_method == AuthMethod::OAuth && p.supports_oauth);
            log::debug!(
                "  Filter {}: api_key={}, auth_method={:?}, supports_oauth={} => {}",
                p.id,
                !p.api_key.is_empty(),
                p.auth_method,
                p.supports_oauth,
                should_fetch
            );
            should_fetch
        })
        .cloned()
        .collect();

    log::debug!(
        "refresh_all_models: will fetch from {} providers",
        providers_to_fetch.len()
    );

    if providers_to_fetch.is_empty() {
        return Err("No providers with API keys configured".to_string());
    }

    // Remove existing models for providers we're refreshing
    let provider_ids: Vec<String> = providers_to_fetch.iter().map(|p| p.id.clone()).collect();
    settings
        .llm_models
        .retain(|m| !provider_ids.contains(&m.provider_id));

    // Fetch models for each provider
    for provider in providers_to_fetch {
        let fetched = fetch_models_for_provider(&provider).await?;

        // Determine if this is an OAuth provider (for display name suffix)
        let is_oauth = provider.auth_method == AuthMethod::OAuth;

        // Convert fetched models to LLMModel format
        for fm in fetched {
            // Add "(OAuth)" suffix to display name for OAuth provider models
            let display_name = if is_oauth {
                format!("{} (OAuth)", fm.display_name)
            } else {
                fm.display_name
            };

            let model = LLMModel {
                id: format!("{}-{}", provider.id, fm.model_id.replace("/", "-")),
                provider_id: provider.id.clone(),
                model_id: fm.model_id,
                display_name,
                supports_vision: fm.supports_vision,
                enabled: true, // Enable all fetched models by default
            };
            settings.llm_models.push(model);
        }
    }

    // Save updated settings
    settings::write_settings(&app, settings.clone());

    Ok(settings.llm_models)
}

/// Fetch models for a single provider (internal helper)
async fn fetch_models_for_provider(provider: &LLMProvider) -> Result<Vec<FetchedModel>, String> {
    log::info!(
        "fetch_models_for_provider: starting for provider id={}, name={}, auth_method={:?}",
        provider.id,
        provider.name,
        provider.auth_method
    );

    // For OAuth providers, use hardcoded models (API fetching requires scopes we don't have)
    if provider.auth_method == AuthMethod::OAuth {
        log::info!(
            "fetch_models_for_provider: using hardcoded models for OAuth provider {}",
            provider.id
        );
        return match provider.id.as_str() {
            "openai_oauth" => Ok(get_openai_oauth_models()),
            "gemini_oauth" => Ok(get_gemini_oauth_models()),
            _ => {
                log::warn!(
                    "fetch_models_for_provider: unknown OAuth provider {}, returning empty list",
                    provider.id
                );
                Ok(vec![])
            }
        };
    }

    // For API key providers, fetch from the API
    let api_key = match get_api_key_for_provider(provider) {
        Ok(key) => {
            log::info!(
                "fetch_models_for_provider: got API key (length={})",
                key.len()
            );
            key
        }
        Err(e) => {
            log::error!("fetch_models_for_provider: failed to get API key: {}", e);
            return Err(e);
        }
    };

    // Map provider IDs to their fetch functions
    let result = match provider.id.as_str() {
        "openai" => {
            log::info!(
                "fetch_models_for_provider: fetching OpenAI models from {}",
                provider.base_url
            );
            fetch_openai_models(&api_key, &provider.base_url).await
        }
        "gemini" => {
            log::info!("fetch_models_for_provider: fetching Gemini models with API key");
            fetch_gemini_models(&api_key).await
        }
        "anthropic" => {
            log::info!("fetch_models_for_provider: returning hardcoded Anthropic models");
            Ok(get_anthropic_models())
        }
        _ => {
            log::info!(
                "fetch_models_for_provider: custom provider {}, returning empty list",
                provider.id
            );
            Ok(vec![])
        }
    };

    match &result {
        Ok(models) => log::info!(
            "fetch_models_for_provider: successfully fetched {} models for {}",
            models.len(),
            provider.id
        ),
        Err(e) => log::error!(
            "fetch_models_for_provider: failed to fetch models for {}: {}",
            provider.id,
            e
        ),
    }

    result
}

/// Fetch models from OpenAI API
async fn fetch_openai_models(api_key: &str, base_url: &str) -> Result<Vec<FetchedModel>, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/models", base_url);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch OpenAI models: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error {}: {}", status, body));
    }

    let data: OpenAIModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

    // Filter to only chat models (gpt-*, o1-*, o3-*, chatgpt-*)
    let models: Vec<FetchedModel> = data
        .data
        .into_iter()
        .filter(|m| {
            let id = m.id.as_str();
            id.starts_with("gpt-")
                || id.starts_with("o1")
                || id.starts_with("o3")
                || id.starts_with("chatgpt-")
        })
        .map(|m| {
            let supports_vision = m.id.contains("gpt-4") || m.id.contains("gpt-4o") || m.id == "o1";
            FetchedModel {
                display_name: m.id.clone(),
                model_id: m.id,
                supports_vision,
            }
        })
        .collect();

    Ok(models)
}

/// Fetch models from Gemini API (OAuth-aware)
async fn fetch_gemini_models_oauth_aware(
    api_key_or_token: &str,
    use_oauth: bool,
) -> Result<Vec<FetchedModel>, String> {
    log::info!(
        "fetch_gemini_models_oauth_aware: starting (use_oauth={}, token_length={})",
        use_oauth,
        api_key_or_token.len()
    );

    let client = reqwest::Client::new();

    // Build request based on auth method
    let url = if use_oauth {
        "https://generativelanguage.googleapis.com/v1beta/models".to_string()
    } else {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={}",
            api_key_or_token
        )
    };

    log::info!(
        "fetch_gemini_models_oauth_aware: requesting URL: {}",
        if use_oauth {
            &url
        } else {
            "https://generativelanguage.googleapis.com/v1beta/models?key=***"
        }
    );

    let request = if use_oauth {
        log::info!("fetch_gemini_models_oauth_aware: using Bearer auth header");
        client.get(&url).bearer_auth(api_key_or_token)
    } else {
        client.get(&url)
    };

    log::info!("fetch_gemini_models_oauth_aware: sending request...");
    let response = match request.send().await {
        Ok(resp) => {
            log::info!(
                "fetch_gemini_models_oauth_aware: got response, status={}",
                resp.status()
            );
            resp
        }
        Err(e) => {
            log::error!("fetch_gemini_models_oauth_aware: request failed: {}", e);
            return Err(format!("Failed to fetch Gemini models: {}", e));
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        log::error!(
            "fetch_gemini_models_oauth_aware: API error {}: {}",
            status,
            body
        );
        return Err(format!("Gemini API error {}: {}", status, body));
    }

    let body_text = response.text().await.map_err(|e| {
        log::error!(
            "fetch_gemini_models_oauth_aware: failed to read response body: {}",
            e
        );
        format!("Failed to read Gemini response: {}", e)
    })?;

    log::debug!(
        "fetch_gemini_models_oauth_aware: response body (first 500 chars): {}",
        &body_text.chars().take(500).collect::<String>()
    );

    let data: GeminiModelsResponse = serde_json::from_str(&body_text).map_err(|e| {
        log::error!(
            "fetch_gemini_models_oauth_aware: failed to parse response: {}",
            e
        );
        format!("Failed to parse Gemini response: {}", e)
    })?;

    log::info!(
        "fetch_gemini_models_oauth_aware: parsed {} models from response",
        data.models.len()
    );

    // Filter to only models that support generateContent
    let models: Vec<FetchedModel> = data
        .models
        .into_iter()
        .filter(|m| {
            let supports = m
                .supported_generation_methods
                .contains(&"generateContent".to_string());
            if !supports {
                log::debug!(
                    "fetch_gemini_models_oauth_aware: filtering out model {} (no generateContent support)",
                    m.name
                );
            }
            supports
        })
        .map(|m| {
            // Extract model ID from "models/gemini-1.5-flash" format
            let model_id = m
                .name
                .strip_prefix("models/")
                .unwrap_or(&m.name)
                .to_string();
            let display_name = m.display_name.unwrap_or(model_id.clone());
            FetchedModel {
                model_id,
                display_name,
                supports_vision: true, // All Gemini models support vision
            }
        })
        .collect();

    log::info!(
        "fetch_gemini_models_oauth_aware: returning {} models after filtering",
        models.len()
    );

    Ok(models)
}

/// Fetch models from Gemini API (legacy, API key only)
async fn fetch_gemini_models(api_key: &str) -> Result<Vec<FetchedModel>, String> {
    fetch_gemini_models_oauth_aware(api_key, false).await
}

/// Get hardcoded Anthropic models (no API available)
fn get_anthropic_models() -> Vec<FetchedModel> {
    vec![
        FetchedModel {
            model_id: "claude-opus-4-5-20251101".to_string(),
            display_name: "Claude Opus 4.5".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "claude-opus-4-20250514".to_string(),
            display_name: "Claude Opus 4".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "claude-sonnet-4-5-20250929".to_string(),
            display_name: "Claude Sonnet 4.5".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "claude-sonnet-4-20250514".to_string(),
            display_name: "Claude Sonnet 4".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "claude-haiku-4-5-20251001".to_string(),
            display_name: "Claude Haiku 4.5".to_string(),
            supports_vision: true,
        },
    ]
}

/// Get hardcoded OpenAI models for OAuth (API fetching not available with OAuth scopes)
/// These are the actual Codex backend model names supported by ChatGPT Plus/Pro
/// Models and their supported reasoning efforts:
/// - gpt-5.2: none/low/medium/high/xhigh
/// - gpt-5.2-codex: low/medium/high/xhigh
/// - gpt-5.1-codex-max: low/medium/high/xhigh
/// - gpt-5.1-codex: low/medium/high
/// - gpt-5.1-codex-mini: medium/high
/// - gpt-5.1: none/low/medium/high
fn get_openai_oauth_models() -> Vec<FetchedModel> {
    vec![
        FetchedModel {
            model_id: "gpt-5.2".to_string(),
            display_name: "GPT-5.2".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "gpt-5.2-codex".to_string(),
            display_name: "GPT-5.2 Codex".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "gpt-5.1-codex-max".to_string(),
            display_name: "GPT-5.1 Codex Max".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "gpt-5.1-codex".to_string(),
            display_name: "GPT-5.1 Codex".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "gpt-5.1-codex-mini".to_string(),
            display_name: "GPT-5.1 Codex Mini".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "gpt-5.1".to_string(),
            display_name: "GPT-5.1".to_string(),
            supports_vision: true,
        },
    ]
}

/// Get hardcoded Gemini models for OAuth (API fetching requires scopes we don't have)
fn get_gemini_oauth_models() -> Vec<FetchedModel> {
    vec![
        FetchedModel {
            model_id: "gemini-2.5-flash".to_string(),
            display_name: "Gemini 2.5 Flash".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "gemini-2.5-pro".to_string(),
            display_name: "Gemini 2.5 Pro".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "gemini-3-flash-preview".to_string(),
            display_name: "Gemini 3 Flash (Preview)".to_string(),
            supports_vision: true,
        },
        FetchedModel {
            model_id: "gemini-3-pro-preview".to_string(),
            display_name: "Gemini 3 Pro (Preview)".to_string(),
            supports_vision: true,
        },
    ]
}
