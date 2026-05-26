//! Navigation helpers and small utilities.

use gpui::{Context, Window};
use std::path::PathBuf;
use crate::InstallerConfig;
use super::super::{InstallerView, Page, LogLevel, LogEntry};

impl InstallerView {
    /// Switch to a new page and request a re-render.
    pub fn navigate_to(&mut self, page: Page, _window: &mut Window, cx: &mut Context<Self>) {
        self.current_page = page;
        cx.notify();
    }

    /// Append an entry to the install log and request a re-render.
    pub fn log(&mut self, level: LogLevel, message: impl Into<String>, cx: &mut Context<Self>) {
        self.log_entries.push(LogEntry { level, message: message.into() });
        cx.notify();
    }

    /// Human-readable byte count (B / KB / MB).
    pub fn format_bytes(bytes: u64) -> String {
        const MB: u64 = 1024 * 1024;
        const KB: u64 = 1024;
        if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{bytes} B")
        }
    }

    /// Build the platform-appropriate default install path and wrap it in an
    /// `InstallerConfig`.
    pub fn default_install_config() -> InstallerConfig {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));

        #[cfg(target_os = "macos")]
        let path = home.join("Applications").join("Pulsar.app");

        #[cfg(windows)]
        let path = PathBuf::from(
            std::env::var("LOCALAPPDATA")
                .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string()),
        )
        .join("Programs")
        .join("Pulsar");

        #[cfg(target_os = "linux")]
        let path = home.join(".local").join("share").join("pulsar");

        #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
        let path = home.join("pulsar");

        InstallerConfig::new(path)
    }
}
