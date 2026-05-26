//! Main installer view — full installer UI with license, version selection,
//! install options, progress, and installed-versions manager.

use gpui::{
    App, AppContext as _, Context, Entity, Focusable, FontWeight, IntoElement, ParentElement,
    InteractiveElement as _, Render, StatefulInteractiveElement as _, Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme, Disableable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, v_flex,
    Icon, IconName,
    progress::Progress,
    spinner::Spinner,
};
use gpui::prelude::FluentBuilder;
use gpui::http_client::{HttpClient, http, AsyncBody};
use futures::AsyncReadExt;
use reqwest_client::ReqwestClient;
use crate::download::{GitHubReleases, HttpDownloadManager, GitHubAsset};
use crate::traits::DownloadManager as _;
use crate::InstallerConfig;
use crate::installed_versions::{InstalledVersion, scan_installed_versions, write_metadata};
use std::path::PathBuf;

// ─── Page enum ────────────────────────────────────────────────────────────────

/// All pages/modes the installer can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Welcome,
    License,
    VersionSelection,
    InstallOptions,
    Installing,
    Complete,
    VersionsManager,
}

impl Page {
    /// 0-based index within the wizard flow (VersionsManager is excluded).
    pub fn wizard_index(self) -> Option<usize> {
        match self {
            Page::Welcome         => Some(0),
            Page::License         => Some(1),
            Page::VersionSelection=> Some(2),
            Page::InstallOptions  => Some(3),
            Page::Installing      => Some(4),
            Page::Complete        => Some(5),
            Page::VersionsManager => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Page::Welcome         => "Welcome",
            Page::License         => "License",
            Page::VersionSelection=> "Select Version",
            Page::InstallOptions  => "Install Options",
            Page::Installing      => "Installing",
            Page::Complete        => "Complete",
            Page::VersionsManager => "Installed Versions",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Page::Welcome         => IconName::Bot,
            Page::License         => IconName::BookOpen,
            Page::VersionSelection=> IconName::Github,
            Page::InstallOptions  => IconName::Settings,
            Page::Installing      => IconName::HardDrive,
            Page::Complete        => IconName::CircleCheck,
            Page::VersionsManager => IconName::Inbox,
        }
    }
}

const WIZARD_STEPS: [Page; 6] = [
    Page::Welcome,
    Page::License,
    Page::VersionSelection,
    Page::InstallOptions,
    Page::Installing,
    Page::Complete,
];

// ─── Data types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: String,
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

// ─── View state ───────────────────────────────────────────────────────────────

pub struct InstallerView {
    pub focus_handle: gpui::FocusHandle,
    pub current_page: Page,

    // ── License ──────────────────────────────────────────────────────────────
    pub license_text: Option<String>,
    pub loading_license: bool,
    pub license_fetch_error: Option<String>,
    pub license_accepted: bool,

    // ── Version selection ────────────────────────────────────────────────────
    pub releases: Vec<ReleaseInfo>,
    pub loading_releases: bool,
    pub loading_more: bool,
    pub current_releases_page: u32,
    pub has_more_releases: bool,
    pub show_prereleases: bool,
    /// Index into `releases` of the currently selected release.
    pub selected_release_idx: Option<usize>,
    pub selected_asset_size: Option<u64>,

    // ── Install options ──────────────────────────────────────────────────────
    pub install_config: InstallerConfig,

    // ── Installing ───────────────────────────────────────────────────────────
    pub install_progress: f32,
    pub install_message: String,
    pub log_entries: Vec<LogEntry>,
    pub install_failed: bool,
    /// Set on successful installation.
    pub installed_path: Option<PathBuf>,

    // ── Versions manager ─────────────────────────────────────────────────────
    pub installed_versions: Vec<InstalledVersion>,
    pub loading_installed: bool,
    /// Index into `installed_versions` awaiting uninstall confirmation.
    pub uninstall_confirm: Option<usize>,
}

// ─── Constructor ──────────────────────────────────────────────────────────────

impl InstallerView {
    pub fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(cx))
    }

    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            current_page: Page::Welcome,
            // License
            license_text: None,
            loading_license: false,
            license_fetch_error: None,
            license_accepted: false,
            // Version selection
            releases: Vec::new(),
            loading_releases: false,
            loading_more: false,
            current_releases_page: 0,
            has_more_releases: true,
            show_prereleases: false,
            selected_release_idx: None,
            selected_asset_size: None,
            // Install options
            install_config: Self::default_install_config(),
            // Installing
            install_progress: 0.0,
            install_message: String::new(),
            log_entries: Vec::new(),
            install_failed: false,
            installed_path: None,
            // Versions manager
            installed_versions: Vec::new(),
            loading_installed: false,
            uninstall_confirm: None,
        }
    }

    fn default_install_config() -> InstallerConfig {
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

// ─── Navigation helpers ───────────────────────────────────────────────────────

impl InstallerView {
    pub fn navigate_to(&mut self, page: Page, _window: &mut Window, cx: &mut Context<Self>) {
        self.current_page = page;
        cx.notify();
    }

    pub fn log(&mut self, level: LogLevel, message: impl Into<String>, cx: &mut Context<Self>) {
        self.log_entries.push(LogEntry { level, message: message.into() });
        cx.notify();
    }

    fn format_bytes(bytes: u64) -> String {
        const MB: u64 = 1024 * 1024;
        const KB: u64 = 1024;
        if bytes >= MB { format!("{:.1} MB", bytes as f64 / MB as f64) }
        else if bytes >= KB { format!("{:.1} KB", bytes as f64 / KB as f64) }
        else { format!("{bytes} B") }
    }
}

// ─── License fetching ─────────────────────────────────────────────────────────

impl InstallerView {
    pub fn fetch_license(&mut self, cx: &mut Context<Self>) {
        self.loading_license = true;
        self.license_fetch_error = None;
        self.license_text = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let client = match ReqwestClient::user_agent("Pulsar-Installer/1.0") {
                Ok(c) => c,
                Err(e) => {
                    this.update(cx, |v, cx| {
                        v.license_fetch_error = Some(format!("Client error: {e}"));
                        v.loading_license = false;
                        cx.notify();
                    }).ok();
                    return;
                }
            };
            let url = "https://raw.githubusercontent.com/Far-Beyond-Pulsar/Pulsar-Native/main/LICENSE.md";
            let request = match http::Request::builder()
                .method("GET")
                .uri(url)
                .body(AsyncBody::default())
            {
                Ok(r) => r,
                Err(e) => {
                    this.update(cx, |v, cx| {
                        v.license_fetch_error = Some(format!("Request build error: {e}"));
                        v.loading_license = false;
                        cx.notify();
                    }).ok();
                    return;
                }
            };
            match client.send(request).await {
                Ok(mut response) => {
                    let mut body = Vec::new();
                    if response.body_mut().read_to_end(&mut body).await.is_ok() {
                        let text = String::from_utf8_lossy(&body).into_owned();
                        this.update(cx, |v, cx| {
                            v.license_text = Some(text);
                            v.loading_license = false;
                            cx.notify();
                        }).ok();
                    } else {
                        this.update(cx, |v, cx| {
                            v.license_fetch_error = Some("Failed to read response body".to_string());
                            v.loading_license = false;
                            cx.notify();
                        }).ok();
                    }
                }
                Err(e) => {
                    this.update(cx, |v, cx| {
                        v.license_fetch_error = Some(format!("HTTP error: {e}"));
                        v.loading_license = false;
                        cx.notify();
                    }).ok();
                }
            }
        }).detach();
    }
}

// ─── Release fetching ─────────────────────────────────────────────────────────

impl InstallerView {
    pub fn fetch_releases(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.loading_releases = true;
        self.current_releases_page = 1;
        self.releases.clear();
        self.selected_release_idx = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let github = GitHubReleases::new("Far-Beyond-Pulsar", "Pulsar-Native");
            match github.get_releases_page(1, 30).await {
                Ok(releases) => {
                    let has_more = releases.len() >= 30;
                    let infos: Vec<ReleaseInfo> = releases
                        .into_iter()
                        .map(|r| ReleaseInfo {
                            tag_name: r.tag_name.clone(),
                            name: r.name.clone(),
                            prerelease: r.prerelease,
                        })
                        .collect();
                    this.update(cx, |v, cx| {
                        v.releases = infos;
                        v.loading_releases = false;
                        v.has_more_releases = has_more;
                        cx.notify();
                    }).ok();
                }
                Err(e) => {
                    tracing::error!("Failed to fetch releases: {}", e);
                    this.update(cx, |v, cx| {
                        v.loading_releases = false;
                        cx.notify();
                    }).ok();
                }
            }
        }).detach();
    }

    pub fn load_more_releases(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.loading_more || !self.has_more_releases { return; }
        self.loading_more = true;
        self.current_releases_page += 1;
        let page = self.current_releases_page;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let github = GitHubReleases::new("Far-Beyond-Pulsar", "Pulsar-Native");
            match github.get_releases_page(page, 30).await {
                Ok(releases) => {
                    let has_more = releases.len() >= 30;
                    let infos: Vec<ReleaseInfo> = releases
                        .into_iter()
                        .map(|r| ReleaseInfo {
                            tag_name: r.tag_name.clone(),
                            name: r.name.clone(),
                            prerelease: r.prerelease,
                        })
                        .collect();
                    this.update(cx, |v, cx| {
                        v.releases.extend(infos);
                        v.loading_more = false;
                        v.has_more_releases = has_more;
                        cx.notify();
                    }).ok();
                }
                Err(e) => {
                    tracing::error!("Failed to fetch more releases: {}", e);
                    this.update(cx, |v, cx| {
                        v.loading_more = false;
                        v.current_releases_page -= 1;
                        cx.notify();
                    }).ok();
                }
            }
        }).detach();
    }

    pub fn select_release(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_release_idx = Some(index);
        cx.notify();
    }
}

// ─── Native file picker ───────────────────────────────────────────────────────

impl InstallerView {
    pub fn open_native_picker(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = smol::unblock(|| -> Option<String> {
                #[cfg(target_os = "macos")]
                {
                    let output = std::process::Command::new("osascript")
                        .args([
                            "-e",
                            "POSIX path of (choose folder with prompt \"Select install folder:\")",
                        ])
                        .output();
                    return output.ok().and_then(|o| {
                        if o.status.success() {
                            String::from_utf8(o.stdout).ok()
                                .map(|s| s.trim().trim_end_matches('/').to_string())
                                .filter(|s| !s.is_empty())
                        } else {
                            None
                        }
                    });
                }
                #[cfg(target_os = "linux")]
                {
                    let output = std::process::Command::new("zenity")
                        .args(["--file-selection", "--directory", "--title=Select install folder"])
                        .output()
                        .or_else(|_| {
                            std::process::Command::new("kdialog")
                                .args(["--getexistingdirectory", "/home"])
                                .output()
                        });
                    return output.ok().and_then(|o| {
                        if o.status.success() {
                            String::from_utf8(o.stdout).ok()
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                        } else {
                            None
                        }
                    });
                }
                #[cfg(windows)]
                {
                    let ps = r#"
Add-Type -AssemblyName System.Windows.Forms
$f = New-Object System.Windows.Forms.FolderBrowserDialog
$f.Description = 'Select install folder'
$null = $f.ShowDialog()
$f.SelectedPath
"#;
                    let output = std::process::Command::new("powershell")
                        .args(["-NoProfile", "-Command", ps])
                        .output();
                    return output.ok().and_then(|o| {
                        if o.status.success() {
                            String::from_utf8(o.stdout).ok()
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                        } else {
                            None
                        }
                    });
                }
                #[allow(unreachable_code)]
                None
            }).await;

            if let Some(path_str) = result {
                let path = PathBuf::from(path_str);
                this.update(cx, |v, cx| {
                    v.install_config.install_path = path;
                    cx.notify();
                }).ok();
            }
        }).detach();
    }
}

// ─── Installed versions loader ────────────────────────────────────────────────

impl InstallerView {
    pub fn load_installed_versions(&mut self, cx: &mut Context<Self>) {
        self.loading_installed = true;
        self.installed_versions.clear();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let versions = smol::unblock(scan_installed_versions).await;
            this.update(cx, |v, cx| {
                v.installed_versions = versions;
                v.loading_installed = false;
                cx.notify();
            }).ok();
        }).detach();
    }

    pub fn uninstall_version(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(ver) = self.installed_versions.get(index) {
            let path = ver.metadata.install_path.clone();
            self.uninstall_confirm = None;
            cx.notify();

            cx.spawn(async move |this, cx| {
                let path_for_retain = path.clone();
                let result = smol::unblock(move || std::fs::remove_dir_all(&path)).await;
                match result {
                    Ok(_) => {
                        this.update(cx, |v, cx| {
                            v.installed_versions.retain(|iv| iv.metadata.install_path != path_for_retain);
                            cx.notify();
                        }).ok();
                    }
                    Err(e) => {
                        tracing::error!("Uninstall failed: {e}");
                    }
                }
            }).detach();
        }
    }
}

// ─── Asset selection ──────────────────────────────────────────────────────────

impl InstallerView {
    /// Pick the best asset from a release for the current OS + arch.
    fn select_asset(assets: &[GitHubAsset]) -> Option<GitHubAsset> {
        let os_token = match std::env::consts::OS {
            "linux"   => "linux",
            "macos"   => "macos",
            "windows" => "windows",
            other => {
                tracing::warn!("Unrecognised OS '{}', cannot select asset", other);
                return None;
            }
        };
        let arch_token = match std::env::consts::ARCH {
            "x86_64"  => "x86_64",
            "aarch64" => "arm64",
            other => {
                tracing::warn!("Unrecognised arch '{}', cannot select asset", other);
                return None;
            }
        };
        let candidates: Vec<&GitHubAsset> = assets
            .iter()
            .filter(|a| {
                let n = &a.name;
                !n.ends_with(".sig") && n.contains(os_token) && n.contains(arch_token)
            })
            .collect();
        tracing::info!(
            "Asset selection: os={os_token} arch={arch_token} → {} candidate(s)",
            candidates.len()
        );
        if candidates.is_empty() { return None; }
        #[cfg(target_os = "macos")]
        if let Some(app_zip) = candidates.iter().find(|a| a.name.ends_with(".app.zip")) {
            return Some((*app_zip).clone());
        }
        candidates.into_iter().next().cloned()
    }
}

// ─── Installation ─────────────────────────────────────────────────────────────

impl InstallerView {
    pub fn start_installation(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(idx) = self.selected_release_idx else {
            self.install_message = "No version selected.".to_string();
            cx.notify();
            return;
        };
        let Some(release_info) = self.releases.get(idx).cloned() else {
            self.install_message = "Invalid selection.".to_string();
            cx.notify();
            return;
        };

        self.install_progress = 0.0;
        self.install_message = "Preparing installation…".to_string();
        self.log_entries.clear();
        self.install_failed = false;
        self.installed_path = None;
        let install_dir = self.install_config.install_path.clone();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let download_manager = HttpDownloadManager::new();
            let github = GitHubReleases::new("Far-Beyond-Pulsar", "Pulsar-Native");

            let download_dir = std::env::temp_dir().join("pulsar-installer");
            if let Err(e) = std::fs::create_dir_all(&download_dir) {
                this.update(cx, |v, cx| {
                    v.install_message = format!("Failed to create temp dir: {e}");
                    v.install_failed = true;
                    v.log(LogLevel::Error, format!("Temp dir error: {e}"), cx);
                }).ok();
                return;
            }

            this.update(cx, |v, cx| {
                v.install_message = format!("Resolving assets for {}…", release_info.name);
                v.log(LogLevel::Info, format!("Resolving {}", release_info.tag_name), cx);
            }).ok();

            let (full_release, asset) = match github.get_all_releases().await {
                Ok(releases) => {
                    match releases.into_iter().find(|r| r.tag_name == release_info.tag_name) {
                        Some(rel) => {
                            match Self::select_asset(&rel.assets) {
                                Some(a) => (rel, a),
                                None => {
                                    this.update(cx, |v, cx| {
                                        v.install_message = "No compatible asset found for your platform.".to_string();
                                        v.install_failed = true;
                                        v.log(LogLevel::Error, "No compatible asset found.", cx);
                                    }).ok();
                                    return;
                                }
                            }
                        }
                        None => {
                            this.update(cx, |v, cx| {
                                v.install_message = "Release not found on GitHub.".to_string();
                                v.install_failed = true;
                                v.log(LogLevel::Error, "Release not found.", cx);
                            }).ok();
                            return;
                        }
                    }
                }
                Err(e) => {
                    this.update(cx, |v, cx| {
                        v.install_message = format!("Failed to fetch releases: {e}");
                        v.install_failed = true;
                        v.log(LogLevel::Error, format!("Fetch error: {e}"), cx);
                    }).ok();
                    return;
                }
            };

            let asset_name = asset.name.clone();
            let total_size = asset.size;
            let file_path = download_dir.join(&asset.name);

            this.update(cx, |v, cx| {
                v.install_message = format!("Downloading {} ({})", asset_name, Self::format_bytes(total_size));
                v.log(LogLevel::Info, format!("Downloading {} ({})", asset_name, Self::format_bytes(total_size)), cx);
            }).ok();

            let progress_state = std::sync::Arc::new(std::sync::Mutex::new((0u64, 0.0f32)));
            let progress_state_clone = progress_state.clone();
            let url = asset.browser_download_url.clone();
            let file_path_dl = file_path.clone();

            let download_task = {
                let dm = download_manager.clone();
                smol::spawn(async move {
                    dm.download(&url, &file_path_dl, Box::new(move |prog| {
                        *progress_state_clone.lock().unwrap() = (prog.processed_bytes, prog.current);
                    })).await
                })
            };

            let mut last_update = std::time::Instant::now();
            loop {
                if download_task.is_finished() { break; }
                if last_update.elapsed() >= std::time::Duration::from_millis(100) {
                    let (processed, file_pct) = *progress_state.lock().unwrap();
                    let overall_pct = if total_size > 0 {
                        (processed as f32 / total_size as f32) * 100.0
                    } else {
                        file_pct
                    };
                    let msg = format!("Downloading {} — {:.1}%", asset_name, file_pct);
                    this.update(cx, |v, cx| {
                        v.install_progress = overall_pct * 0.9; // reserve last 10% for extraction
                        v.install_message = msg;
                        cx.notify();
                    }).ok();
                    last_update = std::time::Instant::now();
                }
                smol::Timer::after(std::time::Duration::from_millis(50)).await;
            }

            match download_task.await {
                Ok(_) => {
                    this.update(cx, |v, cx| {
                        v.log(LogLevel::Success, format!("Downloaded {}", asset_name), cx);
                        v.install_message = format!("Installing {}…", full_release.name);
                        v.install_progress = 90.0;
                        cx.notify();
                    }).ok();

                    match Self::install_release(&file_path, &install_dir, &release_info.tag_name).await {
                        Ok(()) => {
                            // Write install metadata
                            let _ = write_metadata(&install_dir, &release_info.tag_name);

                            this.update(cx, |v, cx| {
                                v.install_progress = 100.0;
                                v.install_message = "Installation complete!".to_string();
                                v.installed_path = Some(install_dir.clone());
                                v.log(LogLevel::Success, format!("Installed → {}", install_dir.display()), cx);
                                v.log(LogLevel::Success, "All done! Pulsar is ready.", cx);
                                v.current_page = Page::Complete;
                                cx.notify();
                            }).ok();
                        }
                        Err(e) => {
                            this.update(cx, |v, cx| {
                                v.install_progress = 100.0;
                                v.install_message = "Installation completed with errors.".to_string();
                                v.install_failed = true;
                                v.log(LogLevel::Error, format!("Install failed: {e}"), cx);
                                v.current_page = Page::Complete;
                                cx.notify();
                            }).ok();
                        }
                    }
                }
                Err(e) => {
                    this.update(cx, |v, cx| {
                        v.install_progress = 100.0;
                        v.install_message = "Download failed.".to_string();
                        v.install_failed = true;
                        v.log(LogLevel::Error, format!("Download failed: {e}"), cx);
                        v.current_page = Page::Complete;
                        cx.notify();
                    }).ok();
                }
            }
        }).detach();
    }

    async fn install_release(
        archive_path: &PathBuf,
        install_dir: &PathBuf,
        version: &str,
    ) -> crate::error::Result<()> {
        use std::fs;

        fs::create_dir_all(install_dir).map_err(crate::error::InstallerError::Io)?;

        let archive_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let is_app_zip = archive_name.ends_with(".app.zip");

        if is_app_zip || archive_name.ends_with(".zip") {
            // For pre-built .app.zip bundles, extract into the *parent* directory so that
            // the zip's top-level "Pulsar.app/" lands at the expected install_dir path,
            // rather than being nested inside it.
            let extract_root = if is_app_zip {
                install_dir.parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| install_dir.clone())
            } else {
                install_dir.clone()
            };
            fs::create_dir_all(&extract_root).map_err(crate::error::InstallerError::Io)?;
            let file = fs::File::open(archive_path).map_err(crate::error::InstallerError::Io)?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| crate::error::InstallerError::Other(e.to_string()))?;
            for i in 0..archive.len() {
                let mut zf = archive.by_index(i)
                    .map_err(|e| crate::error::InstallerError::Other(e.to_string()))?;
                let out = extract_root.join(zf.mangled_name());
                if zf.name().ends_with('/') {
                    fs::create_dir_all(&out).map_err(crate::error::InstallerError::Io)?;
                } else {
                    if let Some(p) = out.parent() {
                        fs::create_dir_all(p).map_err(crate::error::InstallerError::Io)?;
                    }
                    let mut outfile = fs::File::create(&out).map_err(crate::error::InstallerError::Io)?;
                    std::io::copy(&mut zf, &mut outfile).map_err(crate::error::InstallerError::Io)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mode = zf.unix_mode().unwrap_or(0o755);
                        if mode & 0o100 != 0 {
                            fs::set_permissions(&out, fs::Permissions::from_mode(mode))
                                .map_err(crate::error::InstallerError::Io)?;
                        }
                    }
                }
            }
        } else if archive_name.ends_with(".tar.gz") || archive_name.ends_with(".tgz") {
            let file = fs::File::open(archive_path).map_err(crate::error::InstallerError::Io)?;
            let tar = flate2::read::GzDecoder::new(file);
            let mut archive = tar::Archive::new(tar);
            archive.unpack(install_dir).map_err(crate::error::InstallerError::Io)?;
        } else if archive_name.ends_with(".exe") {
            let dest = install_dir.join("pulsar.exe");
            fs::copy(archive_path, &dest).map_err(crate::error::InstallerError::Io)?;
        } else {
            let dest = install_dir.join("pulsar");
            fs::copy(archive_path, &dest).map_err(crate::error::InstallerError::Io)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))
                    .map_err(crate::error::InstallerError::Io)?;
            }
        }

        #[cfg(windows)]
        {
            use crate::platform::WindowsInstaller;
            use crate::traits::{Progress as Prog, ProgressCallback};
            let installer = WindowsInstaller::new(install_dir.clone(), version.to_string());
            let progress: ProgressCallback = Box::new(|p: Prog| {
                tracing::info!("[{}%] {}", p.current, p.message.unwrap_or(""));
            });
            installer.install(progress).await?;
        }

        #[cfg(target_os = "macos")]
        {
            // If the asset was a pre-built .app.zip the bundle is already complete;
            // just ensure the binary is executable.
            if is_app_zip {
                use std::os::unix::fs::PermissionsExt;
                // Walk Contents/MacOS/ and chmod +x every file there.
                let macos_dir = install_dir.join("Contents").join("MacOS");
                if let Ok(entries) = fs::read_dir(&macos_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o755));
                        }
                    }
                }
            } else {
                use crate::platform::MacOSInstaller;
                use crate::traits::{Progress as Prog, ProgressCallback};
                let binary_name = "pulsar".to_string();
                let source_binary = install_dir.join("Contents").join("MacOS").join(&binary_name);
                let installer = MacOSInstaller::new(install_dir.clone(), version.to_string(), binary_name);
                let progress: ProgressCallback = Box::new(|p: Prog| {
                    tracing::info!("[{}%] {}", p.current, p.message.unwrap_or(""));
                });
                installer.install(&source_binary, progress).await?;
            }
        }

        #[cfg(target_os = "linux")]
        {
            use crate::platform::LinuxInstaller;
            use crate::traits::{Progress as Prog, ProgressCallback};
            let installer = LinuxInstaller::new(version.to_string(), false);
            let source_binary = install_dir.join("pulsar");
            let progress: ProgressCallback = Box::new(|p: Prog| {
                tracing::info!("[{}%] {}", p.current, p.message.unwrap_or(""));
            });
            installer.install(&source_binary, progress).await?;
        }

        Ok(())
    }
}

// ─── Open folder / launch helpers ─────────────────────────────────────────────

impl InstallerView {
    pub fn open_install_folder(&self, cx: &mut Context<Self>) {
        if let Some(path) = self.installed_path.clone() {
            cx.spawn(async move |_, _| {
                let _ = smol::unblock(move || {
                    #[cfg(target_os = "macos")]
                    { let _ = std::process::Command::new("open").arg(&path).spawn(); }
                    #[cfg(windows)]
                    { let _ = std::process::Command::new("explorer").arg(&path).spawn(); }
                    #[cfg(target_os = "linux")]
                    { let _ = std::process::Command::new("xdg-open").arg(&path).spawn(); }
                }).await;
            }).detach();
        }
    }

    pub fn open_folder_path(&self, path: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |_, _| {
            let _ = smol::unblock(move || {
                #[cfg(target_os = "macos")]
                { let _ = std::process::Command::new("open").arg(&path).spawn(); }
                #[cfg(windows)]
                { let _ = std::process::Command::new("explorer").arg(&path).spawn(); }
                #[cfg(target_os = "linux")]
                { let _ = std::process::Command::new("xdg-open").arg(&path).spawn(); }
            }).await;
        }).detach();
    }

    pub fn launch_pulsar(&self, cx: &mut Context<Self>) {
        if let Some(path) = self.installed_path.clone() {
            cx.spawn(async move |_, _| {
                let _ = smol::unblock(move || {
                    #[cfg(target_os = "macos")]
                    { let _ = std::process::Command::new("open").arg("-a").arg(&path).spawn(); }
                    #[cfg(windows)]
                    { let _ = std::process::Command::new(path.join("pulsar.exe")).spawn(); }
                    #[cfg(target_os = "linux")]
                    { let _ = std::process::Command::new(path.join("pulsar")).spawn(); }
                }).await;
            }).detach();
        }
    }

    pub fn launch_version_path(&self, path: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |_, _| {
            let _ = smol::unblock(move || {
                #[cfg(target_os = "macos")]
                { let _ = std::process::Command::new("open").arg("-a").arg(&path).spawn(); }
                #[cfg(windows)]
                { let _ = std::process::Command::new(path.join("pulsar.exe")).spawn(); }
                #[cfg(target_os = "linux")]
                { let _ = std::process::Command::new(path.join("pulsar")).spawn(); }
            }).await;
        }).detach();
    }
}

// ─── Focusable / Render ───────────────────────────────────────────────────────

impl Focusable for InstallerView {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InstallerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().background)
            .flex()
            .flex_col()
            .child(self.render_title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_sidebar(cx))
                    .child(self.render_main_content(cx)),
            )
    }
}

// ─── Layout regions ───────────────────────────────────────────────────────────

impl InstallerView {
    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .h(px(36.0))
            .bg(cx.theme().sidebar)
            .border_b_1()
            .border_color(cx.theme().border)
            .pl(px(80.0))
            .items_center()
            .justify_between()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("Pulsar Installer"),
                    ),
            )
            .child(
                div()
                    .pr_4()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.current_page.label()),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_manager = self.current_page == Page::VersionsManager;
        let current_wizard_idx = self.current_page.wizard_index();

        v_flex()
            .w(px(220.0))
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .flex_shrink_0()
            // ── Logo area ──
            .child(
                v_flex()
                    .px_4()
                    .py_5()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .rounded(px(8.0))
                                    .bg(cx.theme().accent)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().accent_foreground)
                                            .child("P"),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_0()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("Pulsar Engine"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Installer"),
                                    ),
                            ),
                    ),
            )
            // ── Navigation ──
            .child(
                v_flex()
                    .flex_1()
                    .px_3()
                    .py_4()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted_foreground)
                            .px_2()
                            .pb_2()
                            .child(if is_manager { "MANAGEMENT" } else { "INSTALLATION STEPS" }),
                    )
                    .when(!is_manager, |el| {
                        el.children(WIZARD_STEPS.iter().enumerate().map(|(idx, &step)| {
                            let is_active = current_wizard_idx == Some(idx);
                            let is_done = current_wizard_idx.map_or(false, |cur| idx < cur);

                            h_flex()
                                .gap_3()
                                .px_2()
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .items_center()
                                .when(is_active, |e| e.bg(cx.theme().accent.opacity(0.12)))
                                .child(
                                    div()
                                        .w(px(22.0))
                                        .h(px(22.0))
                                        .rounded_full()
                                        .flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .justify_center()
                                        .when(is_done, |e| {
                                            e.bg(cx.theme().accent.opacity(0.2))
                                             .child(
                                                 Icon::new(IconName::Check)
                                                     .with_size(px(12.0))
                                                     .text_color(cx.theme().accent),
                                             )
                                        })
                                        .when(is_active, |e| {
                                            e.bg(cx.theme().accent)
                                             .child(
                                                 div()
                                                     .text_xs()
                                                     .font_weight(FontWeight::BOLD)
                                                     .text_color(cx.theme().accent_foreground)
                                                     .child(format!("{}", idx + 1)),
                                             )
                                        })
                                        .when(!is_active && !is_done, |e| {
                                            e.border_1()
                                             .border_color(cx.theme().border)
                                             .child(
                                                 div()
                                                     .text_xs()
                                                     .text_color(cx.theme().muted_foreground)
                                                     .child(format!("{}", idx + 1)),
                                             )
                                        }),
                                )
                                .child(
                                    h_flex()
                                        .flex_1()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Icon::new(step.icon())
                                                .with_size(px(14.0))
                                                .text_color(if is_active {
                                                    cx.theme().accent
                                                } else if is_done {
                                                    cx.theme().accent.opacity(0.6)
                                                } else {
                                                    cx.theme().muted_foreground
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(if is_active {
                                                    FontWeight::SEMIBOLD
                                                } else {
                                                    FontWeight::NORMAL
                                                })
                                                .text_color(if is_active {
                                                    cx.theme().foreground
                                                } else if is_done {
                                                    cx.theme().muted_foreground
                                                } else {
                                                    cx.theme().muted_foreground.opacity(0.6)
                                                })
                                                .child(step.label()),
                                        ),
                                )
                        }))
                    })
                    .when(is_manager, |el| {
                        el.child(
                            h_flex()
                                .gap_3()
                                .px_2()
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .items_center()
                                .bg(cx.theme().accent.opacity(0.12))
                                .child(
                                    Icon::new(IconName::Inbox)
                                        .with_size(px(14.0))
                                        .text_color(cx.theme().accent),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().foreground)
                                        .child("Installed Versions"),
                                ),
                        )
                    }),
            )
            // ── Footer ──
            .child(
                v_flex()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .gap_2()
                    .when(self.current_page != Page::Installing, |el| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().accent)
                                .cursor_pointer()
                                .child(if is_manager {
                                    "← Install New"
                                } else {
                                    "Manage Installed ▸"
                                }),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Far Beyond Pulsar"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground.opacity(0.5))
                            .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
                    ),
            )
    }

    fn render_main_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .map(|el| match self.current_page {
                Page::Welcome         => el.child(self.render_welcome(cx)),
                Page::License         => el.child(self.render_license(cx)),
                Page::VersionSelection=> el.child(self.render_version_selection(cx)),
                Page::InstallOptions  => el.child(self.render_install_options(cx)),
                Page::Installing      => el.child(self.render_installing(cx)),
                Page::Complete        => el.child(self.render_complete(cx)),
                Page::VersionsManager => el.child(self.render_versions_manager(cx)),
            })
    }

    // ── Shared helpers ────────────────────────────────────────────────────────

    pub fn render_panel_header(
        title: &str,
        badge: Option<&str>,
        cx: &mut Context<InstallerView>,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            .px_4()
            .py_3()
            .justify_between()
            .items_center()
            .bg(cx.theme().sidebar)
            .border_b_1()
            .border_color(cx.theme().border)
            .flex_shrink_0()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child(title.to_string()),
                    )
                    .when_some(badge, |el, b| {
                        el.child(
                            div()
                                .px_2()
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(cx.theme().accent.opacity(0.15))
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(cx.theme().accent)
                                .child(b.to_string()),
                        )
                    }),
            )
    }

    pub fn render_loading_state(&self, message: &str, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(Spinner::new().color(cx.theme().accent))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(message.to_string()),
            )
    }

    pub fn feature_pill(label: &str, cx: &mut Context<InstallerView>) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .rounded_full()
            .bg(cx.theme().accent.opacity(0.1))
            .border_1()
            .border_color(cx.theme().accent.opacity(0.25))
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(cx.theme().accent)
            .child(label.to_string())
    }
}

// ─── Page: Welcome ────────────────────────────────────────────────────────────

impl InstallerView {
    fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(Self::render_panel_header("Welcome", None, cx))
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_8()
                    .p_8()
                    // Logo
                    .child(
                        div()
                            .w(px(80.0))
                            .h(px(80.0))
                            .rounded(px(20.0))
                            .bg(cx.theme().accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().accent_foreground)
                                    .child("P"),
                            ),
                    )
                    // Heading
                    .child(
                        v_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Pulsar Engine Installer"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .text_center()
                                    .child("Download and install Pulsar engine versions, or manage existing installations."),
                            ),
                    )
                    // Feature pills
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .justify_center()
                            .child(Self::feature_pill("Cross-platform", cx))
                            .child(Self::feature_pill("Multi-version", cx))
                            .child(Self::feature_pill("Auto-detect arch", cx)),
                    )
                    // Two action cards
                    .child(
                        h_flex()
                            .gap_6()
                            .child(
                                // Install New card
                                v_flex()
                                    .w(px(200.0))
                                    .p_5()
                                    .gap_3()
                                    .rounded(px(12.0))
                                    .border_1()
                                    .border_color(cx.theme().accent.opacity(0.35))
                                    .bg(cx.theme().accent.opacity(0.05))
                                    .cursor_pointer()
                                    .child(
                                        Icon::new(IconName::ArrowDown)
                                            .with_size(px(28.0))
                                            .text_color(cx.theme().accent),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("Install New"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Download and install a release from GitHub."),
                                    )
                                    .child(
                                        Button::new("install-new-btn")
                                            .primary()
                                            .label("Get Started →")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.navigate_to(Page::License, window, cx);
                                                this.fetch_license(cx);
                                            })),
                                    ),
                            )
                            .child(
                                // Manage Installed card
                                v_flex()
                                    .w(px(200.0))
                                    .p_5()
                                    .gap_3()
                                    .rounded(px(12.0))
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().sidebar.opacity(0.5))
                                    .cursor_pointer()
                                    .child(
                                        Icon::new(IconName::Inbox)
                                            .with_size(px(28.0))
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("Manage Installed"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("View, launch, or remove existing installations."),
                                    )
                                    .child(
                                        Button::new("manage-btn")
                                            .outline()
                                            .label("Open Manager →")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.navigate_to(Page::VersionsManager, window, cx);
                                                this.load_installed_versions(cx);
                                            })),
                                    ),
                            ),
                    ),
            )
    }
}

// ─── Page: License ────────────────────────────────────────────────────────────

impl InstallerView {
    fn render_license(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let accepted = self.license_accepted;

        v_flex()
            .size_full()
            .child(Self::render_panel_header("License Agreement", None, cx))
            // Content area
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .map(|el| {
                        if self.loading_license {
                            el.child(self.render_loading_state("Fetching license from GitHub…", cx))
                        } else if let Some(ref err) = self.license_fetch_error.clone() {
                            el.child(self.render_license_error(err.clone(), cx))
                        } else if let Some(ref text) = self.license_text.clone() {
                            el.child(Self::render_license_text(text.clone(), cx))
                        } else {
                            el.child(self.render_loading_state("Loading…", cx))
                        }
                    }),
            )
            // Bottom bar
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .items_center()
                    .child(
                        Button::new("license-back-btn")
                            .outline()
                            .label("← Back")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::Welcome, window, cx);
                            })),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Checkbox::new("license-accept-checkbox")
                                            .checked(accepted)
                                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                                this.license_accepted = *checked;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child("I accept the license agreement"),
                                    ),
                            )
                            .child(
                                Button::new("license-next-btn")
                                    .primary()
                                    .label("Next →")
                                    .disabled(!accepted)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.navigate_to(Page::VersionSelection, window, cx);
                                        this.fetch_releases(window, cx);
                                    })),
                            ),
                    ),
            )
    }

    fn render_license_text(text: String, cx: &mut Context<InstallerView>) -> impl IntoElement {
        v_flex()
            .id("license-scroll")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .child(
                div()
                    .text_xs()
                    .font_family("monospace")
                    .text_color(cx.theme().foreground)
                    .child(text),
            )
    }

    fn render_license_error(&self, error: String, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .p_8()
            .child(
                Icon::new(IconName::CircleX)
                    .with_size(px(40.0))
                    .text_color(cx.theme().danger),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .text_center()
                    .child(format!("Failed to fetch license: {error}")),
            )
            .child(
                Button::new("license-retry-btn")
                    .outline()
                    .label("Retry")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.fetch_license(cx);
                    })),
            )
    }
}

// ─── Page: Version Selection ──────────────────────────────────────────────────

impl InstallerView {
    fn render_version_selection(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_idx = self.selected_release_idx;
        let show_pre = self.show_prereleases;
        let has_selection = selected_idx.is_some();

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Select Version", None, cx))
            // Sub-header
            .child(
                h_flex()
                    .px_6()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar.opacity(0.4))
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::Info)
                                    .with_size(px(13.0))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Select a release to install. Architecture is detected automatically."),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Checkbox::new("show-pre-checkbox")
                                    .checked(show_pre)
                                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                        this.show_prereleases = *checked;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Show pre-releases"),
                            ),
                    ),
            )
            // Release list
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .map(|el| {
                        if self.loading_releases {
                            el.child(self.render_loading_state("Fetching releases from GitHub…", cx))
                        } else if self.releases.is_empty() {
                            el.child(self.render_empty_releases(cx))
                        } else {
                            el.child(self.render_release_list(cx))
                        }
                    }),
            )
            // Action bar
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .items_center()
                    .child(
                        Button::new("vs-back-btn")
                            .outline()
                            .label("← Back")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::License, window, cx);
                            })),
                    )
                    .child(
                        Button::new("vs-next-btn")
                            .primary()
                            .label("Next →")
                            .disabled(!has_selection)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::InstallOptions, window, cx);
                            })),
                    ),
            )
    }

    fn render_release_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_idx = self.selected_release_idx;
        let show_pre = self.show_prereleases;

        let visible: Vec<(usize, ReleaseInfo)> = self.releases
            .iter()
            .enumerate()
            .filter(|(_, r)| show_pre || !r.prerelease)
            .map(|(i, r)| (i, r.clone()))
            .collect();

        v_flex()
            .id("release-list-scroll")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .py_3()
            .gap_2()
            .children(visible.into_iter().map(|(idx, release)| {
                let selected = selected_idx == Some(idx);
                let is_pre = release.prerelease;

                h_flex()
                    .px_4()
                    .py_3()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(if selected {
                        cx.theme().accent.opacity(0.5)
                    } else {
                        cx.theme().border
                    })
                    .bg(if selected {
                        cx.theme().accent.opacity(0.06)
                    } else {
                        cx.theme().sidebar.opacity(0.3)
                    })
                    .gap_3()
                    .items_center()
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.select_release(idx, cx);
                    }))
                    // Radio dot
                    .child(
                        div()
                            .w(px(18.0))
                            .h(px(18.0))
                            .rounded_full()
                            .border_2()
                            .border_color(if selected {
                                cx.theme().accent
                            } else {
                                cx.theme().border
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(selected, |e| {
                                e.child(
                                    div()
                                        .w(px(8.0))
                                        .h(px(8.0))
                                        .rounded_full()
                                        .bg(cx.theme().accent),
                                )
                            }),
                    )
                    .child(
                        Icon::new(IconName::GalleryVerticalEnd)
                            .with_size(px(16.0))
                            .text_color(if selected {
                                cx.theme().accent
                            } else {
                                cx.theme().muted_foreground
                            }),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap(px(2.0))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child(release.name.clone()),
                                    )
                                    .when(is_pre, |e| {
                                        e.child(
                                            div()
                                                .px_2()
                                                .py(px(1.0))
                                                .rounded(px(4.0))
                                                .bg(cx.theme().warning.opacity(0.15))
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(cx.theme().warning)
                                                .child("pre-release"),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(release.tag_name.clone()),
                            ),
                    )
            }))
            .when(self.has_more_releases || self.loading_more, |el: gpui::Stateful<gpui::Div>| {
                el.child(
                    div()
                        .py_3()
                        .flex()
                        .justify_center()
                        .child(
                            Button::new("load-more-btn")
                                .outline()
                                .label(if self.loading_more { "Loading…" } else { "Load More" })
                                .disabled(self.loading_more)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.load_more_releases(window, cx);
                                })),
                        ),
                )
            })
    }

    fn render_empty_releases(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .p_8()
            .child(
                Icon::new(IconName::Github)
                    .with_size(px(48.0))
                    .text_color(cx.theme().muted_foreground.opacity(0.4)),
            )
            .child(
                v_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().muted_foreground)
                            .child("No releases found"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground.opacity(0.6))
                            .child("Could not fetch releases from GitHub"),
                    ),
            )
            .child(
                Button::new("vs-retry-btn")
                    .outline()
                    .label("Retry")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.fetch_releases(window, cx);
                    })),
            )
    }
}

// ─── Page: Install Options ────────────────────────────────────────────────────

impl InstallerView {
    fn render_install_options(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let path_str = self.install_config.install_path.display().to_string();
        let desktop_shortcut = self.install_config.create_desktop_shortcut;
        let start_menu = self.install_config.create_start_menu_shortcut;
        let add_to_path = self.install_config.add_to_path;

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Install Options", None, cx))
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .id("install-opts-scroll")
                    .overflow_y_scroll()
                    .p_6()
                    .gap_6()
                    // Install path
                    .child(
                        v_flex()
                            .gap_3()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Installation Path"),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex_1()
                                            .px_3()
                                            .py_2()
                                            .rounded(px(6.0))
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().sidebar)
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child(path_str),
                                    )
                                    .child(
                                        Button::new("browse-btn")
                                            .outline()
                                            .label("Browse…")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_native_picker(cx);
                                            })),
                                    ),
                            ),
                    )
                    // Options
                    .child(
                        v_flex()
                            .gap_3()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Options"),
                            )
                            .child(
                                v_flex()
                                    .gap_3()
                                    .child(
                                        h_flex()
                                            .gap_3()
                                            .items_center()
                                            .child(
                                                Checkbox::new("desktop-shortcut-cb")
                                                    .checked(desktop_shortcut)
                                                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                                        this.install_config.create_desktop_shortcut = *checked;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                v_flex()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .child("Create Desktop Shortcut"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child("Add a shortcut to your desktop."),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_3()
                                            .items_center()
                                            .child(
                                                Checkbox::new("start-menu-cb")
                                                    .checked(start_menu)
                                                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                                        this.install_config.create_start_menu_shortcut = *checked;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                v_flex()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .child(
                                                                #[cfg(target_os = "macos")]
                                                                "Add to Dock",
                                                                #[cfg(windows)]
                                                                "Create Start Menu Shortcut",
                                                                #[cfg(target_os = "linux")]
                                                                "Create Application Launcher Entry",
                                                                #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
                                                                "Create Shortcut",
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child("Integrate with the system launcher."),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_3()
                                            .items_center()
                                            .child(
                                                Checkbox::new("path-cb")
                                                    .checked(add_to_path)
                                                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                                        this.install_config.add_to_path = *checked;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                v_flex()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .child("Add to PATH"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child("Run `pulsar` from any terminal."),
                                                    ),
                                            ),
                                    ),
                            ),
                    ),
            )
            // Action bar
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .items_center()
                    .child(
                        Button::new("opts-back-btn")
                            .outline()
                            .label("← Back")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::VersionSelection, window, cx);
                            })),
                    )
                    .child(
                        Button::new("opts-install-btn")
                            .primary()
                            .label("Install →")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::Installing, window, cx);
                                this.start_installation(window, cx);
                            })),
                    ),
            )
    }
}

// ─── Page: Installing ─────────────────────────────────────────────────────────

impl InstallerView {
    fn render_installing(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_done = self.install_progress >= 100.0;

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Installing", None, cx))
            // Progress section
            .child(
                v_flex()
                    .px_6()
                    .py_5()
                    .gap_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar.opacity(0.3))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .when(!is_done, |el| el.child(Spinner::new().color(cx.theme().accent)))
                            .when(is_done, |el| {
                                el.child(
                                    Icon::new(IconName::CircleCheck)
                                        .with_size(px(20.0))
                                        .text_color(cx.theme().success),
                                )
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().foreground)
                                    .child(self.install_message.clone()),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Progress::new().value(self.install_progress))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Overall progress"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().accent)
                                            .child(format!("{:.0}%", self.install_progress)),
                                    ),
                            ),
                    ),
            )
            // Log panel
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .px_4()
                            .py_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::SquareTerminal)
                                    .with_size(px(13.0))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("OUTPUT"),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("log-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .px_4()
                            .py_3()
                            .gap(px(3.0))
                            .children(self.log_entries.iter().map(|entry| {
                                let (icon, color) = match entry.level {
                                    LogLevel::Info    => (IconName::Info, cx.theme().muted_foreground),
                                    LogLevel::Success => (IconName::CircleCheck, cx.theme().success),
                                    LogLevel::Warning => (IconName::TriangleAlert, cx.theme().warning),
                                    LogLevel::Error   => (IconName::CircleX, cx.theme().danger),
                                };
                                h_flex()
                                    .gap_2()
                                    .items_start()
                                    .child(
                                        Icon::new(icon)
                                            .with_size(px(12.0))
                                            .text_color(color),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(color)
                                            .child(entry.message.clone()),
                                    )
                            }))
                            .when(self.log_entries.is_empty(), |el: gpui::Stateful<gpui::Div>| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground.opacity(0.5))
                                        .child("Waiting for output…"),
                                )
                            }),
                    ),
            )
    }
}

// ─── Page: Complete ───────────────────────────────────────────────────────────

impl InstallerView {
    fn render_complete(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let success = !self.install_failed;

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Installation Complete", None, cx))
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_6()
                    .p_8()
                    // Result icon
                    .child(
                        div()
                            .w(px(72.0))
                            .h(px(72.0))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(if success {
                                cx.theme().success.opacity(0.12)
                            } else {
                                cx.theme().danger.opacity(0.12)
                            })
                            .border_2()
                            .border_color(if success {
                                cx.theme().success.opacity(0.4)
                            } else {
                                cx.theme().danger.opacity(0.4)
                            })
                            .child(
                                Icon::new(if success { IconName::CircleCheck } else { IconName::CircleX })
                                    .with_size(px(36.0))
                                    .text_color(if success {
                                        cx.theme().success
                                    } else {
                                        cx.theme().danger
                                    }),
                            ),
                    )
                    // Heading
                    .child(
                        v_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(if success {
                                        "Installation Complete!"
                                    } else {
                                        "Installation Finished with Errors"
                                    }),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .text_center()
                                    .child(if success {
                                        "Pulsar engine has been successfully installed on your system."
                                    } else {
                                        "Some steps failed. Review the output log for details."
                                    }),
                            ),
                    )
                    // Summary log
                    .child(
                        v_flex()
                            .w(px(400.0))
                            .max_h(px(160.0))
                            .rounded(px(8.0))
                            .bg(cx.theme().sidebar)
                            .border_1()
                            .border_color(cx.theme().border)
                            .overflow_hidden()
                            .child(
                                h_flex()
                                    .px_3()
                                    .py_2()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().muted_foreground)
                                            .child("SUMMARY"),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .id("log-entries-scroll")
                                    .overflow_y_scroll()
                                    .px_3()
                                    .py_2()
                                    .gap(px(2.0))
                                    .children(
                                        self.log_entries
                                            .iter()
                                            .filter(|e| matches!(e.level, LogLevel::Success | LogLevel::Error | LogLevel::Warning))
                                            .map(|entry| {
                                                let color = match entry.level {
                                                    LogLevel::Success => cx.theme().success,
                                                    LogLevel::Error   => cx.theme().danger,
                                                    LogLevel::Warning => cx.theme().warning,
                                                    LogLevel::Info    => cx.theme().muted_foreground,
                                                };
                                                div()
                                                    .text_xs()
                                                    .text_color(color)
                                                    .child(entry.message.clone())
                                            }),
                                    ),
                            ),
                    ),
            )
            // Action bar
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_end()
                    .gap_3()
                    .when(success && self.installed_path.is_some(), |el| {
                        el
                            .child(
                                Button::new("open-folder-btn")
                                    .outline()
                                    .label("Open Folder")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_install_folder(cx);
                                    })),
                            )
                            .child(
                                Button::new("launch-btn")
                                    .outline()
                                    .label("Launch Pulsar")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.launch_pulsar(cx);
                                    })),
                            )
                    })
                    .child(
                        Button::new("finish-btn")
                            .primary()
                            .label("Finish")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.quit();
                            })),
                    ),
            )
    }
}

// ─── Page: Versions Manager ───────────────────────────────────────────────────

impl InstallerView {
    fn render_versions_manager(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let confirm_idx = self.uninstall_confirm;

        v_flex()
            .size_full()
            .child(
                // Header with refresh
                h_flex()
                    .w_full()
                    .px_4()
                    .py_3()
                    .justify_between()
                    .items_center()
                    .bg(cx.theme().sidebar)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex_shrink_0()
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Installed Versions"),
                            )
                            .when(!self.installed_versions.is_empty(), |el| {
                                el.child(
                                    div()
                                        .px_2()
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(cx.theme().accent.opacity(0.15))
                                        .text_xs()
                                        .text_color(cx.theme().accent)
                                        .child(format!("{}", self.installed_versions.len())),
                                )
                            }),
                    )
                    .child(
                        Button::new("vm-refresh-btn")
                            .ghost()
                            .label("Refresh")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load_installed_versions(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .relative()
                    .map(|el| {
                        if self.loading_installed {
                            el.child(self.render_loading_state("Scanning for installed versions…", cx))
                        } else if self.installed_versions.is_empty() {
                            el.child(self.render_vm_empty(cx))
                        } else {
                            el.child(self.render_vm_list(cx))
                        }
                    })
                    // Confirm overlay
                    .when_some(confirm_idx, |el, idx| {
                        el.child(self.render_uninstall_confirm(idx, cx))
                    }),
            )
    }

    fn render_vm_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .p_8()
            .child(
                Icon::new(IconName::Inbox)
                    .with_size(px(48.0))
                    .text_color(cx.theme().muted_foreground.opacity(0.4)),
            )
            .child(
                v_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().muted_foreground)
                            .child("No installations found"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground.opacity(0.6))
                            .child("Install Pulsar to manage versions here."),
                    ),
            )
            .child(
                Button::new("vm-install-cta")
                    .primary()
                    .label("Install Now →")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_to(Page::Welcome, window, cx);
                    })),
            )
    }

    fn render_vm_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let versions: Vec<InstalledVersion> = self.installed_versions.clone();

        v_flex()
            .id("vm-list-scroll")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .py_3()
            .gap_3()
            .children(versions.into_iter().enumerate().map(|(idx, ver)| {
                let path = ver.metadata.install_path.clone();
                let path_for_open = path.clone();
                let path_for_launch = path.clone();
                let size_str = Self::format_bytes(ver.disk_size_bytes);
                let version_str = ver.metadata.version.clone();
                let date_str = if ver.metadata.install_date.is_empty() {
                    "Date unknown".to_string()
                } else {
                    ver.metadata.install_date.chars().take(10).collect()
                };

                h_flex()
                    .px_4()
                    .py_3()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar.opacity(0.3))
                    .gap_3()
                    .items_center()
                    // Version badge
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded(px(6.0))
                            .bg(cx.theme().accent.opacity(0.12))
                            .border_1()
                            .border_color(cx.theme().accent.opacity(0.3))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().accent)
                            .child(version_str),
                    )
                    // Info
                    .child(
                        v_flex()
                            .flex_1()
                            .gap(px(2.0))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(path.display().to_string()),
                                    )
                                    .when(ver.update_available, |e| {
                                        e.child(
                                            div()
                                                .px_2()
                                                .py(px(1.0))
                                                .rounded(px(4.0))
                                                .bg(cx.theme().warning.opacity(0.15))
                                                .text_xs()
                                                .text_color(cx.theme().warning)
                                                .child("Update Available"),
                                        )
                                    }),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground.opacity(0.6))
                                            .child(size_str),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground.opacity(0.6))
                                            .child("·"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground.opacity(0.6))
                                            .child(date_str),
                                    ),
                            ),
                    )
                    // Actions
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new(format!("vm-launch-{idx}"))
                                    .ghost()
                                    .label("Launch")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.launch_version_path(path_for_launch.clone(), cx);
                                    })),
                            )
                            .child(
                                Button::new(format!("vm-open-{idx}"))
                                    .ghost()
                                    .label("Open Folder")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_folder_path(path_for_open.clone(), cx);
                                    })),
                            )
                            .child(
                                Button::new(format!("vm-uninstall-{idx}"))
                                    .danger()
                                    .label("Uninstall")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.uninstall_confirm = Some(idx);
                                        cx.notify();
                                    })),
                            ),
                    )
            }))
    }

    fn render_uninstall_confirm(&self, idx: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let version = self
            .installed_versions
            .get(idx)
            .map(|v| v.metadata.version.clone())
            .unwrap_or_else(|| "this version".to_string());

        // Semi-transparent backdrop + centered modal
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.5))
            .child(
                v_flex()
                    .w(px(360.0))
                    .p_6()
                    .gap_4()
                    .rounded(px(12.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Confirm Uninstall"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "Are you sure you want to remove {}? This will permanently delete all files.",
                                        version
                                    )),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .justify_end()
                            .child(
                                Button::new("confirm-cancel-btn")
                                    .outline()
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.uninstall_confirm = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("confirm-uninstall-btn")
                                    .danger()
                                    .label("Uninstall")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.uninstall_version(idx, cx);
                                    })),
                            ),
                    ),
            )
    }
}
