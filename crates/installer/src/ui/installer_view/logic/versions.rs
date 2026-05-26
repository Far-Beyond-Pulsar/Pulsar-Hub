//! Installed-versions scanning and uninstallation.

use gpui::Context;
use std::path::{Path, PathBuf};
use crate::installed_versions::scan_installed_versions;
use super::super::InstallerView;

impl InstallerView {
    /// Async-scan for installed Pulsar engines and refresh the list.
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
            })
            .ok();
        })
        .detach();
    }

    /// Delete the installation at `index` from disk after clearing the confirm flag.
    ///
    /// The path to delete is validated before any filesystem operation; if it fails
    /// the safety checks the uninstall is aborted and an error is logged.
    pub fn uninstall_version(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(ver) = self.installed_versions.get(index) else {
            return;
        };

        let install_path = ver.metadata.install_path.clone();

        // ── Compute the version root to delete ────────────────────────────────
        //
        // Layout on disk:
        //   <base>/<version>/           ← version root  (what we delete)
        //     pulsar                    ← binary mode (Linux / Windows / macOS bare)
        //     Pulsar.app/               ← macOS bundle mode
        //       .pulsar-install.json
        //     .pulsar-install.json      ← binary mode
        //
        // The metadata `install_path` points to either:
        //   - the binary/bundle itself (macOS .app or bare binary dir)
        //   - the version root directory
        //
        // We want to delete the version root, NOT a parent of it.

        let delete_path: PathBuf = InstallerView::version_root_from_installed_path(&install_path);

        // ── Safety gate ───────────────────────────────────────────────────────
        match is_safe_to_delete(&delete_path) {
            Ok(()) => {}
            Err(reason) => {
                tracing::error!(
                    "Uninstall aborted — refusing to delete {:?}: {}",
                    delete_path,
                    reason
                );
                // Surface the error to the user via a log entry.
                self.log(
                    super::super::LogLevel::Error,
                    format!("Uninstall blocked: {reason}"),
                    cx,
                );
                self.uninstall_confirm = None;
                cx.notify();
                return;
            }
        }

        self.uninstall_confirm = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let path_for_retain = install_path.clone();
            let result = smol::unblock(move || std::fs::remove_dir_all(&delete_path)).await;
            match result {
                Ok(()) => {
                    this.update(cx, |v, cx| {
                        v.installed_versions
                            .retain(|iv| iv.metadata.install_path != path_for_retain);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("Uninstall failed: {e}");
                    this.update(cx, |v, cx| {
                        v.log(
                            super::super::LogLevel::Error,
                            format!("Uninstall failed: {e}"),
                            cx,
                        );
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}

// ─── Path-safety validation ───────────────────────────────────────────────────

/// Returns `Ok(())` if `path` is safe to pass to `remove_dir_all`, or an `Err`
/// with a human-readable reason string if the delete should be refused.
///
/// Rules (all must pass):
/// 1. Path must be absolute.
/// 2. Path must exist and be a directory.
/// 3. Path must live inside the user's home directory (or `LOCALAPPDATA` on Windows).
/// 4. Path must be at least **3 components deeper** than the root safe base
///    (e.g. home → `Applications` → `Pulsar` → `<version>`).
/// 5. Path must contain at least one component whose name contains "pulsar"
///    (case-insensitive) — ensuring we only ever delete Pulsar-owned directories.
/// 6. Path must not be a known well-known system / user directory.
fn is_safe_to_delete(path: &Path) -> Result<(), String> {
    // ── 1. Absolute path ──────────────────────────────────────────────────────
    if !path.is_absolute() {
        return Err(format!("path is not absolute: {:?}", path));
    }

    // ── 2. Must exist and be a directory ──────────────────────────────────────
    if !path.exists() {
        return Err(format!("path does not exist: {:?}", path));
    }
    if !path.is_dir() {
        return Err(format!("path is not a directory: {:?}", path));
    }

    // ── 3. Must live under the safe base ─────────────────────────────────────
    let safe_base = safe_base_dir();
    if !path.starts_with(&safe_base) {
        return Err(format!(
            "path {:?} is not inside the safe base directory {:?}",
            path, safe_base
        ));
    }

    // ── 4. Depth guard: must be at least 3 levels below safe_base ────────────
    //
    // Example good path:  ~/Applications/Pulsar/1.0.0        (depth 3)
    // Example good path:  ~/.local/share/pulsar/1.0.0        (depth 3)
    // Example bad path:   ~/Applications                     (depth 1) ← Applications itself
    // Example bad path:   ~/Applications/Pulsar              (depth 2) ← engine root, not a version
    let base_depth = safe_base.components().count();
    let path_depth = path.components().count();
    let relative_depth = path_depth.saturating_sub(base_depth);
    if relative_depth < 3 {
        return Err(format!(
            "path {:?} is too shallow (only {} levels below {:?}); \
             expected a version-specific subdirectory at depth ≥ 3",
            path, relative_depth, safe_base
        ));
    }

    // ── 5. Must contain a "pulsar" component (case-insensitive) ──────────────
    let has_pulsar_component = path.components().any(|c| {
        c.as_os_str()
            .to_string_lossy()
            .to_lowercase()
            .contains("pulsar")
    });
    if !has_pulsar_component {
        return Err(format!(
            "path {:?} does not contain a 'pulsar' directory component; \
             refusing to delete a directory that doesn't appear to be a Pulsar installation",
            path
        ));
    }

    // ── 6. Deny-list of well-known directories ────────────────────────────────
    //
    // Belt-and-suspenders: even if all other checks pass, never delete anything
    // whose final component matches a known important directory name.
    const DENY_LIST: &[&str] = &[
        // macOS
        "Applications",
        "Library",
        "Desktop",
        "Documents",
        "Downloads",
        "Movies",
        "Music",
        "Pictures",
        "Public",
        // Linux XDG
        ".local",
        ".config",
        ".cache",
        "share",
        "bin",
        "lib",
        "lib64",
        // Windows
        "AppData",
        "Programs",
        "ProgramData",
        "Users",
        "Windows",
        // Home dir itself
        "home",
    ];
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if DENY_LIST.iter().any(|&d| d.eq_ignore_ascii_case(name)) {
            return Err(format!(
                "path {:?} ends with a protected directory name '{}'",
                path, name
            ));
        }
    }

    Ok(())
}

/// The "safe base" directory: the user's home directory (or `LOCALAPPDATA` on
/// Windows).  Everything we install lives under this tree.
fn safe_base_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local);
            if p.is_absolute() {
                return p;
            }
        }
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp/nonexistent"))
}
