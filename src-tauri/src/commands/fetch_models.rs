// Dynamic Model Fetching Commands
//
// Fetches available models from provider APIs:
// - OpenAI: GET /v1/models
// - Gemini: GET /v1beta/models
// - Anthropic: Mock (no API available)

use crate::settings::{self, LLMModel, LLMProvider};
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

/// Refresh models for ALL configured providers with API keys
/// Returns the complete updated list of models
#[tauri::command]
#[specta::specta]
pub async fn refresh_all_models(app: AppHandle) -> Result<Vec<LLMModel>, String> {
    let mut settings = settings::get_settings(&app);

    // Collect providers that have API keys configured
    let providers_to_fetch: Vec<LLMProvider> = settings
        .llm_providers
        .iter()
        .filter(|p| !p.api_key.is_empty())
        .cloned()
        .collect();

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

        // Convert fetched models to LLMModel format
        for fm in fetched {
            let model = LLMModel {
                id: format!("{}-{}", provider.id, fm.model_id.replace("/", "-")),
                provider_id: provider.id.clone(),
                model_id: fm.model_id,
                display_name: fm.display_name,
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
    match provider.id.as_str() {
        "openai" => fetch_openai_models(&provider.api_key, &provider.base_url).await,
        "gemini" => fetch_gemini_models(&provider.api_key).await,
        "anthropic" => Ok(get_anthropic_models()),
        _ => {
            // Custom provider - return empty (user must enter models manually)
            Ok(vec![])
        }
    }
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

/// Fetch models from Gemini API
async fn fetch_gemini_models(api_key: &str) -> Result<Vec<FetchedModel>, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Gemini models: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Gemini API error {}: {}", status, body));
    }

    let data: GeminiModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;

    // Filter to only models that support generateContent
    let models: Vec<FetchedModel> = data
        .models
        .into_iter()
        .filter(|m| {
            m.supported_generation_methods
                .contains(&"generateContent".to_string())
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

    Ok(models)
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
