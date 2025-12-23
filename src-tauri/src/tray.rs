use crate::settings::{self, PromptMode};
use crate::tray_i18n::get_tray_translations;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Manager, Theme};

#[derive(Clone, Debug, PartialEq)]
pub enum TrayIconState {
    Idle,
    Recording,
    Transcribing,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppTheme {
    Dark,
    Light,
    Colored, // Pink/colored theme for Linux
}

/// Gets the current app theme, with Linux defaulting to Colored theme
pub fn get_current_theme(app: &AppHandle) -> AppTheme {
    if cfg!(target_os = "linux") {
        // On Linux, always use the colored theme
        AppTheme::Colored
    } else {
        // On other platforms, map system theme to our app theme
        if let Some(main_window) = app.get_webview_window("main") {
            match main_window.theme().unwrap_or(Theme::Dark) {
                Theme::Light => AppTheme::Light,
                Theme::Dark => AppTheme::Dark,
                _ => AppTheme::Dark, // Default fallback
            }
        } else {
            AppTheme::Dark
        }
    }
}

/// Gets the appropriate icon path for the given theme and state
pub fn get_icon_path(theme: AppTheme, state: TrayIconState) -> &'static str {
    match (theme, state) {
        // Dark theme uses light icons
        (AppTheme::Dark, TrayIconState::Idle) => "resources/tray_idle.png",
        (AppTheme::Dark, TrayIconState::Recording) => "resources/tray_recording.png",
        (AppTheme::Dark, TrayIconState::Transcribing) => "resources/tray_transcribing.png",
        // Light theme uses dark icons
        (AppTheme::Light, TrayIconState::Idle) => "resources/tray_idle_dark.png",
        (AppTheme::Light, TrayIconState::Recording) => "resources/tray_recording_dark.png",
        (AppTheme::Light, TrayIconState::Transcribing) => "resources/tray_transcribing_dark.png",
        // Colored theme uses pink icons (for Linux)
        (AppTheme::Colored, TrayIconState::Idle) => "resources/ramble.png",
        (AppTheme::Colored, TrayIconState::Recording) => "resources/recording.png",
        (AppTheme::Colored, TrayIconState::Transcribing) => "resources/transcribing.png",
    }
}

pub fn change_tray_icon(app: &AppHandle, icon: TrayIconState) {
    let tray = app.state::<TrayIcon>();
    let theme = get_current_theme(app);

    let icon_path = get_icon_path(theme, icon.clone());

    let _ = tray.set_icon(Some(
        Image::from_path(
            app.path()
                .resolve(icon_path, tauri::path::BaseDirectory::Resource)
                .expect("failed to resolve"),
        )
        .expect("failed to set icon"),
    ));

    // Update menu based on state
    update_tray_menu(app, &icon, None);
}

/// Set the prompt mode and update the tray menu
pub fn set_prompt_mode(app: &AppHandle, mode: PromptMode) {
    use tauri::Emitter;

    let mut settings = settings::get_settings(app);
    settings.prompt_mode = mode;
    settings::write_settings(app, settings);

    // Emit event for overlay/frontend to update
    let _ = app.emit("prompt-mode-changed", mode);

    // Refresh the tray menu to update checkmarks
    update_tray_menu(app, &TrayIconState::Idle, None);
}

pub fn update_tray_menu(app: &AppHandle, state: &TrayIconState, locale: Option<&str>) {
    let settings = settings::get_settings(app);

    let locale = locale.unwrap_or(&settings.app_language);
    let strings = get_tray_translations(Some(locale.to_string()));

    // Platform-specific accelerators
    #[cfg(target_os = "macos")]
    let (settings_accelerator, quit_accelerator) = (Some("Cmd+,"), Some("Cmd+Q"));
    #[cfg(not(target_os = "macos"))]
    let (settings_accelerator, quit_accelerator) = (Some("Ctrl+,"), Some("Ctrl+Q"));

    // Create common menu items
    let version_label = if cfg!(debug_assertions) {
        format!("Ramble v{} (Dev)", env!("CARGO_PKG_VERSION"))
    } else {
        format!("Ramble v{}", env!("CARGO_PKG_VERSION"))
    };
    let version_i = MenuItem::with_id(app, "version", &version_label, false, None::<&str>)
        .expect("failed to create version item");
    let settings_i = MenuItem::with_id(
        app,
        "settings",
        &strings.settings,
        true,
        settings_accelerator,
    )
    .expect("failed to create settings item");
    let check_updates_i = MenuItem::with_id(
        app,
        "check_updates",
        &strings.check_updates,
        settings.update_checks_enabled,
        None::<&str>,
    )
    .expect("failed to create check updates item");
    let quit_i = MenuItem::with_id(app, "quit", &strings.quit, true, quit_accelerator)
        .expect("failed to create quit item");
    let separator = || PredefinedMenuItem::separator(app).expect("failed to create separator");

    // Create prompt mode submenu items with checkmarks
    let current_mode = settings.prompt_mode;

    let mode_dynamic = CheckMenuItem::with_id(
        app,
        "mode_dynamic",
        format!("{} {}", PromptMode::Dynamic.icon(), &strings.dynamic),
        true,
        current_mode == PromptMode::Dynamic,
        None::<&str>,
    )
    .expect("failed to create dynamic mode item");

    let mode_development = CheckMenuItem::with_id(
        app,
        "mode_development",
        format!(
            "{} {}",
            PromptMode::Development.icon(),
            &strings.development
        ),
        true,
        current_mode == PromptMode::Development,
        None::<&str>,
    )
    .expect("failed to create development mode item");

    let mode_conversation = CheckMenuItem::with_id(
        app,
        "mode_conversation",
        format!(
            "{} {}",
            PromptMode::Conversation.icon(),
            &strings.conversation
        ),
        true,
        current_mode == PromptMode::Conversation,
        None::<&str>,
    )
    .expect("failed to create conversation mode item");

    let mode_writing = CheckMenuItem::with_id(
        app,
        "mode_writing",
        format!("{} {}", PromptMode::Writing.icon(), &strings.writing),
        true,
        current_mode == PromptMode::Writing,
        None::<&str>,
    )
    .expect("failed to create writing mode item");

    let mode_email = CheckMenuItem::with_id(
        app,
        "mode_email",
        format!("{} {}", PromptMode::Email.icon(), &strings.email),
        true,
        current_mode == PromptMode::Email,
        None::<&str>,
    )
    .expect("failed to create email mode item");

    let menu = match state {
        TrayIconState::Recording | TrayIconState::Transcribing => {
            let cancel_i = MenuItem::with_id(app, "cancel", &strings.cancel, true, None::<&str>)
                .expect("failed to create cancel item");
            Menu::with_items(
                app,
                &[
                    &version_i,
                    &separator(),
                    &cancel_i,
                    &separator(),
                    &mode_dynamic,
                    &mode_development,
                    &mode_conversation,
                    &mode_writing,
                    &mode_email,
                    &separator(),
                    &settings_i,
                    &check_updates_i,
                    &separator(),
                    &quit_i,
                ],
            )
            .expect("failed to create menu")
        }
        TrayIconState::Idle => Menu::with_items(
            app,
            &[
                &version_i,
                &separator(),
                &mode_dynamic,
                &mode_development,
                &mode_conversation,
                &mode_writing,
                &mode_email,
                &separator(),
                &settings_i,
                &check_updates_i,
                &separator(),
                &quit_i,
            ],
        )
        .expect("failed to create menu"),
    };

    let tray = app.state::<TrayIcon>();
    let _ = tray.set_menu(Some(menu));
    let _ = tray.set_icon_as_template(true);
}
