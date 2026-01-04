use crate::commands::chat::{chat_completion, ChatMessage, ChatResponse};
use crate::managers::chat_persistence::{ChatPersistenceManager, ChatSummary, SavedChat};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[tauri::command]
#[specta::specta]
pub async fn save_chat(
    app: AppHandle,
    title: Option<String>,
    messages: Vec<ChatMessage>,
) -> Result<i64, String> {
    let manager = app.state::<Arc<ChatPersistenceManager>>();
    manager
        .save_chat(title, messages)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_chat(
    app: AppHandle,
    id: i64,
    messages: Vec<ChatMessage>,
) -> Result<(), String> {
    let manager = app.state::<Arc<ChatPersistenceManager>>();
    manager.update_chat(id, messages).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_chat(app: AppHandle, id: i64) -> Result<Option<SavedChat>, String> {
    let manager = app.state::<Arc<ChatPersistenceManager>>();
    manager.get_chat(id).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_saved_chats(app: AppHandle) -> Result<Vec<ChatSummary>, String> {
    let manager = app.state::<Arc<ChatPersistenceManager>>();
    manager.list_chats().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_saved_chat(app: AppHandle, id: i64) -> Result<(), String> {
    let manager = app.state::<Arc<ChatPersistenceManager>>();
    manager.delete_chat(id).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn generate_chat_title(
    app: AppHandle,
    user_message: String,
    assistant_response: String,
) -> Result<String, String> {
    let prompt = format!(
        "Generate a concise 3-5 word title for this conversation based on the following exchange.\n\
        Respond with ONLY the title, no quotes, no labels, no punctuation.\n\n\
        User: {}\n\
        Assistant: {}",
        if user_message.len() > 500 { &user_message[..500] } else { &user_message },
        if assistant_response.len() > 500 { &assistant_response[..500] } else { &assistant_response }
    );

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You are a helpful assistant that generates concise chat titles.".to_string(),
            images: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: prompt,
            images: None,
        },
    ];

    // Use existing chat_completion logic but with our custom prompt
    // We pass None for model_id to use the default chat model
    let response: ChatResponse = chat_completion(app.clone(), messages, None, false).await?;

    let title = response.content.trim().trim_matches('"').to_string();
    Ok(title)
}

#[tauri::command]
#[specta::specta]
pub async fn update_chat_title(app: AppHandle, id: i64, title: String) -> Result<(), String> {
    let manager = app.state::<Arc<ChatPersistenceManager>>();
    manager.update_title(id, title).map_err(|e| e.to_string())
}
