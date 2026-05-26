//! Centralized path policy for installer write/read locations.

use std::path::{Path, PathBuf};

use super::super::InstallerView;

impl InstallerView {
    /// Canonical root where version subdirectories are created.
    pub fn default_versions_root() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));

        #[cfg(target_os = "macos")]
        {
            return home.join("Applications").join("Pulsar");
        }

        #[cfg(windows)]
        {
            return PathBuf::from(
                std::env::var("LOCALAPPDATA")
                    .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string()),
            )
            .join("Programs")
            .join("Pulsar");
        }

        #[cfg(target_os = "linux")]
        {
            return home.join(".local").join("share").join("pulsar");
        }

        #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
        {
            home.join("pulsar")
        }
    }

    /// Normalize user-entered install roots into the canonical versions-root form.
    pub fn normalize_versions_root(path: PathBuf) -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            if path.extension() == Some(std::ffi::OsStr::new("app")) {
                let is_pulsar_app = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.eq_ignore_ascii_case("Pulsar.app"))
                    .unwrap_or(false);
                if is_pulsar_app {
                    return path.with_extension("");
                }
            }
        }

        path
    }

    /// Build per-version install paths from the canonical versions root.
    pub fn compute_install_layout(
        versions_root: &Path,
        version_dir: &str,
        prefer_app_bundle: bool,
    ) -> (PathBuf, PathBuf) {
        let root = versions_root.join(version_dir);

        #[cfg(target_os = "macos")]
        {
            let engine_dir = if prefer_app_bundle {
                root.join("Pulsar.app")
            } else {
                root.clone()
            };
            return (engine_dir, root);
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = prefer_app_bundle;
            (root.clone(), root)
        }
    }

    /// Resolve an installed path to the version root directory.
    pub fn version_root_from_installed_path(path: &Path) -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            if path.extension() == Some(std::ffi::OsStr::new("app")) {
                if let Some(parent) = path.parent() {
                    return parent.to_path_buf();
                }
            }
        }

        path.to_path_buf()
    }

    pub fn sidecar_binary_path(version_root: &Path, sidecar_id: &str) -> PathBuf {
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
}
