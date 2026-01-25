//! OAuth client configuration stored in app data directory.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

use super::tokens::TokenError;

const CONFIG_FILE: &str = "oauth_client_config.json";

static CONFIG_PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OAuthClientConfig {
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub openai_client_id: Option<String>,
}

pub fn init_oauth_config(app: &AppHandle) -> Result<(), TokenError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| TokenError::StorageError(e.to_string()))?;

    if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
        return Err(TokenError::StorageError(e.to_string()));
    }

    let path = app_data_dir.join(CONFIG_FILE);
    let _ = CONFIG_PATH.set(path);
    Ok(())
}

fn config_path() -> Result<PathBuf, TokenError> {
    CONFIG_PATH
        .get()
        .cloned()
        .ok_or_else(|| TokenError::ConfigMissing("OAuth config not initialized".to_string()))
}

fn read_config(path: &Path) -> Result<OAuthClientConfig, TokenError> {
    let json = std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            TokenError::NotFound
        } else {
            TokenError::StorageError(e.to_string())
        }
    })?;

    serde_json::from_str::<OAuthClientConfig>(&json)
        .map_err(|e| TokenError::SerializationError(e.to_string()))
}

fn read_config_or_default(path: &Path) -> Result<OAuthClientConfig, TokenError> {
    match read_config(path) {
        Ok(config) => Ok(config),
        Err(TokenError::NotFound) => Ok(OAuthClientConfig::default()),
        Err(e) => Err(e),
    }
}

fn missing_value_error(key: &str, path: &Path) -> TokenError {
    TokenError::ConfigMissing(format!(
        "Missing required config value: {} (set it in {})",
        key,
        path.to_string_lossy()
    ))
}

pub fn get_google_client_id() -> Result<String, TokenError> {
    let path = config_path()?;
    let config = read_config_or_default(&path)?;
    let value = config
        .google_client_id
        .unwrap_or_default()
        .trim()
        .to_string();
    if value.is_empty() {
        return Err(missing_value_error("google_client_id", &path));
    }
    Ok(value)
}

pub fn get_google_client_secret() -> Result<String, TokenError> {
    let path = config_path()?;
    let config = read_config_or_default(&path)?;
    let value = config
        .google_client_secret
        .unwrap_or_default()
        .trim()
        .to_string();
    if value.is_empty() {
        return Err(missing_value_error("google_client_secret", &path));
    }
    Ok(value)
}

pub fn get_openai_client_id() -> Result<Option<String>, TokenError> {
    let path = config_path()?;
    let config = read_config_or_default(&path)?;
    let value = config
        .openai_client_id
        .unwrap_or_default()
        .trim()
        .to_string();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}
