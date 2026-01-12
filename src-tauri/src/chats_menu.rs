//! Chats menu for macOS app menu bar
//!
//! Provides a "Chats" submenu in the app menu bar with:
//! - "New Chat" option to create a new chat window
//! - List of up to 20 recent chats ordered by last update

use crate::managers::chat_persistence::ChatPersistenceManager;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Wry};

/// Maximum number of recent chats to show in the menu
const MAX_RECENT_CHATS: usize = 20;

/// Builds the Chats submenu with recent chats
pub fn build_chats_submenu(app: &AppHandle) -> Result<Submenu<Wry>, tauri::Error> {
    let chats_submenu = Submenu::with_id(app, "chats_menu", "Chats", true)?;

    // Add "New Chat" item
    let new_chat_item = MenuItem::with_id(app, "chats_new", "New Chat", true, None::<&str>)?;
    chats_submenu.append(&new_chat_item)?;

    // Add separator
    let separator = PredefinedMenuItem::separator(app)?;
    chats_submenu.append(&separator)?;

    // Get recent chats from persistence manager
    if let Some(manager) = app.try_state::<Arc<ChatPersistenceManager>>() {
        match manager.list_chats() {
            Ok(chats) => {
                if chats.is_empty() {
                    // Show disabled "No Saved Chats" item
                    let no_chats_item =
                        MenuItem::with_id(app, "no_chats", "No Saved Chats", false, None::<&str>)?;
                    chats_submenu.append(&no_chats_item)?;
                } else {
                    // Add up to MAX_RECENT_CHATS
                    for chat in chats.into_iter().take(MAX_RECENT_CHATS) {
                        // Truncate title if too long
                        let title = if chat.title.len() > 40 {
                            format!("{}...", &chat.title[..37])
                        } else {
                            chat.title.clone()
                        };

                        let item_id = format!("chat_open_{}", chat.id);
                        let chat_item =
                            MenuItem::with_id(app, &item_id, &title, true, None::<&str>)?;
                        chats_submenu.append(&chat_item)?;
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to list chats for menu: {}", e);
                let error_item = MenuItem::with_id(
                    app,
                    "chats_error",
                    "Error loading chats",
                    false,
                    None::<&str>,
                )?;
                chats_submenu.append(&error_item)?;
            }
        }
    } else {
        log::warn!("ChatPersistenceManager not available for chats menu");
    }

    Ok(chats_submenu)
}

/// Creates the complete app menu with Chats submenu
pub fn build_app_menu(app: &AppHandle) -> Result<Menu<Wry>, tauri::Error> {
    let menu = Menu::new(app)?;

    // Add the standard app submenu (About, Preferences, etc.)
    let app_submenu = Submenu::with_id(app, "app_menu", "Ramble", true)?;
    app_submenu.append(&PredefinedMenuItem::about(app, Some("About Ramble"), None)?)?;
    app_submenu.append(&PredefinedMenuItem::separator(app)?)?;
    app_submenu.append(&PredefinedMenuItem::services(app, None)?)?;
    app_submenu.append(&PredefinedMenuItem::separator(app)?)?;
    app_submenu.append(&PredefinedMenuItem::hide(app, None)?)?;
    app_submenu.append(&PredefinedMenuItem::hide_others(app, None)?)?;
    app_submenu.append(&PredefinedMenuItem::show_all(app, None)?)?;
    app_submenu.append(&PredefinedMenuItem::separator(app)?)?;
    app_submenu.append(&PredefinedMenuItem::quit(app, None)?)?;
    menu.append(&app_submenu)?;

    // Add the Chats submenu
    let chats_submenu = build_chats_submenu(app)?;
    menu.append(&chats_submenu)?;

    // Add Edit menu for standard editing commands
    let edit_submenu = Submenu::with_id(app, "edit_menu", "Edit", true)?;
    edit_submenu.append(&PredefinedMenuItem::undo(app, None)?)?;
    edit_submenu.append(&PredefinedMenuItem::redo(app, None)?)?;
    edit_submenu.append(&PredefinedMenuItem::separator(app)?)?;
    edit_submenu.append(&PredefinedMenuItem::cut(app, None)?)?;
    edit_submenu.append(&PredefinedMenuItem::copy(app, None)?)?;
    edit_submenu.append(&PredefinedMenuItem::paste(app, None)?)?;
    edit_submenu.append(&PredefinedMenuItem::select_all(app, None)?)?;
    menu.append(&edit_submenu)?;

    // Add Window menu
    let window_submenu = Submenu::with_id(app, "window_menu", "Window", true)?;
    window_submenu.append(&PredefinedMenuItem::minimize(app, None)?)?;
    window_submenu.append(&PredefinedMenuItem::maximize(app, None)?)?;
    window_submenu.append(&PredefinedMenuItem::separator(app)?)?;
    window_submenu.append(&PredefinedMenuItem::close_window(app, None)?)?;
    menu.append(&window_submenu)?;

    Ok(menu)
}

/// Updates the Chats menu with fresh data
/// Call this when chats are created, updated, or deleted
pub fn refresh_chats_menu(app: &AppHandle) {
    // Rebuild the entire app menu to refresh the chats list
    match build_app_menu(app) {
        Ok(menu) => {
            if let Err(e) = app.set_menu(menu) {
                log::error!("Failed to update app menu: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to rebuild app menu: {}", e);
        }
    }
}
