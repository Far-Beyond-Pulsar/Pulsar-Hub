//! Scanning and managing already-installed Pulsar versions.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ─── Data types ───────────────────────────────────────────────────────────────

/// JSON metadata written to `<install_dir>/.pulsar-install.json` after a
/// successful installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulsarInstallMetadata {
    pub version: String,
    /// RFC 3339 timestamp of when the installation completed.
    pub install_date: String,
    pub install_path: PathBuf,
}

/// One discovered installation, enriched with runtime data.
#[derive(Debug, Clone)]
pub struct InstalledVersion {
    pub metadata: PulsarInstallMetadata,
    /// Total size of all files under the install directory, in bytes.
    pub disk_size_bytes: u64,
    /// Set to `true` when a newer release is known to exist on GitHub.
    pub update_available: bool,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Scan the platform-specific default locations and return every Pulsar
/// installation found there.
pub fn scan_installed_versions() -> Vec<InstalledVersion> {
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for root in platform_search_roots() {
        if !root.exists() {
            continue;
        }

        // New layout: discover all installs by metadata file under this root.
        for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.file_name() != ".pulsar-install.json" {
                continue;
            }
            let Some(dir) = entry.path().parent() else {
                continue;
            };
            if let Some(ver) = try_load_from_dir(dir) {
                let key = ver.metadata.install_path.clone();
                if seen.insert(key) {
                    results.push(ver);
                }
            }
        }

        // Legacy layout fallback: allow direct root install without metadata walk hit.
        if let Some(ver) = try_load_from_dir(&root) {
            let key = ver.metadata.install_path.clone();
            if seen.insert(key) {
                results.push(ver);
            }
        }
    }

    results.sort_by(|a, b| b.metadata.install_date.cmp(&a.metadata.install_date));
    results
}

/// Write installation metadata to `<dir>/.pulsar-install.json`.
pub fn write_metadata(dir: &Path, version: &str) -> std::io::Result<()> {
    let metadata = PulsarInstallMetadata {
        version: version.to_string(),
        install_date: chrono::Utc::now().to_rfc3339(),
        install_path: dir.to_path_buf(),
    };
    let json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(dir.join(".pulsar-install.json"), json)
}

// ─── Internals ────────────────────────────────────────────────────────────────

fn try_load_from_dir(dir: &Path) -> Option<InstalledVersion> {
    if !dir.exists() {
        return None;
    }
    let meta_path = dir.join(".pulsar-install.json");
    let metadata: PulsarInstallMetadata = if meta_path.exists() {
        let content = std::fs::read_to_string(&meta_path).ok()?;
        serde_json::from_str(&content).ok()?
    } else {
        // Infer minimal metadata from the directory name when no JSON present.
        PulsarInstallMetadata {
            version: dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            install_date: String::new(),
            install_path: dir.to_path_buf(),
        }
    };
    let disk_size_bytes = dir_size(dir);
    Some(InstalledVersion {
        metadata,
        disk_size_bytes,
        update_available: false,
    })
}

/// Return directories to check on each supported platform.
fn platform_search_roots() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();

    #[cfg(target_os = "macos")]
    {
        return vec![
            // Version-managed roots
            PathBuf::from("/Applications/Pulsar"),
            home.join("Applications").join("Pulsar"),
            // Legacy single-install roots
            PathBuf::from("/Applications/Pulsar.app"),
            home.join("Applications").join("Pulsar.app"),
        ];
    }

    #[cfg(windows)]
    {
        let local_app = std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
        return vec![
            PathBuf::from(local_app).join("Programs").join("Pulsar"),
            PathBuf::from("C:\\Program Files\\Pulsar"),
        ];
    }

    #[cfg(target_os = "linux")]
    return vec![
        home.join(".local").join("share").join("pulsar"),
        home.join(".local").join("bin").join("pulsar"),
    ];

    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        vec![] // fallback for unsupported platforms
    }
}

/// Recursively sum the size of every regular file under `path`.
fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
