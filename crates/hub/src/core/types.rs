use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use ui::IconName;

// ── Engine Release Notes Modal ────────────────────────────────────────────

/// A full-screen modal showing a particular engine version's release notes.
#[derive(Clone, Debug)]
pub struct ReleaseNotesModal {
    pub title: String,
    pub body: String,
}

// ── Engine Prompt ─────────────────────────────────────────────────────────

/// A pending prompt asking the user whether to auto-install a missing engine
/// that a project requires.
#[derive(Clone, Debug)]
pub struct EnginePrompt {
    pub project_name: String,
    pub project_path: String,
    pub required: String,
}

// ── Navigation ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EntryScreenView {
    Recent,
    Templates,
    NewProject,
    CloneGit,
    Versions,
    CloudProjects,
    Friends,
}

// ── Templates ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Template {
    pub name: String,
    pub description: String,
    pub icon: IconName,
    pub repo_url: String,
    pub category: String,
}

impl Template {
    pub fn new(name: &str, desc: &str, icon: IconName, repo_url: &str, category: &str) -> Self {
        Self {
            name: name.to_string(),
            description: desc.to_string(),
            icon,
            repo_url: repo_url.to_string(),
            category: category.to_string(),
        }
    }
}

// TODO: Consider loading templates from a remote repo in the github org
//       instead of hardcoding them here. This would allow for easier updates
//       and additions to the template list without requiring a new release of
//       the application.
pub fn get_default_templates() -> Vec<Template> {
    vec![
        Template::new(
            "Blank Project",
            "Empty project with minimal structure",
            IconName::Folder,
            "https://github.com/Far-Beyond-Pulsar/Template-Blank",
            "Basic",
        ),
        Template::new(
            "Core",
            "Core engine features and systems",
            IconName::Settings,
            "https://github.com/pulsar-templates/core.git",
            "Basic",
        ),
        Template::new(
            "2D Platformer",
            "Classic side-scrolling platformer",
            IconName::Gamepad,
            "https://github.com/pulsar-templates/2d-platformer.git",
            "2D",
        ),
        Template::new(
            "2D Top-Down",
            "Top-down 2D game with camera",
            IconName::Map,
            "https://github.com/pulsar-templates/2d-topdown.git",
            "2D",
        ),
        Template::new(
            "3D First Person",
            "FPS with movement and camera",
            IconName::Eye,
            "https://github.com/pulsar-templates/3d-fps.git",
            "3D",
        ),
        Template::new(
            "3D Platformer",
            "3D platformer with physics",
            IconName::Cube,
            "https://github.com/pulsar-templates/3d-platformer.git",
            "3D",
        ),
        Template::new(
            "Tower Defense",
            "Wave-based tower defense",
            IconName::Shield,
            "https://github.com/pulsar-templates/tower-defense.git",
            "Strategy",
        ),
        Template::new(
            "Action RPG",
            "Action-oriented RPG systems",
            IconName::Star,
            "https://github.com/pulsar-templates/action-rpg.git",
            "RPG",
        ),
        Template::new(
            "Visual Novel",
            "Narrative-driven visual novel",
            IconName::BookOpen,
            "https://github.com/pulsar-templates/visual-novel.git",
            "Narrative",
        ),
        Template::new(
            "Puzzle",
            "Puzzle game mechanics",
            IconName::Box,
            "https://github.com/pulsar-templates/puzzle.git",
            "Puzzle",
        ),
        Template::new(
            "Card Game",
            "Card-based game system",
            IconName::CreditCard,
            "https://github.com/pulsar-templates/card-game.git",
            "Card",
        ),
        Template::new(
            "Racing",
            "Racing game with physics",
            IconName::Rocket,
            "https://github.com/pulsar-templates/racing.git",
            "Racing",
        ),
    ]
}

// ── Clone Progress ────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct CloneProgress {
    pub current: usize,
    pub total: usize,
    pub message: String,
    pub completed: bool,
    pub error: Option<String>,
    pub cancelled: bool,
}

pub type SharedCloneProgress = Arc<Mutex<CloneProgress>>;

// ── Git Fetch Status ──────────────────────────────────────────────────────

#[derive(Clone)]
pub enum GitFetchStatus {
    NotStarted,
    Fetching,
    UpToDate,
    UpdatesAvailable(usize),
    Error(String),
}

// ── Cloud Projects ────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Default)]
pub enum CloudServerStatus {
    #[default]
    Unknown,
    Connecting,
    Online {
        latency_ms: u32,
        version: String,
        active_users: u32,
        active_projects: u32,
    },
    Offline,
    Unauthorized,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CloudProjectStatus {
    Idle,
    Preparing,
    Running { user_count: u32 },
    Error(String),
}

#[derive(Clone, Debug)]
pub struct CloudProject {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: CloudProjectStatus,
    pub last_modified: String,
    pub size_bytes: u64,
    pub owner: String,
    /// The default/root environment ID on the Studio server, if one exists.
    /// Used to connect to a per-environment session instead of the legacy lobby.
    pub environment_id: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CloudServer {
    pub id: String,
    pub alias: String,
    pub url: String,
    pub auth_token: String,
    pub username: String,
    #[serde(skip)]
    pub status: CloudServerStatus,
    #[serde(skip)]
    pub projects: Vec<CloudProject>,
}

impl Default for CloudServer {
    fn default() -> Self {
        Self {
            id: String::new(),
            alias: String::new(),
            url: String::new(),
            auth_token: String::new(),
            username: String::new(),
            status: CloudServerStatus::Unknown,
            projects: Vec::new(),
        }
    }
}

// ── Dependency Status ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct DependencyStatus {
    pub rust_installed: bool,
    pub build_tools_installed: bool,
    pub compiler_info: Option<String>,
}

#[derive(Clone, Debug)]
pub struct InstallProgress {
    pub logs: Vec<String>,
    pub progress: f32,
    pub status: InstallStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InstallStatus {
    Idle,
    Downloading,
    Installing,
    Complete,
    Error(String),
}

// ── Plugin System ─────────────────────────────────────────────────────────

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PluginRegistry {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RegistryPlugin {
    pub name: String,
    pub description: String,
    pub repo_url: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip)]
    pub registry_url: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PluginInstallMethod {
    BinaryDownload,
    BuiltFromSource,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstalledPlugin {
    pub name: String,
    pub repo_url: String,
    pub version: String,
    pub installed_at: String,
    pub install_method: PluginInstallMethod,
    pub library_path: String,
}

#[derive(Clone, Debug)]
pub enum PluginInstallPhase {
    FetchingMetadata,
    Downloading { progress: f32 },
    Building { logs: Vec<String> },
    Complete(InstalledPlugin),
    Error(String),
}

// ── Onboarding ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum OnboardingTab {
    #[default]
    Theme,
    Plugins,
}

// ── Pending Invite ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PendingInvite {
    pub from_username: String,
    pub from_home_server: Option<String>,
    pub message: String,
    pub notification_id: String,
}

// ── Download Manager ────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum DownloadKind {
    EngineVersion { version: String },
    TemplateClone { name: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DownloadStatus {
    Downloading {
        bytes_downloaded: u64,
        total_bytes: u64,
        speed_bps: u64,
    },
    Complete,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct DownloadItem {
    pub id: String,
    pub kind: DownloadKind,
    pub status: DownloadStatus,
    pub started_at: std::time::Instant,
}

impl DownloadItem {
    pub fn label(&self) -> String {
        match &self.kind {
            DownloadKind::EngineVersion { version } => format!("Engine v{}", version),
            DownloadKind::TemplateClone { name } => format!("Template: {}", name),
        }
    }

    pub fn progress_fraction(&self) -> f32 {
        match &self.status {
            DownloadStatus::Downloading {
                bytes_downloaded,
                total_bytes,
                ..
            } => {
                if *total_bytes > 0 {
                    (*bytes_downloaded as f32 / *total_bytes as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    pub fn downloaded_display(&self) -> String {
        match &self.status {
            DownloadStatus::Downloading {
                bytes_downloaded,
                total_bytes,
                ..
            } => format!("{} / {}", format_bytes(*bytes_downloaded), format_bytes(*total_bytes)),
            DownloadStatus::Complete => "Complete".to_string(),
            DownloadStatus::Failed(e) => e.clone(),
        }
    }

    pub fn speed_display(&self) -> String {
        match &self.status {
            DownloadStatus::Downloading { speed_bps, .. } => {
                if *speed_bps > 0 {
                    format!("{}/s", format_bytes(*speed_bps))
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;
    const PB: u64 = 1024 * TB;
    if bytes >= PB {
        format!("{:.1} PB", bytes as f64 / PB as f64)
    } else if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// ── Version Management ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct VersionState {
    pub installed: Vec<crate::service::installer_service::InstalledVersion>,
    pub available_releases: Vec<crate::service::installer_service::GitHubRelease>,
    pub install_state: crate::service::installer_service::VersionInstallState,
    pub fetching: bool,
    /// True while an additional page of releases is being fetched.
    pub loading_more: bool,
    /// True if any source repo still has more pages to load.
    pub has_more: bool,
    /// Currently enabled release channels.
    pub selected_channels: Vec<crate::service::installer_service::ReleaseChannel>,
    /// Per-repo pagination/loading state backing `available_releases`.
    pub channel_sources: Vec<crate::service::installer_service::ChannelSource>,
}

impl Default for VersionState {
    fn default() -> Self {
        Self {
            installed: Vec::new(),
            available_releases: Vec::new(),
            install_state: crate::service::installer_service::VersionInstallState::Idle,
            fetching: false,
            loading_more: false,
            has_more: true,
            selected_channels: crate::service::installer_service::ReleaseChannel::ALL
                .to_vec()
                .into_iter()
                .filter(|c| *c != crate::service::installer_service::ReleaseChannel::Nightly)
                .collect(),
            channel_sources: crate::service::installer_service::default_channel_sources(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VersionRemoveRequested {
    pub version: String,
}

#[derive(Clone, Debug)]
pub struct VersionInstallRequested {
    pub version: String,
}
