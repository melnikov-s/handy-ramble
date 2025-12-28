use crate::llm_client::create_client;
use crate::settings::get_settings;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Send a chat completion request to the configured LLM provider
///
/// # Arguments
/// * `model_id` - Optional model ID to use. Falls back to `default_chat_model_id` if not provided.
#[tauri::command]
#[specta::specta]
pub async fn chat_completion(
    app: AppHandle,
    messages: Vec<ChatMessage>,
    model_id: Option<String>,
) -> Result<String, String> {
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

    // Create the client
    let client = create_client(provider, provider.api_key.clone())?;

    // Convert messages to OpenAI format
    let mut openai_messages: Vec<ChatCompletionRequestMessage> = Vec::new();

    for msg in messages {
        let openai_msg = match msg.role.as_str() {
            "system" => ChatCompletionRequestSystemMessageArgs::default()
                .content(msg.content)
                .build()
                .map_err(|e| e.to_string())?
                .into(),
            "user" => ChatCompletionRequestUserMessageArgs::default()
                .content(msg.content)
                .build()
                .map_err(|e| e.to_string())?
                .into(),
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

    Ok(content)
}
