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

    /// Returns true when `path` is inside the installer's legal write/read area.
    pub fn path_in_legal_area(path: &Path) -> bool {
        if !path.is_absolute() {
            return false;
        }

        let path_norm = normalize_path(&std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));

        legal_roots()
            .into_iter()
            .map(|root| normalize_path(&std::fs::canonicalize(&root).unwrap_or(root)))
            .any(|root| path_norm.starts_with(&root) || path_norm == root)
    }

    /// Suggest a sandbox-safe equivalent path for warning messages.
    pub fn sandbox_expected_path(path: &Path) -> PathBuf {
        let legal_root = Self::default_versions_root();
        if let Some(name) = path.file_name() {
            legal_root.join(name)
        } else {
            legal_root
        }
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
                // Version-managed layout: <versions_root>/<version>/Pulsar.app
                // Only strip to parent when grandparent is the canonical versions root.
                if let Some(parent) = path.parent() {
                    if let Some(grandparent) = parent.parent() {
                        if normalize_path(grandparent) == normalize_path(&Self::default_versions_root()) {
                            return parent.to_path_buf();
                        }
                    }
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

fn legal_roots() -> Vec<PathBuf> {
    let mut roots = vec![InstallerView::default_versions_root()];

    #[cfg(target_os = "macos")]
    {
        // Legacy single-app installs created before version-managed root policy.
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join("Applications").join("Pulsar.app"));
        }
        roots.push(PathBuf::from("/Applications/Pulsar.app"));
    }

    roots
}

fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                out.push(component.as_os_str());
            }
        }
    }
    out
}
