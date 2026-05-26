//! Helpers for opening install directories and launching the Pulsar binary.

use gpui::Context;
use std::path::PathBuf;
use super::super::InstallerView;

impl InstallerView {
    // ─── Open folder ──────────────────────────────────────────────────────────

    /// Reveal `self.installed_path` in the system file manager.
    pub fn open_install_folder(&self, cx: &mut Context<Self>) {
        if let Some(path) = self.installed_path.clone() {
            self.open_folder_path(path, cx);
        }
    }

    /// Reveal an arbitrary path in the system file manager.
    pub fn open_folder_path(&self, path: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |_, _| {
            let _ = smol::unblock(move || reveal_in_file_manager(&path)).await;
        })
        .detach();
    }

    // ─── Launch ───────────────────────────────────────────────────────────────

    /// Launch the engine at `self.installed_path`.
    pub fn launch_pulsar(&self, cx: &mut Context<Self>) {
        if let Some(path) = self.installed_path.clone() {
            self.launch_version_path(path, cx);
        }
    }

    /// Launch the engine at an arbitrary installed-version path.
    pub fn launch_version_path(&self, path: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |_, _| {
            let _ = smol::unblock(move || launch_engine(&path)).await;
        })
        .detach();
    }

    /// Launch a sidecar binary installed under the version root.
    pub fn launch_sidecar_path(&self, install_path: PathBuf, sidecar_id: String, cx: &mut Context<Self>) {
        cx.spawn(async move |_, _| {
            let _ = smol::unblock(move || launch_sidecar(&install_path, &sidecar_id)).await;
        })
        .detach();
    }
}

// ─── Platform implementations ─────────────────────────────────────────────────

fn reveal_in_file_manager(path: &PathBuf) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .args(["-R", &path.to_string_lossy()])
        .spawn();

    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(path).spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

fn launch_engine(path: &PathBuf) {
    #[cfg(target_os = "macos")]
    {
        if path.extension() == Some(std::ffi::OsStr::new("app")) {
            let _ = std::process::Command::new("open")
                .arg("-a")
                .arg(path)
                .spawn();
        } else {
            let _ = std::process::Command::new(path.join("pulsar")).spawn();
        }
    }

    #[cfg(windows)]
    let _ = std::process::Command::new(path.join("pulsar.exe")).spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new(path.join("pulsar")).spawn();
}

fn launch_sidecar(install_path: &PathBuf, sidecar_id: &str) {
    let version_root = version_root_from_install_path(install_path);
    let bin = sidecar_binary_path(&version_root, sidecar_id);
    let _ = std::process::Command::new(bin).spawn();
}

fn version_root_from_install_path(path: &PathBuf) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if path.extension() == Some(std::ffi::OsStr::new("app")) {
            if let Some(parent) = path.parent() {
                return parent.to_path_buf();
            }
        }
    }

    path.clone()
}

fn sidecar_binary_path(version_root: &PathBuf, sidecar_id: &str) -> PathBuf {
    #[cfg(windows)]
    {
        return version_root
            .join(sidecar_id)
            .join(format!("{sidecar_id}.exe"));
    }

    #[cfg(not(windows))]
    {
        version_root.join(sidecar_id).join(sidecar_id)
    }
}
