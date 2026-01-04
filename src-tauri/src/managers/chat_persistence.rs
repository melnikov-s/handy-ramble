use anyhow::Result;
use chrono::Utc;
use log::{debug, info};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::chat::ChatMessage;

/// Database migrations for chat history.
static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS chats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL DEFAULT 'New Chat',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            messages_json TEXT NOT NULL
        );",
    ),
    M::up("CREATE INDEX IF NOT EXISTS idx_chats_updated_at ON chats(updated_at DESC);"),
];

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct SavedChat {
    pub id: i64,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: Vec<ChatMessage>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct ChatSummary {
    pub id: i64,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: usize,
}

pub struct ChatPersistenceManager {
    app_handle: AppHandle,
    db_path: PathBuf,
}

impl ChatPersistenceManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_data_dir = app_handle.path().app_data_dir()?;
        let db_path = app_data_dir.join("chats.db");

        let manager = Self {
            app_handle: app_handle.clone(),
            db_path,
        };

        manager.init_database()?;

        Ok(manager)
    }

    fn init_database(&self) -> Result<()> {
        info!("Initializing chat database at {:?}", self.db_path);

        let mut conn = Connection::open(&self.db_path)?;
        let migrations = Migrations::new(MIGRATIONS.to_vec());

        #[cfg(debug_assertions)]
        migrations.validate().expect("Invalid chat migrations");

        migrations.to_latest(&mut conn)?;

        Ok(())
    }

    fn get_connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    pub fn save_chat(&self, title: Option<String>, messages: Vec<ChatMessage>) -> Result<i64> {
        let conn = self.get_connection()?;
        let now = Utc::now().timestamp();
        let messages_json = serde_json::to_string(&messages)?;
        let title = title.unwrap_or_else(|| "New Chat".to_string());

        conn.execute(
            "INSERT INTO chats (title, created_at, updated_at, messages_json) VALUES (?1, ?2, ?3, ?4)",
            params![title, now, now, messages_json],
        )?;

        let id = conn.last_insert_rowid();
        debug!("Saved new chat with id: {}", id);

        // Emit event for UI updates
        let _ = self.app_handle.emit("chats-updated", ());

        Ok(id)
    }

    pub fn update_chat(&self, id: i64, messages: Vec<ChatMessage>) -> Result<()> {
        let conn = self.get_connection()?;
        let now = Utc::now().timestamp();
        let messages_json = serde_json::to_string(&messages)?;

        conn.execute(
            "UPDATE chats SET messages_json = ?1, updated_at = ?2 WHERE id = ?3",
            params![messages_json, now, id],
        )?;

        debug!("Updated chat with id: {}", id);
        Ok(())
    }

    pub fn update_title(&self, id: i64, title: String) -> Result<()> {
        let conn = self.get_connection()?;
        let now = Utc::now().timestamp();

        conn.execute(
            "UPDATE chats SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now, id],
        )?;

        debug!("Updated title for chat {}: {}", id, title);
        let _ = self.app_handle.emit("chats-updated", ());
        Ok(())
    }

    pub fn get_chat(&self, id: i64) -> Result<Option<SavedChat>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, messages_json FROM chats WHERE id = ?1",
        )?;

        let chat = stmt
            .query_row([id], |row| {
                let messages_json: String = row.get("messages_json")?;
                let messages: Vec<ChatMessage> =
                    serde_json::from_str(&messages_json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;

                Ok(SavedChat {
                    id: row.get("id")?,
                    title: row.get("title")?,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                    messages,
                })
            })
            .optional()?;

        Ok(chat)
    }

    pub fn list_chats(&self) -> Result<Vec<ChatSummary>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, messages_json FROM chats ORDER BY updated_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let messages_json: String = row.get("messages_json")?;
            let message_count = serde_json::from_str::<Vec<serde_json::Value>>(&messages_json)
                .map(|v| v.len())
                .unwrap_or(0);

            Ok(ChatSummary {
                id: row.get("id")?,
                title: row.get("title")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
                message_count,
            })
        })?;

        let mut chats = Vec::new();
        for row in rows {
            chats.push(row?);
        }

        Ok(chats)
    }

    pub fn delete_chat(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM chats WHERE id = ?1", params![id])?;

        debug!("Deleted chat with id: {}", id);
        let _ = self.app_handle.emit("chats-updated", ());
        Ok(())
    }
}
