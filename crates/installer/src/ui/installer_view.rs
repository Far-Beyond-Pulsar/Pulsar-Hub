//! Main installer view — styled after the Pulsar-Native level editor / agent panel.

use gpui::{
    App, AppContext as _, Context, Entity, Focusable, FontWeight, IntoElement, ParentElement,
    InteractiveElement as _, Render, StatefulInteractiveElement as _, Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme,
    Disableable as _,
    Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, v_flex,
    Icon, IconName,
    progress::Progress,
    spinner::Spinner,
};
use crate::download::{GitHubReleases, HttpDownloadManager, GitHubRelease, GitHubAsset};
use crate::traits::DownloadManager as _;
use std::path::PathBuf;
use gpui::prelude::FluentBuilder;

// ─── Step definitions ────────────────────────────────────────────────────────

/// Installer wizard steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Welcome,
    VersionSelection,
    Installing,
    Complete,
}

impl Page {
    fn index(self) -> usize {
        match self {
            Page::Welcome => 0,
            Page::VersionSelection => 1,
            Page::Installing => 2,
            Page::Complete => 3,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Page::Welcome => "Welcome",
            Page::VersionSelection => "Select Version",
            Page::Installing => "Installing",
            Page::Complete => "Complete",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Page::Welcome => IconName::Bot,
            Page::VersionSelection => IconName::Github,
            Page::Installing => IconName::HardDrive,
            Page::Complete => IconName::CircleCheck,
        }
    }
}

const STEPS: [Page; 4] = [
    Page::Welcome,
    Page::VersionSelection,
    Page::Installing,
    Page::Complete,
];

// ─── Data types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub prerelease: bool,
    pub selected: bool,
}

// ─── Log entry ────────────────────────────────────────────────────────────────

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

// ─── View ─────────────────────────────────────────────────────────────────────

pub struct InstallerView {
    focus_handle: gpui::FocusHandle,
    current_page: Page,

    // Release loading
    releases: Vec<ReleaseInfo>,
    loading_releases: bool,
    loading_more: bool,
    current_releases_page: u32,
    has_more_releases: bool,

    // Installation
    install_progress: f32,
    install_message: String,
    log_entries: Vec<LogEntry>,
    install_failed: bool,
}

impl InstallerView {
    pub fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(cx))
    }

    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            current_page: Page::Welcome,
            releases: Vec::new(),
            loading_releases: false,
            loading_more: false,
            current_releases_page: 0,
            has_more_releases: true,
            install_progress: 0.0,
            install_message: String::new(),
            log_entries: Vec::new(),
            install_failed: false,
        }
    }

    fn navigate_to(&mut self, page: Page, _window: &mut Window, cx: &mut Context<Self>) {
        self.current_page = page;
        cx.notify();
    }

    fn log(&mut self, level: LogLevel, message: impl Into<String>, cx: &mut Context<Self>) {
        self.log_entries.push(LogEntry { level, message: message.into() });
        cx.notify();
    }

    // ── Release fetching ──────────────────────────────────────────────────

    fn fetch_releases(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.loading_releases = true;
        self.current_releases_page = 1;
        self.releases.clear();
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
                            selected: false,
                        })
                        .collect();
                    this.update(cx, |this, cx| {
                        this.releases = infos;
                        this.loading_releases = false;
                        this.has_more_releases = has_more;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("Failed to fetch releases: {}", e);
                    this.update(cx, |this, cx| {
                        this.loading_releases = false;
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn load_more_releases(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.loading_more || !self.has_more_releases {
            return;
        }
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
                            selected: false,
                        })
                        .collect();
                    this.update(cx, |this, cx| {
                        this.releases.extend(infos);
                        this.loading_more = false;
                        this.has_more_releases = has_more;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("Failed to fetch more releases: {}", e);
                    this.update(cx, |this, cx| {
                        this.loading_more = false;
                        this.current_releases_page -= 1;
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn toggle_release(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(release) = self.releases.get_mut(index) {
            release.selected = !release.selected;
            cx.notify();
        }
    }

    fn selected_count(&self) -> usize {
        self.releases.iter().filter(|r| r.selected).count()
    }

    // ── Asset selection ───────────────────────────────────────────────────

    /// Pick the single best asset from a release for the current OS + arch.
    ///
    /// Naming convention used in Pulsar releases:
    ///   `<name>-<os>-<arch>[.ext][.sig]`
    ///   e.g. `pulsar_engine-macos-arm64`, `pulsar-host-windows-x86_64.exe`
    ///
    /// Rules (in order):
    ///  1. Skip `.sig` signature files — they're not installable.
    ///  2. Must contain the current OS token (`linux`, `macos`, `windows`).
    ///  3. Must contain the current arch token (`x86_64` or `arm64`).
    ///     Note: Rust's `ARCH` returns `"aarch64"` but release files use `arm64`.
    ///  4. On macOS, prefer `.app.zip` bundles over raw binaries when both exist.
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

        // Rust calls it "aarch64"; the release files call it "arm64".
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
                !n.ends_with(".sig")          // skip signatures
                    && n.contains(os_token)   // must match OS
                    && n.contains(arch_token) // must match arch
            })
            .collect();

        tracing::info!(
            "Asset selection: os={os_token} arch={arch_token} → {} candidate(s): {:?}",
            candidates.len(),
            candidates.iter().map(|a| &a.name).collect::<Vec<_>>()
        );

        if candidates.is_empty() {
            return None;
        }

        // On macOS prefer the .app.zip bundle (proper macOS app) when present.
        #[cfg(target_os = "macos")]
        if let Some(app_zip) = candidates.iter().find(|a| a.name.ends_with(".app.zip")) {
            tracing::info!("Preferring .app.zip: {}", app_zip.name);
            return Some((*app_zip).clone());
        }

        // Default: first match (deterministic given sorted GitHub asset list).
        candidates.into_iter().next().cloned()
    }

    // ── Installation ──────────────────────────────────────────────────────

    fn start_installation(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.install_progress = 0.0;
        self.install_message = "Preparing installation…".to_string();
        self.log_entries.clear();
        self.install_failed = false;
        cx.notify();

        let selected_releases: Vec<GitHubRelease> = self
            .releases
            .iter()
            .filter(|r| r.selected)
            .map(|r| GitHubRelease {
                tag_name: r.tag_name.clone(),
                name: r.name.clone(),
                body: String::new(),
                assets: Vec::new(),
                prerelease: r.prerelease,
            })
            .collect();

        if selected_releases.is_empty() {
            self.install_message = "No versions selected.".to_string();
            cx.notify();
            return;
        }

        cx.spawn(async move |this, cx| {
            let download_manager = HttpDownloadManager::new();
            let github = GitHubReleases::new("Far-Beyond-Pulsar", "Pulsar-Native");

            let download_dir = std::env::temp_dir().join("pulsar-installer");
            if let Err(e) = std::fs::create_dir_all(&download_dir) {
                this.update(cx, |this, cx| {
                    this.install_message = format!("Failed to create temp dir: {e}");
                    this.install_failed = true;
                    this.log(LogLevel::Error, format!("Temp dir error: {e}"), cx);
                })
                .ok();
                return;
            }

            // Resolve full release assets using the deterministic selector.
            let mut releases_with_assets: Vec<(GitHubRelease, GitHubAsset)> = Vec::new();
            let mut total_size = 0u64;

            for sel in &selected_releases {
                this.update(cx, |this, cx| {
                    this.install_message = format!("Resolving assets for {}…", sel.name);
                    this.log(LogLevel::Info, format!("Resolving {}", sel.tag_name), cx);
                })
                .ok();

                match github.get_all_releases().await {
                    Ok(releases) => {
                        if let Some(full_release) = releases.into_iter().find(|r| r.tag_name == sel.tag_name) {
                            match Self::select_asset(&full_release.assets) {
                                Some(a) => {
                                    tracing::info!("Selected asset '{}' ({}) for {}", a.name, Self::format_bytes(a.size), sel.tag_name);
                                    total_size += a.size;
                                    releases_with_assets.push((full_release, a));
                                }
                                None => {
                                    let os = std::env::consts::OS;
                                    let arch = std::env::consts::ARCH;
                                    this.update(cx, |this, cx| {
                                        this.log(
                                            LogLevel::Warning,
                                            format!("No asset found for {tag} on {os}/{arch}", tag = sel.tag_name),
                                            cx,
                                        );
                                    }).ok();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        this.update(cx, |this, cx| {
                            this.install_message = format!("Failed to resolve release: {e}");
                            this.install_failed = true;
                            this.log(LogLevel::Error, format!("Resolve error: {e}"), cx);
                        }).ok();
                        return;
                    }
                }
            }

            let mut downloaded_bytes = 0u64;

            for (idx, (release, asset)) in releases_with_assets.iter().enumerate() {
                let release_num = idx + 1;
                let total_count = releases_with_assets.len();
                let release_name = release.name.clone();
                let asset_name = asset.name.clone();

                this.update(cx, |this, cx| {
                    this.install_message = format!("Downloading {release_num}/{total_count}: {release_name}");
                    this.log(LogLevel::Info, format!("Downloading {} ({})", asset_name, Self::format_bytes(asset.size)), cx);
                }).ok();

                let file_path = download_dir.join(&asset.name);
                let file_path_for_install = file_path.clone();
                let url = asset.browser_download_url.clone();
                let base_downloaded = downloaded_bytes;

                let progress_state = std::sync::Arc::new(std::sync::Mutex::new((0u64, 0.0f32)));
                let progress_state_clone = progress_state.clone();

                let download_task = {
                    let download_manager = download_manager.clone();
                    smol::spawn(async move {
                        download_manager
                            .download(&url, &file_path, Box::new(move |prog| {
                                *progress_state_clone.lock().unwrap() = (prog.processed_bytes, prog.current);
                            }))
                            .await
                    })
                };

                let mut last_update = std::time::Instant::now();
                loop {
                    if download_task.is_finished() { break; }
                    if last_update.elapsed() >= std::time::Duration::from_millis(100) {
                        let (processed, file_pct) = *progress_state.lock().unwrap();
                        let current_bytes = base_downloaded + processed;
                        let overall_pct = if total_size > 0 {
                            (current_bytes as f32 / total_size as f32) * 100.0
                        } else { file_pct };

                        let msg = format!("Downloading {} — {:.1}%", asset_name, file_pct);
                        this.update(cx, |this, cx| {
                            this.install_progress = overall_pct;
                            this.install_message = msg;
                            cx.notify();
                        }).ok();

                        last_update = std::time::Instant::now();
                    }
                    smol::Timer::after(std::time::Duration::from_millis(50)).await;
                }

                match download_task.await {
                    Ok(_) => {
                        downloaded_bytes += asset.size;
                        this.update(cx, |this, cx| {
                            this.log(LogLevel::Success, format!("Downloaded {}", asset_name), cx);
                            this.install_message = format!("Installing {}…", release_name);
                        }).ok();

                        let install_result = Self::install_release(&file_path_for_install, &release.tag_name).await;
                        match install_result {
                            Ok(install_path) => {
                                this.update(cx, |this, cx| {
                                    this.log(LogLevel::Success, format!("Installed → {}", install_path.display()), cx);
                                }).ok();
                            }
                            Err(e) => {
                                this.update(cx, |this, cx| {
                                    this.log(LogLevel::Error, format!("Install failed: {e}"), cx);
                                    this.install_failed = true;
                                }).ok();
                            }
                        }
                    }
                    Err(e) => {
                        this.update(cx, |this, cx| {
                            this.log(LogLevel::Error, format!("Download failed: {e}"), cx);
                            this.install_failed = true;
                        }).ok();
                        continue;
                    }
                }
            }

            this.update(cx, |this, cx| {
                this.install_progress = 100.0;
                if this.install_failed {
                    this.install_message = "Installation completed with errors.".to_string();
                    this.log(LogLevel::Warning, "Finished with errors — check the log above.", cx);
                } else {
                    this.install_message = "Installation complete!".to_string();
                    this.log(LogLevel::Success, "All versions installed successfully.", cx);
                }
                this.current_page = Page::Complete;
                cx.notify();
            }).ok();
        })
        .detach();
    }

    fn format_bytes(bytes: u64) -> String {
        const MB: u64 = 1024 * 1024;
        const KB: u64 = 1024;
        if bytes >= MB { format!("{:.1} MB", bytes as f64 / MB as f64) }
        else if bytes >= KB { format!("{:.1} KB", bytes as f64 / KB as f64) }
        else { format!("{bytes} B") }
    }

    async fn install_release(archive_path: &PathBuf, version: &str) -> crate::error::Result<PathBuf> {
        use std::fs;

        #[cfg(windows)]
        let install_dir = PathBuf::from(
            std::env::var("LOCALAPPDATA")
                .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string()),
        )
        .join("Programs")
        .join("Pulsar");

        #[cfg(target_os = "macos")]
        let install_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/Users/Default"))
            .join("Applications")
            .join("Pulsar.app");

        #[cfg(target_os = "linux")]
        let install_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/home/default"))
            .join(".local")
            .join("share")
            .join("pulsar")
            .join(version);

        fs::create_dir_all(&install_dir).map_err(crate::error::InstallerError::Io)?;

        let archive_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Determine file type by examining the full filename (extension can be compound,
        // e.g. ".app.zip" or ".tar.gz").
        if archive_name.ends_with(".app.zip") || archive_name.ends_with(".zip") {
            // ZIP archive (includes macOS .app.zip bundles)
            let file = fs::File::open(archive_path).map_err(crate::error::InstallerError::Io)?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| crate::error::InstallerError::Other(e.to_string()))?;
            for i in 0..archive.len() {
                let mut zf = archive
                    .by_index(i)
                    .map_err(|e| crate::error::InstallerError::Other(e.to_string()))?;
                let out = install_dir.join(zf.mangled_name());
                if zf.name().ends_with('/') {
                    fs::create_dir_all(&out).map_err(crate::error::InstallerError::Io)?;
                } else {
                    if let Some(p) = out.parent() {
                        fs::create_dir_all(p).map_err(crate::error::InstallerError::Io)?;
                    }
                    let mut outfile = fs::File::create(&out).map_err(crate::error::InstallerError::Io)?;
                    std::io::copy(&mut zf, &mut outfile).map_err(crate::error::InstallerError::Io)?;

                    // Restore executable bit for files inside a zip on Unix.
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
            tracing::info!("Extracted zip to: {}", install_dir.display());
        } else if archive_name.ends_with(".tar.gz") || archive_name.ends_with(".tgz") {
            // Gzipped tarball
            let file = fs::File::open(archive_path).map_err(crate::error::InstallerError::Io)?;
            let tar = flate2::read::GzDecoder::new(file);
            let mut archive = tar::Archive::new(tar);
            archive.unpack(&install_dir).map_err(crate::error::InstallerError::Io)?;
            tracing::info!("Extracted tar.gz to: {}", install_dir.display());
        } else if archive_name.ends_with(".exe") {
            // Windows executable — copy directly.
            let dest = install_dir.join("pulsar.exe");
            fs::copy(archive_path, &dest).map_err(crate::error::InstallerError::Io)?;
            tracing::info!("Copied .exe to: {}", dest.display());
        } else {
            // Raw binary (no extension) — typical for Linux/macOS Pulsar releases.
            // Derive a stable name: strip any platform/arch suffix and use "pulsar".
            let dest = install_dir.join("pulsar");
            fs::copy(archive_path, &dest).map_err(crate::error::InstallerError::Io)?;
            tracing::info!("Copied binary to: {}", dest.display());

            // Make executable on Unix platforms.
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
            use crate::traits::{Progress, ProgressCallback};
            let installer = WindowsInstaller::new(install_dir.clone(), version.to_string());
            let progress: ProgressCallback = Box::new(|p: Progress| {
                tracing::info!("[{}%] {}", p.current, p.message.unwrap_or(""));
            });
            installer.install(progress).await?;
        }

        #[cfg(target_os = "macos")]
        {
            use crate::platform::MacOSInstaller;
            use crate::traits::{Progress, ProgressCallback};
            let binary_name = "pulsar".to_string();
            let source_binary = install_dir.join("Contents").join("MacOS").join(&binary_name);
            let installer = MacOSInstaller::new(install_dir.clone(), version.to_string(), binary_name);
            let progress: ProgressCallback = Box::new(|p: Progress| {
                tracing::info!("[{}%] {}", p.current, p.message.unwrap_or(""));
            });
            installer.install(&source_binary, progress).await?;
        }

        #[cfg(target_os = "linux")]
        {
            use crate::platform::LinuxInstaller;
            use crate::traits::{Progress, ProgressCallback};
            let installer = LinuxInstaller::new(version.to_string(), false);
            let source_binary = install_dir.join("pulsar");
            let progress: ProgressCallback = Box::new(|p: Progress| {
                tracing::info!("[{}%] {}", p.current, p.message.unwrap_or(""));
            });
            installer.install(&source_binary, progress).await?;
        }

        Ok(install_dir)
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
        // Root shell: full window, no padding, dark background
        div()
            .size_full()
            .bg(cx.theme().background)
            .flex()
            .flex_col()
            .child(self.render_title_bar(cx))
            .child(
                // Body: sidebar + content side-by-side
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
    // Title bar — matches Pulsar-Native transparent title bar style
    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .h(px(36.0))
            .bg(cx.theme().sidebar)
            .border_b_1()
            .border_color(cx.theme().border)
            // Left-pad enough for macOS traffic lights
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
            // Right side: current step label
            .child(
                div()
                    .pr_4()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.current_page.label()),
            )
    }

    // Sidebar — dark panel with step navigation (like level editor panels)
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_idx = self.current_page.index();

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
                                // Pulsar "P" monogram
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
            // ── Step navigation ──
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
                            .child("INSTALLATION STEPS"),
                    )
                    .children(STEPS.iter().enumerate().map(|(idx, &step)| {
                        let is_active = idx == current_idx;
                        let is_done = idx < current_idx;

                        h_flex()
                            .gap_3()
                            .px_2()
                            .py(px(6.0))
                            .rounded(px(6.0))
                            .items_center()
                            .when(is_active, |el| el.bg(cx.theme().accent.opacity(0.12)))
                            // Step number circle
                            .child(
                                div()
                                    .w(px(22.0))
                                    .h(px(22.0))
                                    .rounded_full()
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .justify_center()
                                    .when(is_done, |el| {
                                        el.bg(cx.theme().accent.opacity(0.2))
                                          .child(
                                              Icon::new(IconName::Check)
                                                  .with_size(px(12.0))
                                                  .text_color(cx.theme().accent),
                                          )
                                    })
                                    .when(is_active, |el| {
                                        el.bg(cx.theme().accent)
                                          .child(
                                              div()
                                                  .text_xs()
                                                  .font_weight(FontWeight::BOLD)
                                                  .text_color(cx.theme().accent_foreground)
                                                  .child(format!("{}", idx + 1)),
                                          )
                                    })
                                    .when(!is_active && !is_done, |el| {
                                        el.border_1()
                                          .border_color(cx.theme().border)
                                          .child(
                                              div()
                                                  .text_xs()
                                                  .text_color(cx.theme().muted_foreground)
                                                  .child(format!("{}", idx + 1)),
                                          )
                                    }),
                            )
                            // Step label + icon
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
                                            .font_weight(if is_active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
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
                    })),
            )
            // ── Footer ──
            .child(
                v_flex()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .gap_1()
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

    // Main content router
    fn render_main_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .map(|el| match self.current_page {
                Page::Welcome => el.child(self.render_welcome(cx)),
                Page::VersionSelection => el.child(self.render_version_selection(cx)),
                Page::Installing => el.child(self.render_installing(cx)),
                Page::Complete => el.child(self.render_complete(cx)),
            })
    }
}

// ─── Page renderers ───────────────────────────────────────────────────────────

impl InstallerView {
    // ── Welcome ──────────────────────────────────────────────────────────

    fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(Self::render_panel_header("Welcome", None, cx))
            // Content
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_6()
                    .p_8()
                    // Big logo
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
                                    .child("Download and install Pulsar engine versions from GitHub releases."),
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
                    ),
            )
            // Bottom action bar
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_end()
                    .child(
                        Button::new("start-btn")
                            .primary()
                            .label("Get Started →")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::VersionSelection, window, cx);
                                this.fetch_releases(window, cx);
                            })),
                    ),
            )
    }

    fn feature_pill(label: &str, cx: &mut Context<InstallerView>) -> impl IntoElement {
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

    // ── Version Selection ─────────────────────────────────────────────────

    fn render_version_selection(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_count = self.selected_count();
        let count_badge = if selected_count > 0 {
            Some(format!("{selected_count} selected"))
        } else {
            None
        };

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Select Versions", count_badge.as_deref(), cx))
            // Sub-header description
            .child(
                h_flex()
                    .px_6()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar.opacity(0.4))
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Info)
                            .with_size(px(13.0))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Select one or more releases to install. Architecture is detected automatically."),
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
                        Button::new("back-btn")
                            .outline()
                            .label("← Back")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::Welcome, window, cx);
                            })),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .when(selected_count > 0, |el| {
                                el.child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{selected_count} version(s) selected")),
                                )
                            })
                            .child(
                                Button::new("install-btn")
                                    .primary()
                                    .label("Install Selected →")
                                    .disabled(selected_count == 0)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.navigate_to(Page::Installing, window, cx);
                                        this.start_installation(window, cx);
                                    })),
                            ),
                    ),
            )
    }

    fn render_release_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("release-list-scroll")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .py_3()
            .gap_2()
            .children(
                self.releases
                    .iter()
                    .enumerate()
                    .map(|(idx, release)| {
                        let selected = release.selected;
                        let is_prerelease = release.prerelease;
                        let release_name = release.name.clone();
                        let tag_name = release.tag_name.clone();

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
                            // Checkbox
                            .child(
                                Checkbox::new(format!("release-{idx}"))
                                    .checked(selected)
                                    .on_click(cx.listener(move |this, _checked: &bool, window, cx| {
                                        this.toggle_release(idx, window, cx);
                                    })),
                            )
                            // Icon
                            .child(
                                Icon::new(IconName::GalleryVerticalEnd)
                                    .with_size(px(16.0))
                                    .text_color(if selected {
                                        cx.theme().accent
                                    } else {
                                        cx.theme().muted_foreground
                                    }),
                            )
                            // Labels
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
                                                    .child(release_name),
                                            )
                                            .when(is_prerelease, |el| {
                                                el.child(
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
                                            .child(tag_name),
                                    ),
                            )
                            // Selected indicator
                            .when(selected, |el| {
                                el.child(
                                    Icon::new(IconName::Check)
                                        .with_size(px(14.0))
                                        .text_color(cx.theme().accent),
                                )
                            })
                    }),
            )
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

    fn render_loading_state(&self, message: &str, cx: &mut Context<Self>) -> impl IntoElement {
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
                Button::new("retry-btn")
                    .outline()
                    .label("Retry")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.fetch_releases(window, cx);
                    })),
            )
    }

    // ── Installing ────────────────────────────────────────────────────────

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
                    // Status row
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
                    // Progress bar + percentage
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
                                    LogLevel::Info => (IconName::Info, cx.theme().muted_foreground),
                                    LogLevel::Success => (IconName::CircleCheck, cx.theme().success),
                                    LogLevel::Warning => (IconName::TriangleAlert, cx.theme().warning),
                                    LogLevel::Error => (IconName::CircleX, cx.theme().danger),
                                };
                                h_flex()
                                    .gap_2()
                                    .items_start()
                                    .child(
                                        Icon::new(icon)
                                            .with_size(px(12.0))
                                            .text_color(color)
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

    // ── Complete ──────────────────────────────────────────────────────────

    fn render_complete(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let success = !self.install_failed;

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Installation Complete", None, cx))
            // Central result
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
                    // Heading + sub
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
                    // Summary log (compact)
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
                                                    LogLevel::Error => cx.theme().danger,
                                                    LogLevel::Warning => cx.theme().warning,
                                                    LogLevel::Info => cx.theme().muted_foreground,
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

    // ── Shared panel header (matches Pulsar-Native properties_inspector style) ──

    fn render_panel_header(
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
                    .when_some(badge, |el, badge_text| {
                        el.child(
                            div()
                                .px_2()
                                .py(px(2.0))
                                .rounded(px(4.0))
                                .bg(cx.theme().accent.opacity(0.15))
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(cx.theme().accent)
                                .child(badge_text.to_string()),
                        )
                    }),
            )
    }
}
