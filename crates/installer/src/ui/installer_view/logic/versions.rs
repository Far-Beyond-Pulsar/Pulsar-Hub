//! Installed-versions scanning and uninstallation.

use gpui::Context;
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
    pub fn uninstall_version(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(ver) = self.installed_versions.get(index) else {
            return;
        };

        let path = ver.metadata.install_path.clone();
        // For macOS .app bundles, remove from the parent version directory.
        let delete_path = if path.extension() == Some(std::ffi::OsStr::new("app")) {
            path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| path.clone())
        } else {
            path.clone()
        };

        self.uninstall_confirm = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let path_for_retain = path.clone();
            let result = smol::unblock(move || std::fs::remove_dir_all(&delete_path)).await;
            match result {
                Ok(_) => {
                    this.update(cx, |v, cx| {
                        v.installed_versions
                            .retain(|iv| iv.metadata.install_path != path_for_retain);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("Uninstall failed: {e}");
                }
            }
        })
        .detach();
    }
}
