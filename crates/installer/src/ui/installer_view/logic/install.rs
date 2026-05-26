//! Core installation pipeline: asset download, extraction, platform setup, sidecars.

use gpui::{Context, Window};
use std::path::PathBuf;
use crate::download::{GitHubReleases, HttpDownloadManager};
use crate::traits::DownloadManager as _;
use crate::installed_versions::write_metadata;
use super::super::{InstallerView, LogLevel, Page};

const GITHUB_ORG:  &str = "Far-Beyond-Pulsar";
const GITHUB_REPO: &str = "Pulsar-Native";

impl InstallerView {
    // ─── Entry point ──────────────────────────────────────────────────────────

    /// Validate state, compute paths, then hand off to the async pipeline.
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

        // Reset install state.
        self.install_progress = 0.0;
        self.install_message = "Preparing installation…".to_string();
        self.log_entries.clear();
        self.install_failed = false;
        self.installed_path = None;

        let prefer_app_bundle = self.macos_use_app_bundle;
        let versions_root = InstallerView::normalize_versions_root(self.install_config.install_path.clone());
        if !InstallerView::path_in_legal_area(&versions_root) {
            let expected = InstallerView::default_versions_root();
            self.install_failed = true;
            self.install_message = "Install blocked: selected path escapes sandbox.".to_string();
            self.log(
                LogLevel::Warning,
                format!(
                    "Install blocked: '{}' escapes sandbox. Expected install root '{}'.",
                    versions_root.display(),
                    expected.display()
                ),
                cx,
            );
            tracing::warn!(
                "Install blocked: '{}' escapes sandbox. Expected '{}'.",
                versions_root.display(),
                expected.display()
            );
            cx.notify();
            return;
        }

        // Sanitise version string into a safe directory name.
        let version_dir = release_info
            .tag_name
            .trim()
            .trim_start_matches('v')
            .replace('/', "_")
            .replace(':', "_")
            .replace(' ', "_");

        // Compute per-version install layout from canonical versions root.
        let (install_dir, version_root) =
            InstallerView::compute_install_layout(&versions_root, &version_dir, prefer_app_bundle);

        let selected_sidecars = self.selected_sidecars.clone();
        let sidecar_specs: Vec<(String, PathBuf)> = selected_sidecars
            .iter()
            .map(|id| (id.clone(), version_root.join(id.as_str())))
            .collect();

        cx.notify();

        cx.spawn(async move |this, cx| {
            run_installation(this, cx, release_info, install_dir, sidecar_specs, prefer_app_bundle).await;
        })
        .detach();
    }

    // ─── Archive extraction + platform post-install ────────────────────────────

    /// Extract/install an archive into `install_dir` then run platform-specific
    /// post-installation steps (shortcuts, PATH, .app bundle creation, etc.).
    pub(crate) async fn install_release(
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
            extract_zip(archive_path, install_dir, is_app_zip)?;
        } else if archive_name.ends_with(".tar.gz") || archive_name.ends_with(".tgz") {
            extract_tar_gz(archive_path, install_dir)?;
        } else if archive_name.ends_with(".exe") {
            let dest = install_dir.join("pulsar.exe");
            fs::copy(archive_path, &dest).map_err(crate::error::InstallerError::Io)?;
        } else {
            install_raw_binary(archive_path, install_dir)?;
        }

        platform_post_install(archive_path, install_dir, version, is_app_zip).await?;

        Ok(())
    }

    /// Resolve the final installed path from the expected dir + archive name
    /// (needed because macOS .app.zip extracts under a different name).
    pub(crate) fn resolve_installed_path(
        expected_install_dir: &PathBuf,
        archive_path: &PathBuf,
    ) -> PathBuf {
        let archive_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        #[cfg(target_os = "macos")]
        if archive_name.ends_with(".app.zip") {
            if expected_install_dir.exists() {
                return expected_install_dir.clone();
            }
            // Walk the parent directory and return the newest .app bundle.
            if let Some(parent) = expected_install_dir.parent() {
                if let Ok(entries) = std::fs::read_dir(parent) {
                    let mut apps: Vec<(std::time::SystemTime, PathBuf)> = entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| p.extension() == Some(std::ffi::OsStr::new("app")))
                        .filter_map(|p| {
                            let mtime = std::fs::metadata(&p)
                                .and_then(|m| m.modified())
                                .ok()?;
                            Some((mtime, p))
                        })
                        .collect();
                    apps.sort_by(|a, b| b.0.cmp(&a.0));
                    if let Some((_, p)) = apps.into_iter().next() {
                        return p;
                    }
                }
            }
        }

        expected_install_dir.clone()
    }

    // ─── Sidecar binaries ─────────────────────────────────────────────────────

    /// Copy a plain binary (no bundle wrapping) into `install_dir/{binary_name}[.exe]`.
    pub(crate) async fn install_sidecar_binary(
        archive_path: &PathBuf,
        install_dir: &PathBuf,
        binary_name: &str,
    ) -> crate::error::Result<()> {
        let archive_path = archive_path.clone();
        let install_dir = install_dir.clone();
        let binary_name = binary_name.to_string();

        smol::unblock(move || {
            use std::fs;
            fs::create_dir_all(&install_dir).map_err(crate::error::InstallerError::Io)?;

            let archive_name = archive_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let dest = if archive_name.ends_with(".exe") {
                install_dir.join(format!("{binary_name}.exe"))
            } else {
                install_dir.join(&binary_name)
            };

            fs::copy(&archive_path, &dest).map_err(crate::error::InstallerError::Io)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))
                    .map_err(crate::error::InstallerError::Io)?;
                // Strip quarantine attribute on macOS.
                let _ = std::process::Command::new("xattr")
                    .args(["-d", "com.apple.quarantine", &dest.to_string_lossy()])
                    .output();
            }

            Ok(())
        })
        .await
    }

    // ─── macOS runtime dependency check ───────────────────────────────────────

    /// Prompt the user to install OpenSSL 1.1 if the binary links against it but
    /// it is not present on the system.
    #[cfg(target_os = "macos")]
    pub(crate) async fn ensure_macos_openssl_runtime() -> crate::error::Result<()> {
        smol::unblock(macos_ensure_openssl).await
    }
}

// ─── Async pipeline ───────────────────────────────────────────────────────────

async fn run_installation(
    this: gpui::WeakEntity<InstallerView>,
    cx: &mut gpui::AsyncApp,
    release_info: super::super::ReleaseInfo,
    install_dir: PathBuf,
    sidecar_specs: Vec<(String, PathBuf)>,
    prefer_app_bundle: bool,
) {
    let download_manager = HttpDownloadManager::new();
    let github = GitHubReleases::new(GITHUB_ORG, GITHUB_REPO);
    let download_dir = std::env::temp_dir().join("pulsar-installer");

    if let Err(e) = std::fs::create_dir_all(&download_dir) {
        this.update(cx, |v, cx| {
            v.install_message = format!("Failed to create temp dir: {e}");
            v.install_failed = true;
            v.log(LogLevel::Error, format!("Temp dir error: {e}"), cx);
        })
        .ok();
        return;
    }

    this.update(cx, |v, cx| {
        v.install_message = format!("Resolving assets for {}…", release_info.name);
        v.log(LogLevel::Info, format!("Resolving {}", release_info.tag_name), cx);
    })
    .ok();

    // Resolve the full release + primary asset.
    let (full_release, asset) = match github.get_all_releases().await {
        Ok(releases) => {
            match releases.into_iter().find(|r| r.tag_name == release_info.tag_name) {
                Some(rel) => {
                    match InstallerView::select_asset_for(
                        "pulsar_engine",
                        &rel.assets,
                        prefer_app_bundle,
                        true,
                    ) {
                        Some(a) => (rel, a),
                        None => {
                            this.update(cx, |v, cx| {
                                v.install_message =
                                    "No compatible asset found for your platform.".to_string();
                                v.install_failed = true;
                                v.log(LogLevel::Error, "No compatible asset found.", cx);
                            })
                            .ok();
                            return;
                        }
                    }
                }
                None => {
                    this.update(cx, |v, cx| {
                        v.install_message = "Release not found on GitHub.".to_string();
                        v.install_failed = true;
                        v.log(LogLevel::Error, "Release not found.", cx);
                    })
                    .ok();
                    return;
                }
            }
        }
        Err(e) => {
            this.update(cx, |v, cx| {
                v.install_message = format!("Failed to fetch releases: {e}");
                v.install_failed = true;
                v.log(LogLevel::Error, format!("Fetch error: {e}"), cx);
            })
            .ok();
            return;
        }
    };

    // Download primary asset with progress reporting.
    let asset_name  = asset.name.clone();
    let total_size  = asset.size;
    let file_path   = download_dir.join(&asset.name);
    let url         = asset.browser_download_url.clone();
    let size_str    = InstallerView::format_bytes(total_size);

    this.update(cx, |v, cx| {
        v.install_message = format!("Downloading {asset_name} ({size_str})");
        v.log(LogLevel::Info, format!("Downloading {asset_name} ({size_str})"), cx);
    })
    .ok();

    let progress_state =
        std::sync::Arc::new(std::sync::Mutex::new((0u64, 0.0f32)));
    let progress_clone = progress_state.clone();
    let file_path_dl  = file_path.clone();
    let dm_clone      = download_manager.clone();

    let download_task = smol::spawn(async move {
        dm_clone
            .download(&url, &file_path_dl, Box::new(move |prog| {
                *progress_clone.lock().unwrap() = (prog.processed_bytes, prog.current);
            }))
            .await
    });

    // Poll progress while downloading.
    let mut last_update = std::time::Instant::now();
    loop {
        if download_task.is_finished() {
            break;
        }
        if last_update.elapsed() >= std::time::Duration::from_millis(100) {
            let (processed, file_pct) = *progress_state.lock().unwrap();
            let overall_pct = if total_size > 0 {
                (processed as f32 / total_size as f32) * 100.0
            } else {
                file_pct
            };
            let msg = format!("Downloading {asset_name} — {file_pct:.1}%");
            this.update(cx, |v, cx| {
                v.install_progress = overall_pct * 0.9; // reserve 10% for extraction
                v.install_message  = msg;
                cx.notify();
            })
            .ok();
            last_update = std::time::Instant::now();
        }
        smol::Timer::after(std::time::Duration::from_millis(50)).await;
    }

    match download_task.await {
        Ok(_) => {
            this.update(cx, |v, cx| {
                v.log(LogLevel::Success, format!("Downloaded {asset_name}"), cx);
                v.install_message = format!("Installing {}…", full_release.name);
                v.install_progress = 90.0;
                cx.notify();
            })
            .ok();

            match InstallerView::install_release(&file_path, &install_dir, &release_info.tag_name).await {
                Ok(()) => {
                    let final_path =
                        InstallerView::resolve_installed_path(&install_dir, &file_path);
                    let _ = write_metadata(&final_path, &release_info.tag_name);

                    // Install optional sidecar packages sequentially.
                    for (sidecar_id, sidecar_dir) in &sidecar_specs {
                        install_sidecar(
                            &this, cx, &full_release.assets, sidecar_id, sidecar_dir,
                            &download_dir, &download_manager,
                        )
                        .await;
                    }

                    this.update(cx, |v, cx| {
                        v.install_progress = 100.0;
                        v.install_message  = "Installation complete!".to_string();
                        v.installed_path   = Some(final_path.clone());
                        v.log(LogLevel::Success, format!("Installed → {}", final_path.display()), cx);
                        v.log(LogLevel::Success, "All done! Pulsar is ready.", cx);
                        v.current_page = Page::Complete;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |v, cx| {
                        v.install_progress = 100.0;
                        v.install_message  = "Installation completed with errors.".to_string();
                        v.install_failed   = true;
                        v.log(LogLevel::Error, format!("Install failed: {e}"), cx);
                        v.current_page = Page::Complete;
                        cx.notify();
                    })
                    .ok();
                }
            }
        }
        Err(e) => {
            this.update(cx, |v, cx| {
                v.install_progress = 100.0;
                v.install_message  = "Download failed.".to_string();
                v.install_failed   = true;
                v.log(LogLevel::Error, format!("Download failed: {e}"), cx);
                v.current_page = Page::Complete;
                cx.notify();
            })
            .ok();
        }
    }
}

/// Download and install a single sidecar package.
async fn install_sidecar(
    this: &gpui::WeakEntity<InstallerView>,
    cx: &mut gpui::AsyncApp,
    all_assets: &[crate::download::GitHubAsset],
    sidecar_id: &str,
    sidecar_dir: &PathBuf,
    download_dir: &PathBuf,
    download_manager: &HttpDownloadManager,
) {
    match InstallerView::select_asset_for(sidecar_id, all_assets, false, false) {
        Some(asset) => {
            let scar_name = asset.name.clone();
            let scar_url  = asset.browser_download_url.clone();
            let scar_path = download_dir.join(&scar_name);
            let size_str  = InstallerView::format_bytes(asset.size);

            this.update(cx, |v, cx| {
                v.log(LogLevel::Info, format!("Downloading {scar_name} ({size_str})"), cx);
                v.install_message = format!("Downloading {sidecar_id}…");
                cx.notify();
            })
            .ok();

            match download_manager
                .download(&scar_url, &scar_path, Box::new(|_| {}))
                .await
            {
                Ok(_) => {
                    match InstallerView::install_sidecar_binary(&scar_path, sidecar_dir, sidecar_id).await {
                        Ok(()) => {
                            this.update(cx, |v, cx| {
                                v.log(
                                    LogLevel::Success,
                                    format!("{sidecar_id} → {}", sidecar_dir.display()),
                                    cx,
                                );
                            })
                            .ok();
                        }
                        Err(e) => {
                            this.update(cx, |v, cx| {
                                v.log(LogLevel::Warning, format!("{sidecar_id} install failed: {e}"), cx);
                            })
                            .ok();
                        }
                    }
                }
                Err(e) => {
                    this.update(cx, |v, cx| {
                        v.log(LogLevel::Warning, format!("{sidecar_id} download failed: {e}"), cx);
                    })
                    .ok();
                }
            }
        }
        None => {
            this.update(cx, |v, cx| {
                v.log(
                    LogLevel::Warning,
                    format!("No {sidecar_id} asset for this platform, skipping."),
                    cx,
                );
            })
            .ok();
        }
    }
}

// ─── Archive helpers ──────────────────────────────────────────────────────────

fn extract_zip(
    archive_path: &PathBuf,
    install_dir: &PathBuf,
    is_app_zip: bool,
) -> crate::error::Result<()> {
    use std::fs;

    // .app.zip bundles are extracted into the *parent* so the bundle lands at install_dir.
    let extract_root = if is_app_zip {
        install_dir
            .parent()
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
        let mut zf = archive
            .by_index(i)
            .map_err(|e| crate::error::InstallerError::Other(e.to_string()))?;
        let out = extract_root.join(zf.mangled_name());

        if zf.name().ends_with('/') {
            fs::create_dir_all(&out).map_err(crate::error::InstallerError::Io)?;
        } else {
            if let Some(p) = out.parent() {
                fs::create_dir_all(p).map_err(crate::error::InstallerError::Io)?;
            }
            let mut outfile =
                fs::File::create(&out).map_err(crate::error::InstallerError::Io)?;
            std::io::copy(&mut zf, &mut outfile)
                .map_err(crate::error::InstallerError::Io)?;

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

    Ok(())
}

fn extract_tar_gz(archive_path: &PathBuf, install_dir: &PathBuf) -> crate::error::Result<()> {
    use std::fs;
    let file = fs::File::open(archive_path).map_err(crate::error::InstallerError::Io)?;
    let tar = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(tar);
    archive
        .unpack(install_dir)
        .map_err(crate::error::InstallerError::Io)
}

fn install_raw_binary(archive_path: &PathBuf, install_dir: &PathBuf) -> crate::error::Result<()> {
    // On macOS the platform post-install step handles binary mode; skip the
    // raw copy here to avoid clobbering the bundle root.
    #[cfg(not(target_os = "macos"))]
    {
        use std::fs;
        let dest = install_dir.join("pulsar");
        fs::copy(archive_path, &dest).map_err(crate::error::InstallerError::Io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))
                .map_err(crate::error::InstallerError::Io)?;
        }
    }
    #[cfg(target_os = "macos")]
    let _ = (archive_path, install_dir); // handled in platform_post_install
    Ok(())
}

// ─── Platform post-install ────────────────────────────────────────────────────

async fn platform_post_install(
    archive_path: &PathBuf,
    install_dir: &PathBuf,
    version: &str,
    is_app_zip: bool,
) -> crate::error::Result<()> {
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
        macos_post_install(archive_path, install_dir, version, is_app_zip).await?;
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

    #[allow(unused_variables)]
    let _ = (archive_path, install_dir, version, is_app_zip);
    Ok(())
}

#[cfg(target_os = "macos")]
async fn macos_post_install(
    archive_path: &PathBuf,
    install_dir: &PathBuf,
    version: &str,
    is_app_zip: bool,
) -> crate::error::Result<()> {
    use std::fs;

    if is_app_zip {
        // Pre-built bundle: just ensure executables inside the .app are runnable.
        use std::os::unix::fs::PermissionsExt;
        let app_dir = InstallerView::resolve_installed_path(install_dir, archive_path);
        let macos_dir = app_dir.join("Contents").join("MacOS");
        if let Ok(entries) = fs::read_dir(&macos_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o755));
                }
            }
        }
        return Ok(());
    }

    let is_bundle_path =
        install_dir.extension() == Some(std::ffi::OsStr::new("app"));

    if is_bundle_path {
        // Build a proper .app bundle via MacOSInstaller.
        use crate::platform::MacOSInstaller;
        use crate::traits::{Progress as Prog, ProgressCallback};
        let installer = MacOSInstaller::new(
            install_dir.clone(),
            version.to_string(),
            "pulsar".to_string(),
        );
        let progress: ProgressCallback = Box::new(|p: Prog| {
            tracing::info!("[{}%] {}", p.current, p.message.unwrap_or(""));
        });
        installer.install(archive_path, progress).await?;
    } else {
        // Binary mode: copy + chmod + quarantine strip.
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(install_dir).map_err(crate::error::InstallerError::Io)?;
        let dest = install_dir.join("pulsar");
        fs::copy(archive_path, &dest).map_err(crate::error::InstallerError::Io)?;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))
            .map_err(crate::error::InstallerError::Io)?;
        let _ = std::process::Command::new("xattr")
            .args(["-d", "com.apple.quarantine", &dest.to_string_lossy()])
            .output();

        // Check for OpenSSL 1.1 dependency in linked libraries.
        if let Ok(output) =
            std::process::Command::new("otool").arg("-L").arg(&dest).output()
        {
            let linked = String::from_utf8_lossy(&output.stdout);
            if linked.contains("libssl.1.1.dylib") || linked.contains("openssl@1.1") {
                InstallerView::ensure_macos_openssl_runtime().await?;
            }
        }
    }

    Ok(())
}

// ─── macOS OpenSSL runtime helper ─────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn macos_ensure_openssl() -> crate::error::Result<()> {
    use std::path::Path;
    use std::process::Command;

    let has_ssl = Path::new("/opt/homebrew/opt/openssl@1.1/lib/libssl.1.1.dylib").exists()
        || Path::new("/usr/local/opt/openssl@1.1/lib/libssl.1.1.dylib").exists();

    if has_ssl {
        return Ok(());
    }

    // Prompt the user before doing anything.
    let prompt = Command::new("osascript")
        .args([
            "-e",
            "display dialog \"This Pulsar engine binary requires OpenSSL 1.1. \
             Install Homebrew (if needed) and OpenSSL 1.1 now?\" \
             buttons {\"Cancel\", \"Install\"} default button \"Install\" with icon caution",
        ])
        .output()
        .map_err(crate::error::InstallerError::Io)?;

    if !prompt.status.success()
        || !String::from_utf8_lossy(&prompt.stdout).contains("Install")
    {
        return Err(crate::error::InstallerError::Other(
            "OpenSSL 1.1 dependency setup was cancelled by user.".to_string(),
        ));
    }

    let has_brew = Command::new("/bin/bash")
        .args(["-lc", "command -v brew >/dev/null 2>&1"])
        .status()
        .map_err(crate::error::InstallerError::Io)?
        .success();

    if has_brew {
        let install = Command::new("/bin/bash")
            .args(["-lc", "brew install openssl@1.1 || brew install rbenv/tap/openssl@1.1"])
            .output()
            .map_err(crate::error::InstallerError::Io)?;

        let has_ssl_after =
            Path::new("/opt/homebrew/opt/openssl@1.1/lib/libssl.1.1.dylib").exists()
                || Path::new("/usr/local/opt/openssl@1.1/lib/libssl.1.1.dylib").exists();

        if has_ssl_after {
            return Ok(());
        }

        return Err(crate::error::InstallerError::Other(format!(
            "Failed to install OpenSSL 1.1 via Homebrew: {}",
            String::from_utf8_lossy(&install.stderr).trim()
        )));
    }

    // No Homebrew — open Terminal so the user can set it up interactively.
    let cmd = "NONINTERACTIVE=1 /bin/bash -c \\\"$(curl -fsSL \
               https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\\\"; \
               brew install openssl@1.1 || brew install rbenv/tap/openssl@1.1; \
               echo; echo 'Dependency setup finished. Return to Pulsar Installer and retry.'";
    let escaped  = cmd.replace('\\', "\\\\").replace('"', "\\\"");
    let script   = format!("do script \"{escaped}\"");
    let _ = Command::new("osascript")
        .args(["-e", "tell application \"Terminal\"", "-e", &script, "-e", "activate", "-e", "end tell"])
        .status();

    Err(crate::error::InstallerError::Other(
        "Homebrew is required for OpenSSL 1.1. \
         A Terminal setup script was launched; complete it, then retry install."
            .to_string(),
    ))
}
