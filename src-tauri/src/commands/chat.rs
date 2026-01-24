use crate::llm_client::create_client;
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

    // Get API key from provider
    if provider.api_key.is_empty() {
        return Err(format!(
            "No API key configured for provider: {}",
            provider.name
        ));
    }

    // Use Gemini native API for all Gemini models (supports grounding)
    if provider.id == "gemini" {
        return chat_completion_gemini_native(
            &app,
            provider,
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
            &model.model_id,
            messages,
            enable_grounding,
        )
        .await;
    }

    // Create the client
    let client = create_client(provider, provider.api_key.clone())?;

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
    model_id: &str,
    messages: Vec<ChatMessage>,
    enable_grounding: bool,
) -> Result<ChatResponse, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model_id, provider.api_key
    );

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

    let request_body = if enable_grounding {
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
        .header("x-api-key", &provider.api_key)
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
