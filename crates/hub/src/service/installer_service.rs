use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ── Data Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulsarInstallMetadata {
    pub version: String,
    pub install_date: String,
    pub install_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct InstalledVersion {
    pub metadata: PulsarInstallMetadata,
    pub disk_size_bytes: u64,
    pub update_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub assets: Vec<GitHubAsset>,
    pub prerelease: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VersionInstallState {
    Idle,
    FetchingReleases,
    Downloading {
        version: String,
        progress: f32,
    },
    Extracting {
        version: String,
    },
    Complete {
        version: String,
    },
    Error {
        version: String,
        message: String,
    },
}

// ── Scan Installed Versions ─────────────────────────────────────────────────

pub fn scan_installed_versions() -> Vec<InstalledVersion> {
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for root in platform_search_roots() {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(&root).max_depth(2).into_iter().filter_map(|e| e.ok()) {
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
                let key = canonical_or_same(&ver.metadata.install_path);
                if seen.insert(key) {
                    results.push(ver);
                }
            }
        }
        if let Some(ver) = try_load_from_dir(&root) {
            let key = canonical_or_same(&ver.metadata.install_path);
            if seen.insert(key) {
                results.push(ver);
            }
        }
    }

    results.sort_by(|a, b| b.metadata.install_date.cmp(&a.metadata.install_date));
    results
}

pub fn write_metadata(dir: &Path, version: &str) -> std::io::Result<()> {
    let metadata = PulsarInstallMetadata {
        version: version.to_string(),
        install_date: chrono::Utc::now().to_rfc3339(),
        install_path: dir.to_path_buf(),
    };
    let json =
        serde_json::to_string_pretty(&metadata).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(dir.join(".pulsar-install.json"), json)
}

fn try_load_from_dir(dir: &Path) -> Option<InstalledVersion> {
    if !dir.exists() {
        return None;
    }
    let meta_path = dir.join(".pulsar-install.json");
    let metadata: PulsarInstallMetadata = if meta_path.exists() {
        let content = std::fs::read_to_string(&meta_path).ok()?;
        let parsed: PulsarInstallMetadata = serde_json::from_str(&content).ok()?;
        if !looks_like_install_dir(dir) {
            return None;
        }
        let dir_norm = canonical_or_same(dir);
        let meta_norm = canonical_or_same(&parsed.install_path);
        if dir_norm != meta_norm {
            return None;
        }
        parsed
    } else {
        if !looks_like_install_dir(dir) {
            return None;
        }
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

fn looks_like_install_dir(dir: &Path) -> bool {
    dir.join("pulsar").is_file()
        || dir.join("pulsar.exe").is_file()
        || dir.join("Contents").join("Info.plist").exists()
}

// ── GitHub Releases ─────────────────────────────────────────────────────────

const GITHUB_API: &str = "https://api.github.com/repos/Far-Beyond-Pulsar/Pulsar-Native/releases";

pub fn fetch_releases_blocking() -> Result<Vec<GitHubRelease>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Pulsar-Hub/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(GITHUB_API).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let releases: Vec<GitHubRelease> = resp.json().map_err(|e| e.to_string())?;
    Ok(releases)
}

pub fn find_platform_asset(release: &GitHubRelease) -> Option<&GitHubAsset> {
    let (os, arch, ext) = platform_info();
    let candidates = [
        format!("pulsar_engine-{}-{}.{}", os, arch, ext),
        format!("pulsar_engine_{}_{}.{}", os, arch, ext),
        format!("pulsar-{}-{}.{}", os, arch, ext),
        format!("pulsar_{}_{}.{}", os, arch, ext),
        format!("{}-{}.{}", os, arch, ext),
        format!("{}_{}.{}", os, arch, ext),
    ];
    for pat in &candidates {
        if let Some(a) = release
            .assets
            .iter()
            .find(|a| a.name.to_lowercase().contains(&pat.to_lowercase()))
        {
            return Some(a);
        }
    }
    release.assets.iter().find(|a| {
        let n = a.name.to_lowercase();
        n.contains(&os) && n.contains(&arch)
    })
}

pub fn download_and_extract_blocking(
    url: &str,
    dest_dir: &Path,
    version: &str,
    progress_cb: impl Fn(f32),
) -> Result<(), String> {
    progress_cb(0.0);

    let client = reqwest::blocking::Client::builder()
        .user_agent("Pulsar-Hub/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Download failed: HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;

    let (_, _, ext) = platform_info();
    if ext == "exe" {
        let exe_path = dest_dir.join("pulsar.exe");
        let mut file = std::fs::File::create(&exe_path).map_err(|e| e.to_string())?;
        use std::io::Read;
        let mut reader = resp;
        let mut buf = [0u8; 8192];
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| e.to_string())?;
            downloaded += n as u64;
            if total > 0 {
                progress_cb((downloaded as f32 / total as f32) * 80.0);
            }
        }
        write_metadata(dest_dir, version).map_err(|e| e.to_string())?;
    } else {
        let archive_path = dest_dir.parent().unwrap_or(dest_dir).join(format!(
            "pulsar-{}.tar.gz",
            version
        ));
        {
            let mut file = std::fs::File::create(&archive_path).map_err(|e| e.to_string())?;
            use std::io::Read;
            let mut reader = resp;
            let mut buf = [0u8; 8192];
            loop {
                let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| e.to_string())?;
                downloaded += n as u64;
                if total > 0 {
                    progress_cb((downloaded as f32 / total as f32) * 80.0);
                }
            }
        }
        progress_cb(80.0);

        let archive_file = std::fs::File::open(&archive_path).map_err(|e| e.to_string())?;
        let dec = flate2::read::GzDecoder::new(archive_file);
        let mut ar = tar::Archive::new(dec);
        ar.unpack(dest_dir).map_err(|e| format!("Extract failed: {}", e))?;
        let _ = std::fs::remove_file(&archive_path);

        write_metadata(dest_dir, version).map_err(|e| e.to_string())?;
    }

    progress_cb(100.0);
    Ok(())
}

pub fn remove_version(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn open_install_dir(dir: &Path) {
    let _ = open::that(dir);
}

// ── Platform Helpers ────────────────────────────────────────────────────────

pub fn platform_search_roots() -> Vec<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();

    #[cfg(windows)]
    {
        let local_app = std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
        vec![
            PathBuf::from(local_app).join("Programs").join("Pulsar"),
            PathBuf::from("C:\\Program Files\\Pulsar"),
        ]
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/Applications/Pulsar"),
            home.join("Applications").join("Pulsar"),
        ]
    }
    #[cfg(target_os = "linux")]
    {
        vec![
            home.join(".local").join("share").join("pulsar"),
            home.join(".local").join("bin").join("pulsar"),
        ]
    }
}

pub fn default_install_path() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    #[cfg(windows)]
    {
        let local_app = std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".to_string());
        PathBuf::from(local_app)
            .join("Programs")
            .join("Pulsar")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Applications/Pulsar")
    }
    #[cfg(target_os = "linux")]
    {
        home.join(".local").join("share").join("pulsar")
    }
}

pub fn launch_engine(install_dir: &Path) -> Result<(), String> {
    let exe = if cfg!(windows) {
        install_dir.join("pulsar.exe")
    } else if cfg!(target_os = "macos") {
        install_dir
            .join("Contents")
            .join("MacOS")
            .join("pulsar")
    } else {
        install_dir.join("pulsar")
    };
    if !exe.exists() {
        return Err(format!("Binary not found: {}", exe.display()));
    }
    open::that(&exe).map_err(|e| e.to_string())
}

// ── Internals ───────────────────────────────────────────────────────────────

fn platform_info() -> (String, String, String) {
    let os = if cfg!(windows) {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else {
        "linux".to_string()
    };
    let arch = std::env::consts::ARCH.to_string();
    let ext = if cfg!(windows) {
        "exe".to_string()
    } else {
        "tar.gz".to_string()
    };
    (os, arch, ext)
}

fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

fn canonical_or_same(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
