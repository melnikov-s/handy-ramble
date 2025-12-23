//! Known applications database for app-to-category mapping suggestions.
//!
//! Contains a curated list of popular applications with their bundle identifiers
//! and suggested prompt categories.

use serde::{Deserialize, Serialize};
use specta::Type;

/// A known application with suggested category
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct KnownApp {
    pub bundle_id: String,
    pub name: String,
    pub suggested_category: String,
}

/// Get the list of known applications with suggested categories
pub fn get_known_applications() -> Vec<KnownApp> {
    vec![
        // === AI-Powered Development Environments (2024-2025) ===
        KnownApp {
            bundle_id: "com.todesktop.230313mzl4w4u92".to_string(),
            name: "Cursor".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "dev.zed.Zed".to_string(),
            name: "Zed".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.codeium.windsurf".to_string(),
            name: "Windsurf".to_string(),
            suggested_category: "development".to_string(),
        },
        // === Traditional IDEs & Editors ===
        KnownApp {
            bundle_id: "com.microsoft.VSCode".to_string(),
            name: "Visual Studio Code".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.microsoft.VSCodeInsiders".to_string(),
            name: "VS Code Insiders".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.apple.dt.Xcode".to_string(),
            name: "Xcode".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.jetbrains.intellij".to_string(),
            name: "IntelliJ IDEA".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.jetbrains.intellij.ce".to_string(),
            name: "IntelliJ IDEA CE".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.jetbrains.pycharm".to_string(),
            name: "PyCharm".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.jetbrains.pycharm.ce".to_string(),
            name: "PyCharm CE".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.jetbrains.WebStorm".to_string(),
            name: "WebStorm".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.jetbrains.goland".to_string(),
            name: "GoLand".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.jetbrains.rustrover".to_string(),
            name: "RustRover".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.jetbrains.fleet".to_string(),
            name: "Fleet".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.sublimehq.Sublime-Text".to_string(),
            name: "Sublime Text".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "org.vim.MacVim".to_string(),
            name: "MacVim".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.neovide.neovide".to_string(),
            name: "Neovide".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.panic.Nova".to_string(),
            name: "Nova".to_string(),
            suggested_category: "development".to_string(),
        },
        // === Terminals ===
        KnownApp {
            bundle_id: "com.apple.Terminal".to_string(),
            name: "Terminal".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.googlecode.iterm2".to_string(),
            name: "iTerm2".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "dev.warp.Warp-Stable".to_string(),
            name: "Warp".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "co.zeit.hyper".to_string(),
            name: "Hyper".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.mitchellh.ghostty".to_string(),
            name: "Ghostty".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "net.kovidgoyal.kitty".to_string(),
            name: "Kitty".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.github.alacritty".to_string(),
            name: "Alacritty".to_string(),
            suggested_category: "development".to_string(),
        },
        // === AI Chat Applications ===
        KnownApp {
            bundle_id: "com.anthropic.claudefordesktop".to_string(),
            name: "Claude".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.openai.chat".to_string(),
            name: "ChatGPT".to_string(),
            suggested_category: "development".to_string(),
        },
        // === Chat/Messaging ===
        KnownApp {
            bundle_id: "com.tinyspeck.slackmacgap".to_string(),
            name: "Slack".to_string(),
            suggested_category: "conversation".to_string(),
        },
        KnownApp {
            bundle_id: "com.hnc.Discord".to_string(),
            name: "Discord".to_string(),
            suggested_category: "conversation".to_string(),
        },
        KnownApp {
            bundle_id: "com.apple.MobileSMS".to_string(),
            name: "Messages".to_string(),
            suggested_category: "conversation".to_string(),
        },
        KnownApp {
            bundle_id: "ru.keepcoder.Telegram".to_string(),
            name: "Telegram".to_string(),
            suggested_category: "conversation".to_string(),
        },
        KnownApp {
            bundle_id: "net.whatsapp.WhatsApp".to_string(),
            name: "WhatsApp".to_string(),
            suggested_category: "conversation".to_string(),
        },
        KnownApp {
            bundle_id: "com.facebook.Messenger".to_string(),
            name: "Messenger".to_string(),
            suggested_category: "conversation".to_string(),
        },
        KnownApp {
            bundle_id: "com.apple.FaceTime".to_string(),
            name: "FaceTime".to_string(),
            suggested_category: "conversation".to_string(),
        },
        KnownApp {
            bundle_id: "us.zoom.xos".to_string(),
            name: "Zoom".to_string(),
            suggested_category: "conversation".to_string(),
        },
        KnownApp {
            bundle_id: "com.microsoft.teams2".to_string(),
            name: "Microsoft Teams".to_string(),
            suggested_category: "conversation".to_string(),
        },
        KnownApp {
            bundle_id: "com.webex.meetingmanager".to_string(),
            name: "Webex".to_string(),
            suggested_category: "conversation".to_string(),
        },
        // === Email ===
        KnownApp {
            bundle_id: "com.apple.mail".to_string(),
            name: "Mail".to_string(),
            suggested_category: "email".to_string(),
        },
        KnownApp {
            bundle_id: "com.microsoft.Outlook".to_string(),
            name: "Outlook".to_string(),
            suggested_category: "email".to_string(),
        },
        KnownApp {
            bundle_id: "com.readdle.smartemail-Mac".to_string(),
            name: "Spark".to_string(),
            suggested_category: "email".to_string(),
        },
        KnownApp {
            bundle_id: "com.superhuman.Superhuman".to_string(),
            name: "Superhuman".to_string(),
            suggested_category: "email".to_string(),
        },
        KnownApp {
            bundle_id: "com.freron.MailMate".to_string(),
            name: "MailMate".to_string(),
            suggested_category: "email".to_string(),
        },
        // === Writing/Notes ===
        KnownApp {
            bundle_id: "com.apple.Notes".to_string(),
            name: "Notes".to_string(),
            suggested_category: "writing".to_string(),
        },
        KnownApp {
            bundle_id: "md.obsidian".to_string(),
            name: "Obsidian".to_string(),
            suggested_category: "writing".to_string(),
        },
        KnownApp {
            bundle_id: "com.apple.iWork.Pages".to_string(),
            name: "Pages".to_string(),
            suggested_category: "writing".to_string(),
        },
        KnownApp {
            bundle_id: "com.microsoft.Word".to_string(),
            name: "Microsoft Word".to_string(),
            suggested_category: "writing".to_string(),
        },
        KnownApp {
            bundle_id: "notion.id".to_string(),
            name: "Notion".to_string(),
            suggested_category: "writing".to_string(),
        },
        KnownApp {
            bundle_id: "com.logseq.logseq".to_string(),
            name: "Logseq".to_string(),
            suggested_category: "writing".to_string(),
        },
        KnownApp {
            bundle_id: "com.craft.craft".to_string(),
            name: "Craft".to_string(),
            suggested_category: "writing".to_string(),
        },
        KnownApp {
            bundle_id: "com.ulyssesapp.mac".to_string(),
            name: "Ulysses".to_string(),
            suggested_category: "writing".to_string(),
        },
        KnownApp {
            bundle_id: "com.iawriter.mac".to_string(),
            name: "iA Writer".to_string(),
            suggested_category: "writing".to_string(),
        },
        KnownApp {
            bundle_id: "pro.writer.mac".to_string(),
            name: "Writer Pro".to_string(),
            suggested_category: "writing".to_string(),
        },
        // === Browsers (default/development) ===
        KnownApp {
            bundle_id: "com.google.Chrome".to_string(),
            name: "Google Chrome".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.apple.Safari".to_string(),
            name: "Safari".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "org.mozilla.firefox".to_string(),
            name: "Firefox".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.brave.Browser".to_string(),
            name: "Brave".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "company.thebrowser.Browser".to_string(),
            name: "Arc".to_string(),
            suggested_category: "development".to_string(),
        },
        // === Productivity/Dev Tools ===
        KnownApp {
            bundle_id: "com.linear".to_string(),
            name: "Linear".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.figma.Desktop".to_string(),
            name: "Figma".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.docker.docker".to_string(),
            name: "Docker Desktop".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.postmanlabs.mac".to_string(),
            name: "Postman".to_string(),
            suggested_category: "development".to_string(),
        },
        KnownApp {
            bundle_id: "com.insomnia.app".to_string(),
            name: "Insomnia".to_string(),
            suggested_category: "development".to_string(),
        },
    ]
}

/// Look up a known app by bundle identifier
pub fn find_known_app(bundle_id: &str) -> Option<KnownApp> {
    get_known_applications()
        .into_iter()
        .find(|app| app.bundle_id == bundle_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_known_applications() {
        let apps = get_known_applications();
        assert!(!apps.is_empty());

        // Check that VS Code is in the list
        let vscode = apps.iter().find(|a| a.name == "Visual Studio Code");
        assert!(vscode.is_some());
        assert_eq!(vscode.unwrap().suggested_category, "development");
    }

    #[test]
    fn test_find_known_app() {
        let slack = find_known_app("com.tinyspeck.slackmacgap");
        assert!(slack.is_some());
        assert_eq!(slack.unwrap().suggested_category, "conversation");

        let unknown = find_known_app("com.unknown.app");
        assert!(unknown.is_none());
    }
}
