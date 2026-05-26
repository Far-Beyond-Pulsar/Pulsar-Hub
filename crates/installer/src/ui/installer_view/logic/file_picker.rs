//! Native OS folder-picker integration (macOS/Linux/Windows).

use gpui::Context;
use std::path::PathBuf;
use super::super::{InstallerView, LogLevel};

impl InstallerView {
    /// Spawn a native folder-picker dialog and update `install_config.install_path`
    /// with the result (if the user confirmed).
    pub fn open_native_picker(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = smol::unblock(pick_folder).await;
            if let Some(path_str) = result {
                let path = PathBuf::from(path_str);
                this.update(cx, |v, cx| {
                    let normalized = InstallerView::normalize_versions_root(path);
                    if InstallerView::path_in_legal_area(&normalized) {
                        v.install_config.install_path = normalized;
                    } else {
                        let expected = InstallerView::default_versions_root();
                        v.log(
                            LogLevel::Warning,
                            format!(
                                "Selected path '{}' escapes sandbox. Keeping current path; expected root '{}'.",
                                normalized.display(),
                                expected.display()
                            ),
                            cx,
                        );
                        tracing::warn!(
                            "Selected path '{}' escapes sandbox. Expected '{}'.",
                            normalized.display(),
                            expected.display()
                        );
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}

// ─── Platform implementations ─────────────────────────────────────────────────

fn pick_folder() -> Option<String> {
    pick_folder_impl()
}

#[cfg(target_os = "macos")]
fn pick_folder_impl() -> Option<String> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            "POSIX path of (choose folder with prompt \"Select install folder:\")",
        ])
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn pick_folder_impl() -> Option<String> {
    // Try zenity first, then kdialog as fallback.
    let output = std::process::Command::new("zenity")
        .args(["--file-selection", "--directory", "--title=Select install folder"])
        .output()
        .or_else(|_| {
            std::process::Command::new("kdialog")
                .args(["--getexistingdirectory", "/home"])
                .output()
        })
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    }
}

#[cfg(windows)]
fn pick_folder_impl() -> Option<String> {
    let ps = r#"
Add-Type -AssemblyName System.Windows.Forms
$f = New-Object System.Windows.Forms.FolderBrowserDialog
$f.Description = 'Select install folder'
$null = $f.ShowDialog()
$f.SelectedPath
"#;
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", ps])
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn pick_folder_impl() -> Option<String> {
    None
}
