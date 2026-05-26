//! Domain types shared across the entire installer UI.

use gpui_component::IconName;

// ─── Page enum ────────────────────────────────────────────────────────────────

/// All top-level pages / modes the installer can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Welcome,
    License,
    VersionSelection,
    ReleaseNotes,
    InstallOptions,
    Installing,
    Complete,
    VersionsManager,
}

impl Page {
    /// 0-based index within the linear wizard flow.
    /// Returns `None` for `VersionsManager` which lives outside the flow.
    pub fn wizard_index(self) -> Option<usize> {
        match self {
            Page::Welcome          => Some(0),
            Page::License          => Some(1),
            Page::VersionSelection => Some(2),
            Page::ReleaseNotes     => Some(3),
            Page::InstallOptions   => Some(4),
            Page::Installing       => Some(5),
            Page::Complete         => Some(6),
            Page::VersionsManager  => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Page::Welcome          => "Welcome",
            Page::License          => "License",
            Page::VersionSelection => "Select Version",
            Page::ReleaseNotes     => "Release Notes",
            Page::InstallOptions   => "Install Options",
            Page::Installing       => "Installing",
            Page::Complete         => "Complete",
            Page::VersionsManager  => "Installed Versions",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Page::Welcome          => IconName::Bot,
            Page::License          => IconName::BookOpen,
            Page::VersionSelection => IconName::Github,
            Page::ReleaseNotes     => IconName::Notes,
            Page::InstallOptions   => IconName::Settings,
            Page::Installing       => IconName::HardDrive,
            Page::Complete         => IconName::CircleCheck,
            Page::VersionsManager  => IconName::Inbox,
        }
    }
}

/// Ordered wizard steps (excludes `VersionsManager`).
pub const WIZARD_STEPS: [Page; 7] = [
    Page::Welcome,
    Page::License,
    Page::VersionSelection,
    Page::ReleaseNotes,
    Page::InstallOptions,
    Page::Installing,
    Page::Complete,
];

/// Optional sidecar packages that can be co-installed with the engine.
/// Tuple: `(asset_prefix, display_name, description)`.
pub const SIDECAR_PACKAGES: &[(&str, &str, &str)] = &[
    (
        "pulsar-host",
        "Pulsar Host",
        "Host process for spawning and managing engine instances.",
    ),
    (
        "pulsar-multiedit",
        "Pulsar Multi-Edit",
        "Multi-user concurrent editing extension.",
    ),
];

// ─── Data structs ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub prerelease: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}
