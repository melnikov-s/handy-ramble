use crate::llm_client::{create_client, get_api_key_for_provider_async};
use crate::settings::{get_settings, get_system_prompt_content};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Serialize, Deserialize, specta::Type, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub images: Option<Vec<String>>, // Base64 encoded images
}

#[derive(Debug, Serialize, Deserialize, specta::Type, Clone)]
pub struct GroundingChunk {
    pub uri: Option<String>,
    pub title: Option<String>,
    // we can add content later if needed
}

#[derive(Debug, Serialize, Deserialize, specta::Type, Clone)]
pub struct GroundingMetadata {
    pub search_entry_point: Option<String>,
    pub chunks: Vec<GroundingChunk>,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct ChatResponse {
    pub content: String,
    pub grounding_metadata: Option<GroundingMetadata>,
}

/// Send a chat completion request to the configured LLM provider
///
/// # Arguments
/// * `model_id` - Optional model ID to use. Falls back to `default_chat_model_id` if not provided.
/// * `enable_grounding` - Whether to enable web search grounding (supported for Gemini and Anthropic)
#[tauri::command]
#[specta::specta]
pub async fn chat_completion(
    app: AppHandle,
    messages: Vec<ChatMessage>,
    model_id: Option<String>,
    enable_grounding: bool,
) -> Result<ChatResponse, String> {
    let settings = get_settings(&app);

    // Determine which model to use
    let model_id = model_id
        .or(settings.default_chat_model_id.clone())
        .ok_or_else(|| "No model specified and no default chat model configured".to_string())?;

    // Look up the model
    let model = settings
        .get_model(&model_id)
        .ok_or_else(|| format!("Model '{}' not found in configured models", model_id))?;

    // Look up the provider for this model
    let provider = settings.get_provider(&model.provider_id).ok_or_else(|| {
        format!(
            "Provider '{}' not found for model '{}'",
            model.provider_id, model_id
        )
    })?;

    // Get API key or OAuth token using the OAuth-aware helper (with auto-refresh)
    let api_key = get_api_key_for_provider_async(provider).await?;

    // Use Gemini native API for all Gemini models (supports grounding)
    // Handle both "gemini" (API key) and "gemini_oauth" (OAuth) providers
    if provider.id == "gemini" || provider.id == "gemini_oauth" {
        return chat_completion_gemini_native(
            &app,
            provider,
            &api_key,
            &model.model_id,
            messages,
            enable_grounding,
        )
        .await;
    }

    // Use Anthropic native API for Claude models (supports web search)
    if provider.id == "anthropic" {
        return chat_completion_anthropic_native(
            &app,
            provider,
            &api_key,
            &model.model_id,
            messages,
            enable_grounding,
        )
        .await;
    }

    // Use Codex API for OpenAI OAuth (ChatGPT Plus/Pro subscription)
    if provider.id == "openai_oauth" {
        return chat_completion_openai_codex(&app, &api_key, &model.model_id, messages).await;
    }

    // Create the client
    let client = create_client(provider, api_key.clone())?;

    // Convert messages to OpenAI format
    let mut openai_messages: Vec<ChatCompletionRequestMessage> = Vec::new();

    // Inject system prompt if configured
    if let Some(system_prompt) = get_system_prompt_content(&app) {
        let system_msg = ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt)
            .build()
            .map_err(|e| e.to_string())?;
        openai_messages.push(system_msg.into());
    }

    for msg in messages {
        let openai_msg = match msg.role.as_str() {
            "system" => ChatCompletionRequestSystemMessageArgs::default()
                .content(msg.content)
                .build()
                .map_err(|e| e.to_string())?
                .into(),
            "user" => {
                if let Some(images) = msg.images {
                    if !images.is_empty() {
                        use async_openai::types::{
                            ChatCompletionRequestMessageContentPartImageArgs,
                            ChatCompletionRequestMessageContentPartTextArgs,
                            ChatCompletionRequestUserMessageContentPart, ImageUrlArgs,
                        };

                        let mut parts: Vec<ChatCompletionRequestUserMessageContentPart> =
                            Vec::new();

                        // Add text part
                        parts.push(
                            ChatCompletionRequestMessageContentPartTextArgs::default()
                                .text(msg.content)
                                .build()
                                .map_err(|e| e.to_string())?
                                .into(),
                        );

                        // Add image parts
                        for base64_image in images {
                            parts.push(
                                ChatCompletionRequestMessageContentPartImageArgs::default()
                                    .image_url(
                                        ImageUrlArgs::default()
                                            .url(format!("data:image/png;base64,{}", base64_image))
                                            .build()
                                            .map_err(|e| e.to_string())?,
                                    )
                                    .build()
                                    .map_err(|e| e.to_string())?
                                    .into(),
                            );
                        }

                        ChatCompletionRequestUserMessageArgs::default()
                            .content(parts)
                            .build()
                            .map_err(|e| e.to_string())?
                            .into()
                    } else {
                        ChatCompletionRequestUserMessageArgs::default()
                            .content(msg.content)
                            .build()
                            .map_err(|e| e.to_string())?
                            .into()
                    }
                } else {
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(msg.content)
                        .build()
                        .map_err(|e| e.to_string())?
                        .into()
                }
            }
            "assistant" => {
                // For assistant messages, we'll treat them as user context for now
                ChatCompletionRequestUserMessageArgs::default()
                    .content(format!("Previous assistant response: {}", msg.content))
                    .build()
                    .map_err(|e| e.to_string())?
                    .into()
            }
            _ => continue,
        };
        openai_messages.push(openai_msg);
    }

    // Build the request using the model's API model_id
    let request = CreateChatCompletionRequestArgs::default()
        .model(&model.model_id)
        .messages(openai_messages)
        .build()
        .map_err(|e| format!("Failed to build request: {}", e))?;

    // Make the API call
    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|e| format!("Chat completion failed: {}", e))?;

    // Extract the response content
    let content = response
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone())
        .ok_or_else(|| "No response content".to_string())?;

    Ok(ChatResponse {
        content,
        grounding_metadata: None,
    })
}

/// Native Gemini API call for search grounding
async fn chat_completion_gemini_native(
    app: &AppHandle,
    provider: &crate::settings::LLMProvider,
    api_key: &str,
    model_id: &str,
    messages: Vec<ChatMessage>,
    enable_grounding: bool,
) -> Result<ChatResponse, String> {
    use crate::settings::AuthMethod;

    let mut contents = Vec::new();

    // Inject system prompt as first user message if configured
    if let Some(system_prompt) = get_system_prompt_content(app) {
        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{ "text": system_prompt }]
        }));
        // Add a model acknowledgment to maintain conversation flow
        contents.push(serde_json::json!({
            "role": "model",
            "parts": [{ "text": "Understood. I will follow these instructions." }]
        }));
    }

    // When grounding is enabled, add an instruction to always use Google Search
    if enable_grounding {
        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{ "text": "IMPORTANT: You MUST use Google Search to find current, accurate information before responding. Always search the web first." }]
        }));
        contents.push(serde_json::json!({
            "role": "model",
            "parts": [{ "text": "Understood. I will use Google Search to find current information." }]
        }));
    }

    for msg in messages {
        let role = if msg.role == "assistant" {
            "model"
        } else {
            "user"
        };

        let mut parts = Vec::new();

        // Add text content
        parts.push(serde_json::json!({ "text": msg.content }));

        // Add images if present
        if let Some(images) = msg.images {
            for base64_image in images {
                parts.push(serde_json::json!({
                    "inline_data": {
                        "mime_type": "image/png",
                        "data": base64_image
                    }
                }));
            }
        }

        contents.push(serde_json::json!({
            "role": role,
            "parts": parts
        }));
    }

    let inner_request_body = if enable_grounding {
        serde_json::json!({
            "contents": contents,
            "tools": [{
                "google_search": {}
            }]
        })
    } else {
        serde_json::json!({
            "contents": contents
        })
    };

    // Branch based on auth method
    if provider.auth_method == AuthMethod::OAuth {
        // OAuth: Use Code Assist API
        chat_completion_gemini_code_assist(api_key, model_id, inner_request_body).await
    } else {
        // API key: Use standard Generative Language API
        chat_completion_gemini_api_key(api_key, model_id, inner_request_body).await
    }
}

/// Gemini API call using API key (standard Generative Language API)
async fn chat_completion_gemini_api_key(
    api_key: &str,
    model_id: &str,
    request_body: serde_json::Value,
) -> Result<ChatResponse, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model_id, api_key
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Gemini API error {}: {}", status, body));
    }

    let res_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    parse_gemini_response(res_json)
}

/// Gemini API call using OAuth (Code Assist API)
async fn chat_completion_gemini_code_assist(
    access_token: &str,
    model_id: &str,
    inner_request_body: serde_json::Value,
) -> Result<ChatResponse, String> {
    use crate::oauth::google::{
        build_code_assist_url, ensure_project_id, unwrap_code_assist_response,
        wrap_request_for_code_assist,
    };

    // Get or provision project ID
    let project_id = ensure_project_id(access_token)
        .await
        .map_err(|e| format!("Failed to get project ID: {}", e))?;

    // Build the Code Assist URL
    let url = build_code_assist_url("generateContent", false);

    // Wrap the request for Code Assist
    let request_body = wrap_request_for_code_assist(&project_id, model_id, inner_request_body);

    log::info!(
        "Code Assist API request:\n  URL: {}\n  Project: {}\n  Model: {}\n  Body: {}",
        url,
        project_id,
        model_id,
        serde_json::to_string_pretty(&request_body).unwrap_or_default()
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
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
        .map_err(|e| format!("Code Assist request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        log::error!("Code Assist API error: status={}, body={}", status, body);
        return Err(format!("Code Assist API error {}: {}", status, body));
    }

    let res_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Code Assist response: {}", e))?;

    // Unwrap the Code Assist response wrapper
    let unwrapped = unwrap_code_assist_response(res_json)
        .map_err(|e| format!("Failed to unwrap Code Assist response: {}", e))?;

    parse_gemini_response(unwrapped)
}

/// Parse a standard Gemini API response (works for both API key and Code Assist)
fn parse_gemini_response(res_json: serde_json::Value) -> Result<ChatResponse, String> {
    let candidate = &res_json["candidates"][0];
    let content = candidate["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| "No text in Gemini response".to_string())?
        .to_string();

    // Parse grounding metadata
    let mut grounding_metadata = None;
    if let Some(grounding_info) = candidate.get("groundingMetadata") {
        let mut chunks = Vec::new();
        if let Some(grounding_chunks) = grounding_info["groundingChunks"].as_array() {
            for chunk in grounding_chunks {
                if let Some(web) = chunk.get("web") {
                    let uri = web["uri"].as_str().map(|s| s.to_string());
                    let title = web["title"].as_str().map(|s| s.to_string());
                    log::info!("Grounding chunk - uri: {:?}, title: {:?}", uri, title);
                    chunks.push(GroundingChunk { uri, title });
                }
            }
        }

        let search_entry_point = grounding_info["searchEntryPoint"]["renderedContent"]
            .as_str()
            .map(|s| s.to_string());

        if !chunks.is_empty() || search_entry_point.is_some() {
            grounding_metadata = Some(GroundingMetadata {
                search_entry_point,
                chunks,
            });
        }
    }

    Ok(ChatResponse {
        content,
        grounding_metadata,
    })
}

/// Native Anthropic Messages API call with web search support
async fn chat_completion_anthropic_native(
    app: &AppHandle,
    provider: &crate::settings::LLMProvider,
    api_key: &str,
    model_id: &str,
    messages: Vec<ChatMessage>,
    enable_grounding: bool,
) -> Result<ChatResponse, String> {
    let url = format!("{}/messages", provider.base_url);

    // Build messages array for Anthropic format
    let mut anthropic_messages = Vec::new();

    for msg in &messages {
        let role = if msg.role == "assistant" {
            "assistant"
        } else {
            "user"
        };

        // Build content array
        let mut content_parts = Vec::new();

        // Add images if present (Anthropic uses source.type = "base64")
        if let Some(images) = &msg.images {
            for base64_image in images {
                content_parts.push(serde_json::json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": base64_image
                    }
                }));
            }
        }

        // Add text content
        content_parts.push(serde_json::json!({
            "type": "text",
            "text": msg.content
        }));

        anthropic_messages.push(serde_json::json!({
            "role": role,
            "content": content_parts
        }));
    }

    // Build request body
    let mut request_body = serde_json::json!({
        "model": model_id,
        "max_tokens": 8192,
        "messages": anthropic_messages
    });

    // Add system prompt if configured
    if let Some(system_prompt) = get_system_prompt_content(app) {
        request_body["system"] = serde_json::json!(system_prompt);
    }

    // Add web search tool if grounding is enabled
    if enable_grounding {
        request_body["tools"] = serde_json::json!([{
            "type": "web_search_20250305",
            "name": "web_search"
        }]);
        // Force the model to always use web search when grounding is enabled
        request_body["tool_choice"] = serde_json::json!({
            "type": "tool",
            "name": "web_search"
        });
    }

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Anthropic API error {}: {}", status, body));
    }

    let res_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // Extract text content from response
    // Anthropic returns content as an array of blocks
    let content_blocks = res_json["content"]
        .as_array()
        .ok_or_else(|| "No content in Anthropic response".to_string())?;

    let mut text_content = String::new();
    let mut grounding_chunks = Vec::new();

    for block in content_blocks {
        match block["type"].as_str() {
            Some("text") => {
                if let Some(text) = block["text"].as_str() {
                    text_content.push_str(text);
                }
                // Check for citations in this text block
                if let Some(citations) = block["citations"].as_array() {
                    for citation in citations {
                        if citation["type"].as_str() == Some("web_search_result_location") {
                            grounding_chunks.push(GroundingChunk {
                                uri: citation["url"].as_str().map(|s| s.to_string()),
                                title: citation["title"].as_str().map(|s| s.to_string()),
                            });
                        }
                    }
                }
            }
            Some("web_search_tool_result") => {
                // Extract sources from web search results
                if let Some(results) = block["content"].as_array() {
                    for result in results {
                        if result["type"].as_str() == Some("web_search_result") {
                            grounding_chunks.push(GroundingChunk {
                                uri: result["url"].as_str().map(|s| s.to_string()),
                                title: result["title"].as_str().map(|s| s.to_string()),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Build grounding metadata if we have any citations
    let grounding_metadata = if !grounding_chunks.is_empty() {
        // Deduplicate chunks by URL
        let mut seen_urls = std::collections::HashSet::new();
        let unique_chunks: Vec<GroundingChunk> = grounding_chunks
            .into_iter()
            .filter(|chunk| {
                if let Some(ref uri) = chunk.uri {
                    seen_urls.insert(uri.clone())
                } else {
                    true
                }
            })
            .collect();

        Some(GroundingMetadata {
            search_entry_point: None,
            chunks: unique_chunks,
        })
    } else {
        None
    };

    Ok(ChatResponse {
        content: text_content,
        grounding_metadata,
    })
}

/// OpenAI Codex API call for ChatGPT Plus/Pro OAuth
/// Uses the Codex backend at chatgpt.com/backend-api instead of api.openai.com
async fn chat_completion_openai_codex(
    app: &AppHandle,
    access_token: &str,
    model_id: &str,
    messages: Vec<ChatMessage>,
) -> Result<ChatResponse, String> {
    use crate::oauth::openai::API_ENDPOINT;
    use crate::oauth::tokens::load_tokens;
    use crate::oauth::OAuthProvider;

    // Load tokens to get chatgpt_account_id
    let tokens = load_tokens(OAuthProvider::OpenAI)
        .map_err(|e| format!("Failed to load OAuth tokens: {}", e))?;

    let chatgpt_account_id = tokens
        .chatgpt_account_id
        .ok_or_else(|| "No ChatGPT account ID found in tokens".to_string())?;

    // Get reasoning effort from settings
    let settings = get_settings(app);
    let reasoning_effort = get_valid_reasoning_effort(&settings.openai_reasoning_effort, model_id);

    // Normalize model name for Codex API
    let normalized_model = normalize_codex_model(model_id);

    // Build input array in Responses API format
    let mut input = Vec::new();

    // Add system prompt as developer message if configured
    if let Some(system_prompt) = get_system_prompt_content(app) {
        input.push(serde_json::json!({
            "type": "message",
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": system_prompt
            }]
        }));
    }

    // Convert messages to Responses API format
    for msg in messages {
        let role = match msg.role.as_str() {
            "assistant" => "assistant",
            "system" => "developer",
            _ => "user",
        };

        let mut content_parts = Vec::new();

        // Add text content
        // Note: assistant messages require 'output_text', others use 'input_text'
        let content_type = if role == "assistant" {
            "output_text"
        } else {
            "input_text"
        };

        content_parts.push(serde_json::json!({
            "type": content_type,
            "text": msg.content
        }));

        // Add images if present
        if let Some(images) = msg.images {
            for base64_image in images {
                content_parts.push(serde_json::json!({
                    "type": "input_image",
                    "image_url": format!("data:image/png;base64,{}", base64_image)
                }));
            }
        }

        input.push(serde_json::json!({
            "type": "message",
            "role": role,
            "content": content_parts
        }));
    }

    // Get Codex instructions for this model
    let instructions = get_codex_instructions(&normalized_model);

    // Build request body for Codex Responses API
    // Note: stream must be true for the ChatGPT backend, but we handle the SSE response
    let request_body = serde_json::json!({
        "model": normalized_model,
        "instructions": instructions,
        "input": input,
        "store": false,
        "stream": true,
        "reasoning": {
            "effort": reasoning_effort,
            "summary": "auto"
        },
        "text": {
            "verbosity": "medium"
        },
        "include": ["reasoning.encrypted_content"]
    });

    let url = format!("{}/codex/responses", API_ENDPOINT);

    log::info!(
        "Codex API request:\n  URL: {}\n  Model: {}\n  Account ID: {}\n  Body: {}",
        url,
        normalized_model,
        chatgpt_account_id,
        serde_json::to_string_pretty(&request_body).unwrap_or_default()
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("chatgpt-account-id", &chatgpt_account_id)
        .header("OpenAI-Beta", "responses=experimental")
        .header("originator", "codex_cli_rs")
        .header("accept", "text/event-stream")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Codex API request failed: {}", e))?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Codex response: {}", e))?;

    log::info!(
        "Codex API response: status={}, body_length={}",
        status,
        response_text.len()
    );
    log::debug!("Codex API full response:\n{}", response_text);

    if !status.is_success() {
        // Check for usage limit errors and provide a clearer message
        if response_text.contains("usage_limit_reached")
            || response_text.contains("usage_not_included")
            || response_text.contains("rate_limit_exceeded")
        {
            return Err(
                "ChatGPT usage limit reached. Please try again later or check your subscription."
                    .to_string(),
            );
        }
        return Err(format!("Codex API error {}: {}", status, response_text));
    }

    // Parse SSE stream to extract final response
    // The Codex API returns SSE events, we need to find the response.done event
    let res_json = parse_codex_sse_response(&response_text)?;

    // Parse Codex Responses API response
    // The response format has an "output" array with message items
    let output = res_json["output"]
        .as_array()
        .ok_or_else(|| "No output in Codex response".to_string())?;

    let mut text_content = String::new();

    for item in output {
        if item["type"].as_str() == Some("message") {
            if let Some(content) = item["content"].as_array() {
                for part in content {
                    if part["type"].as_str() == Some("output_text") {
                        if let Some(text) = part["text"].as_str() {
                            if !text_content.is_empty() {
                                text_content.push('\n');
                            }
                            text_content.push_str(text);
                        }
                    }
                }
            }
        }
    }

    if text_content.is_empty() {
        return Err("No text content in Codex response".to_string());
    }

    Ok(ChatResponse {
        content: text_content,
        grounding_metadata: None,
    })
}

/// Normalize model name for Codex API
/// Maps user-friendly model names to Codex-supported variants
fn normalize_codex_model(model: &str) -> String {
    let normalized = model.to_lowercase();

    // GPT-5.2 variants
    if normalized.contains("gpt-5.2-codex") || normalized.contains("gpt 5.2 codex") {
        return "gpt-5.2-codex".to_string();
    }
    if normalized.contains("gpt-5.2") || normalized.contains("gpt 5.2") {
        return "gpt-5.2".to_string();
    }

    // GPT-5.1 Codex variants
    if normalized.contains("gpt-5.1-codex-max") || normalized.contains("codex-max") {
        return "gpt-5.1-codex-max".to_string();
    }
    if normalized.contains("gpt-5.1-codex-mini") || normalized.contains("codex-mini") {
        return "gpt-5.1-codex-mini".to_string();
    }
    if normalized.contains("gpt-5.1-codex") || normalized.contains("gpt 5.1 codex") {
        return "gpt-5.1-codex".to_string();
    }

    // GPT-5.1 general
    if normalized.contains("gpt-5.1") || normalized.contains("gpt 5.1") {
        return "gpt-5.1".to_string();
    }

    // Legacy GPT-5 (map to 5.1)
    if normalized.contains("gpt-5") || normalized.contains("gpt 5") {
        return "gpt-5.1".to_string();
    }

    // O-series models
    if normalized.contains("o3") {
        return "o3".to_string();
    }
    if normalized.contains("o4-mini") || normalized.contains("o4 mini") {
        return "o4-mini".to_string();
    }

    // Default to gpt-5.1
    "gpt-5.1".to_string()
}

/// Get Codex instructions (system prompt) for a given model
/// These instructions are required by the Codex API
fn get_codex_instructions(model: &str) -> String {
    // Minimal instructions that work for our voice assistant use case
    // The Codex API requires this field - without it, we get "Instructions are required"
    //
    // Note: The full Codex CLI fetches model-specific prompts from GitHub:
    // https://github.com/openai/codex/tree/main/codex-rs/core
    // For our simpler use case (voice transcription refinement), minimal instructions suffice.
    let model_family = get_model_family(model);

    match model_family {
        "gpt-5.2-codex" | "codex" | "codex-max" => {
            r#"You are a helpful AI assistant specialized in coding tasks. 
You help users with programming questions, code review, debugging, and software development.
Be concise, accurate, and provide practical solutions."#
                .to_string()
        }
        _ => {
            // General purpose models (gpt-5.2, gpt-5.1)
            r#"You are a helpful AI assistant. 
Provide clear, concise, and accurate responses.
When helping with code or technical topics, be precise and practical."#
                .to_string()
        }
    }
}

/// Get the model family for selecting appropriate instructions
fn get_model_family(model: &str) -> &'static str {
    let normalized = model.to_lowercase();

    if normalized.contains("gpt-5.2-codex") {
        return "gpt-5.2-codex";
    }
    if normalized.contains("codex-max") {
        return "codex-max";
    }
    if normalized.contains("codex") {
        return "codex";
    }
    if normalized.contains("gpt-5.2") {
        return "gpt-5.2";
    }
    "gpt-5.1"
}

/// Parse SSE (Server-Sent Events) response from Codex API
/// The API returns events like:
///   data: {"type":"response.output_item.added",...}
///   data: {"type":"response.done","response":{...}}
/// We need to find the response.done event and extract the response object
fn parse_codex_sse_response(sse_text: &str) -> Result<serde_json::Value, String> {
    for line in sse_text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            // Try to parse the JSON data
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                let event_type = event["type"].as_str().unwrap_or("");

                // Look for response.done or response.completed event
                if event_type == "response.done" || event_type == "response.completed" {
                    if let Some(response) = event.get("response") {
                        log::info!("Found response.done event with response object");
                        return Ok(response.clone());
                    }
                }
            }
        }
    }

    // If no response.done event found, try to parse the whole thing as JSON
    // (fallback for non-streaming responses)
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(sse_text) {
        return Ok(json);
    }

    Err("Could not find response.done event in SSE stream".to_string())
}

/// Get a valid reasoning effort for a model, adjusting if necessary
/// Different models support different reasoning effort levels:
/// - gpt-5.2: none/low/medium/high/xhigh
/// - gpt-5.2-codex: low/medium/high/xhigh (no "none")
/// - gpt-5.1-codex-max: low/medium/high/xhigh (no "none")
/// - gpt-5.1-codex: low/medium/high (no "none", no "xhigh")
/// - gpt-5.1-codex-mini: medium/high (only medium and high)
/// - gpt-5.1: none/low/medium/high (no "xhigh")
fn get_valid_reasoning_effort(requested_effort: &str, model_id: &str) -> String {
    let normalized_model = model_id.to_lowercase();
    let effort = requested_effort.to_lowercase();

    // Determine model capabilities
    let is_codex = normalized_model.contains("codex");
    let is_mini = normalized_model.contains("mini");
    let is_max = normalized_model.contains("max");

    let supports_none =
        !is_codex && (normalized_model.contains("gpt-5.2") || normalized_model.contains("gpt-5.1"));
    let supports_xhigh = normalized_model.contains("gpt-5.2") || is_max;
    let supports_low = !is_mini;

    // Validate and adjust effort
    match effort.as_str() {
        "none" => {
            if supports_none {
                "none".to_string()
            } else if supports_low {
                "low".to_string()
            } else {
                "medium".to_string()
            }
        }
        "low" => {
            if supports_low {
                "low".to_string()
            } else {
                "medium".to_string()
            }
        }
        "medium" => "medium".to_string(),
        "high" => "high".to_string(),
        "xhigh" => {
            if supports_xhigh {
                "xhigh".to_string()
            } else {
                "high".to_string()
            }
        }
        _ => "medium".to_string(), // Default fallback
    }
}
