use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
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
    /// ISO-8601 publish timestamp from the GitHub API.
    #[serde(default)]
    pub published_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// A release channel a user can opt into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReleaseChannel {
    /// Anything `>= 1.0.0` from the main Pulsar-Native repo.
    Stable,
    /// Anything `< 1.0.0` from the main Pulsar-Native repo.
    Alpha,
    /// Anything from the dedicated Nightly repo.
    Nightly,
}

impl ReleaseChannel {
    pub const ALL: [ReleaseChannel; 3] = [
        ReleaseChannel::Stable,
        ReleaseChannel::Alpha,
        ReleaseChannel::Nightly,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ReleaseChannel::Stable => "Stable",
            ReleaseChannel::Alpha => "Alpha",
            ReleaseChannel::Nightly => "Nightly",
        }
    }

    pub fn repo(&self) -> &'static str {
        match self {
            ReleaseChannel::Stable | ReleaseChannel::Alpha => "Far-Beyond-Pulsar/Pulsar-Native",
            ReleaseChannel::Nightly => "Far-Beyond-Pulsar/Nightly",
        }
    }

    /// Whether this channel would show `release` (repo + version scoped).
    pub fn includes(&self, release: &GitHubRelease) -> bool {
        match self {
            ReleaseChannel::Nightly => true,
            ReleaseChannel::Stable => version_major(&release.tag_name).map(|m| m >= 1).unwrap_or(false),
            ReleaseChannel::Alpha => version_major(&release.tag_name)
                .map(|m| m >= 0 && m < 1)
                .unwrap_or(false),
        }
    }
}

/// Pagination/loading state for a single source repo being paged through.
#[derive(Debug, Clone)]
pub struct ChannelSource {
    pub repo: &'static str,
    pub page: u32,
    pub has_more: bool,
    pub loading: bool,
    pub error: Option<String>,
    /// Releases fetched so far for this repo (unfiltered by channel).
    pub fetched: Vec<GitHubRelease>,
}

impl ChannelSource {
    pub fn new(repo: &'static str) -> Self {
        Self {
            repo,
            page: 0,
            has_more: true,
            loading: false,
            error: None,
            fetched: Vec::new(),
        }
    }
}

/// The canonical set of repos used as release sources.
pub fn default_channel_sources() -> Vec<ChannelSource> {
    vec![
        ChannelSource::new(ReleaseChannel::Stable.repo()),
        ChannelSource::new(ReleaseChannel::Nightly.repo()),
    ]
}

/// Parse the leading numeric major/minor from a release tag for version checks.
fn version_major(tag: &str) -> Option<i64> {
    let t = tag
        .trim()
        .trim_start_matches(|c: char| c == 'v' || c == 'V' || c == ' ');
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Parse a leading `x.y.z` from a release tag / version string.
pub fn parse_version(tag: &str) -> Option<(u64, u64, u64)> {
    let t = tag
        .trim()
        .trim_start_matches(['v', 'V', '>', '<', '=', ' ']);
    let first: &str = t.split_whitespace().next().unwrap_or(t);
    let mut parts = first
        .split('.')
        .map(|p| p.trim_end_matches(|c: char| !c.is_ascii_digit()));
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let patch = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Some((major, minor, patch))
}

/// Parse the minimum version a project requires out of an `engine_version`
/// string. `">0.1.0"` → `0.1.0`; a `nightly-…` tag yields `None`.
pub fn required_min_version(required: &str) -> Option<(u64, u64, u64)> {
    let t = required.trim();
    if t.to_lowercase().starts_with("nightly-") {
        return None;
    }
    parse_version(t)
}

/// Whether `installed_version` satisfies a project's `required` engine version.
///
/// Exact `nightly-…` tags must match exactly; everything else is treated as a
/// minimum `x.y.z` requirement.
pub fn installed_satisfies(installed_version: &str, required: &str) -> bool {
    let req = required.trim();
    if req.eq_ignore_ascii_case("src") {
        return installed_version.trim().eq_ignore_ascii_case("src");
    }
    if req.to_lowercase().starts_with("nightly-") {
        return installed_version.trim() == req;
    }
    let Some(min) = required_min_version(req) else {
        return false;
    };
    parse_version(installed_version).map(|v| v >= min).unwrap_or(false)
}

/// Whether an installed set contains at least one version satisfying `required`.
/// Scanned installed versions plus the special local "src" engine (if a source
/// checkout is configured). The `src` entry ties projects that opt into the
/// `src` engine version to a local source checkout.
pub fn installed_versions_with_src(src: Option<&std::path::Path>) -> Vec<InstalledVersion> {
    let mut versions = scan_installed_versions();
    if let Some(src) = src {
        versions.retain(|v| v.metadata.version != "src");
        versions.push(InstalledVersion {
            metadata: PulsarInstallMetadata {
                version: "src".to_string(),
                install_date: chrono::Utc::now().to_rfc3339(),
                install_path: src.to_path_buf(),
            },
            disk_size_bytes: 0,
            update_available: false,
        });
        versions.sort_by(|a, b| b.metadata.install_date.cmp(&a.metadata.install_date));
    }
    versions
}

pub fn any_installed_satisfies(installed: &[InstalledVersion], required: &str) -> bool {
    installed
        .iter()
        .any(|v| installed_satisfies(&v.metadata.version, required))
}

/// Sort `releases` newest-first by their publish date.
pub fn sort_releases_newest_first(releases: &mut Vec<GitHubRelease>) {
    releases.sort_by(|a, b| release_date_millis(b).cmp(&release_date_millis(a)));
}

fn release_date_millis(release: &GitHubRelease) -> i64 {
    chrono::DateTime::parse_from_rfc3339(&release.published_at)
        .map(|d| d.timestamp_millis())
        .unwrap_or(0)
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
        || dir.join("pulsar_engine").is_file()
        || dir.join("pulsar_engine.exe").is_file()
        || dir.join("Contents").join("Info.plist").exists()
        || dir.join(".pulsar-install.json").is_file()
}

// ── GitHub Releases ─────────────────────────────────────────────────────────

const GITHUB_API: &str = "https://api.github.com/repos";

/// Number of releases returned per page by the GitHub releases API.
pub const RELEASES_PER_PAGE: u32 = 30;

/// Fetch one page of releases from a given `owner/repo` slug.
pub fn fetch_repo_releases_blocking(
    repo: &str,
    page: u32,
) -> Result<Vec<GitHubRelease>, String> {
    use std::time::Duration;

    let client = reqwest::blocking::Client::builder()
        .user_agent("Pulsar-Hub/1.0")
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!(
        "{}/{}/releases?page={}&per_page={}",
        GITHUB_API, repo, page, RELEASES_PER_PAGE
    );
    let mut last_err = String::new();
    for attempt in 1..=3 {
        match client.get(&url).send() {
            Ok(resp) if resp.status().is_success() => {
                return resp.json::<Vec<GitHubRelease>>().map_err(|e| e.to_string());
            }
            Ok(resp) if resp.status().is_server_error() => {
                last_err = format!("HTTP {} (attempt {}/3)", resp.status(), attempt);
                std::thread::sleep(Duration::from_secs(2 * attempt as u64));
            }
            Ok(resp) => {
                return Err(format!("HTTP {}", resp.status()));
            }
            Err(e) => {
                last_err = format!("{} (attempt {}/3)", e, attempt);
                std::thread::sleep(Duration::from_secs(2 * attempt as u64));
            }
        }
    }
    Err(last_err)
}

/// The `(repo, tag)` GitHub identifiers for a given installed engine version.
fn repo_tag_for_version(version: &str) -> (&'static str, String) {
    if version.to_lowercase().starts_with("nightly-") {
        ("Far-Beyond-Pulsar/Nightly", version.to_string())
    } else {
        let t = if version.starts_with(['v', 'V']) {
            version.to_string()
        } else {
            format!("v{}", version)
        };
        ("Far-Beyond-Pulsar/Pulsar-Native", t)
    }
}

/// Fetch the GitHub **release notes** (release `body`) for a given installed
/// engine version. Falls back to a short message when unavailable.
pub fn release_notes_for_version(version: &str) -> String {
    let (repo, tag) = repo_tag_for_version(version);
    let url = format!(
        "{}/{}/releases/tags/{}",
        GITHUB_API, repo, tag
    );

    let client = match reqwest::blocking::Client::builder()
        .user_agent("Pulsar-Hub/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(_) => return format!("Could not load release notes for **{}**.", version),
    };

    let resp = match client.get(&url).send() {
        Ok(r) if r.status().is_success() => r,
        _ => {
            return format!(
                "No release notes are available for engine version **{}**.",
                version
            )
        }
    };

    match resp.json::<GitHubRelease>() {
        Ok(release) if !release.body.trim().is_empty() => release.body,
        _ => format!(
            "No release notes are available for engine version **{}**.",
            version
        ),
    }
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
    // Nightly builds use `Pulsar-Native_<platform>_<hash>.zip` naming.
    if let Some(token) = nightly_platform_token(&os, &arch) {
        let needle = format!("pulsar-native_{}_", token);
        if let Some(a) = release
            .assets
            .iter()
            .find(|a| {
                let n = a.name.to_lowercase();
                n.starts_with("pulsar-native_") && n.contains(&needle) && n.ends_with(".zip")
            })
        {
            tracing::info!(
                "Asset selection: os={os} arch={arch} nightly_token={token} selected={}",
                a.name
            );
            return Some(a);
        }
        tracing::warn!(
            "No nightly asset matched token '{token}' (os={os} arch={arch}); available: {:?}",
            release.assets.iter().map(|a| a.name.as_str()).collect::<Vec<_>>()
        );
    }
    release.assets.iter().find(|a| {
        let n = a.name.to_lowercase();
        n.contains(&os) && n.contains(&arch)
    })
}

/// Map the current OS/arch to the Nightly asset platform token.
fn nightly_platform_token(os: &str, arch: &str) -> Option<String> {
    match (os, arch) {
        ("windows", "x86_64") => Some("x64".to_string()),
        ("windows", "aarch64") => Some("arm64".to_string()),
        ("macos", "x86_64") => Some("macos-x64".to_string()),
        ("macos", "aarch64") => Some("macos-arm64".to_string()),
        ("linux", "x86_64") => Some("linux-x64".to_string()),
        ("linux", "aarch64") => Some("linux-arm64".to_string()),
        _ => None,
    }
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
    let is_zip = url.to_lowercase().ends_with(".zip");
    let archive_path = if is_zip || ext != "exe" {
        let name = url
            .rsplit('/')
            .next()
            .filter(|n| !n.is_empty())
            .map(|n| n.to_string())
            .unwrap_or_else(|| {
                if is_zip {
                    format!("pulsar-{}.zip", version)
                } else {
                    format!("pulsar-{}.tar.gz", version)
                }
            });
        Some(dest_dir.parent().unwrap_or(dest_dir).join(name))
    } else {
        None
    };

    if let Some(archive_path) = &archive_path {
        let mut file = std::fs::File::create(archive_path).map_err(|e| e.to_string())?;
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
        progress_cb(80.0);
        if is_zip {
            extract_zip(archive_path, dest_dir).map_err(|e| e.to_string())?;
            flatten_archive_root(dest_dir);
            place_engine_binary_at_root(dest_dir);
        } else {
            extract_tar_gz(archive_path, dest_dir)
                .map_err(|e| e.to_string())?;
        }
        let _ = std::fs::remove_file(archive_path);
        write_metadata(dest_dir, version).map_err(|e| e.to_string())?;
    } else {
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
    }

    progress_cb(100.0);
    Ok(())
}

// ── Download Progress ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub done: bool,
    pub error: Option<String>,
}

impl Default for DownloadProgress {
    fn default() -> Self {
        Self {
            bytes_downloaded: 0,
            total_bytes: 0,
            speed_bps: 0,
            done: false,
            error: None,
        }
    }
}

pub fn download_and_extract_with_progress(
    url: &str,
    dest_dir: &Path,
    version: &str,
    progress: Arc<Mutex<DownloadProgress>>,
) {
    let client = match reqwest::blocking::Client::builder()
        .user_agent("Pulsar-Hub/1.0")
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let mut p = progress.lock();
            p.error = Some(e.to_string());
            p.done = true;
            return;
        }
    };

    let resp = match client.get(url).send() {
        Ok(r) => r,
        Err(e) => {
            let mut p = progress.lock();
            p.error = Some(e.to_string());
            p.done = true;
            return;
        }
    };
    if !resp.status().is_success() {
        let mut p = progress.lock();
        p.error = Some(format!("HTTP {}", resp.status()));
        p.done = true;
        return;
    }

    let total = resp.content_length().unwrap_or(0);
    {
        let mut p = progress.lock();
        p.total_bytes = total;
    }

    if let Err(e) = std::fs::create_dir_all(dest_dir) {
        let mut p = progress.lock();
        p.error = Some(e.to_string());
        p.done = true;
        return;
    }

    let (_, _, ext) = platform_info();
    // Nightly assets are `.zip` archives — even on Windows, where `ext` is
    // "exe" — so decide on the URL's extension before falling back to the
    // raw-exe / tar.gz paths.
    let is_zip = url.to_lowercase().ends_with(".zip");
    let write_result = if is_zip {
        let archive_name = url
            .rsplit('/')
            .next()
            .filter(|n| !n.is_empty())
            .unwrap_or(&format!("pulsar-{}.zip", version))
            .to_string();
        let archive_path = dest_dir.parent().unwrap_or(dest_dir).join(&archive_name);
        let r = download_file_with_progress(resp, archive_path.clone(), &progress);
        if r.is_ok() {
            {
                let mut p = progress.lock();
                p.speed_bps = 0;
            }
            if let Err(e) = extract_zip(&archive_path, dest_dir) {
                let mut p = progress.lock();
                p.error = Some(e);
                p.done = true;
                return;
            }
            flatten_archive_root(dest_dir);
            place_engine_binary_at_root(dest_dir);
            let _ = std::fs::remove_file(&archive_path);
        }
        r
    } else if ext == "exe" {
        download_file_with_progress(resp, dest_dir.join("pulsar.exe"), &progress)
    } else {
        let archive_name = url
            .rsplit('/')
            .next()
            .filter(|n| !n.is_empty())
            .unwrap_or(&format!("pulsar-{}.tar.gz", version))
            .to_string();
        let archive_path = dest_dir.parent().unwrap_or(dest_dir).join(&archive_name);
        let r = download_file_with_progress(resp, archive_path.clone(), &progress);
        if r.is_ok() {
            {
                let mut p = progress.lock();
                p.speed_bps = 0;
            }
            if let Err(e) = extract_tar_gz(&archive_path, dest_dir) {
                let mut p = progress.lock();
                p.error = Some(e);
                p.done = true;
                return;
            }
            let _ = std::fs::remove_file(&archive_path);
        }
        r
    };

    match write_result {
        Ok(()) => {
            let _ = write_metadata(dest_dir, version);
            let mut p = progress.lock();
            p.bytes_downloaded = p.total_bytes;
            p.done = true;
        }
        Err(e) => {
            let mut p = progress.lock();
            p.error = Some(e);
            p.done = true;
        }
    }
}

/// Many archives (e.g. Nightly zips) wrap everything in a single top-level
/// folder. If `dest` contains exactly one directory and no root files, hoist
/// that folder's contents up into `dest` so the engine binary ends up at the
/// install root.
fn flatten_archive_root(dest: &Path) {
    let mut root_files = 0;
    let mut subdirs: Vec<PathBuf> = Vec::new();
    let dirs = std::fs::read_dir(dest);
    if let Ok(entries) = dirs {
        for entry in entries.filter_map(|e| e.ok()) {
            let kind = entry.file_type().ok();
            if kind.map(|k| k.is_dir()).unwrap_or(false) {
                subdirs.push(entry.path());
            } else {
                root_files += 1;
            }
        }
    }
    // If there are already files at the root (e.g. pulsar.exe) or more than
    // one subdirectory, don't try to flatten.
    if root_files > 0 || subdirs.len() != 1 {
        return;
    }
    let inner = &subdirs[0];
    if let Ok(entries) = std::fs::read_dir(inner) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name();
            let target = dest.join(&name);
            let _ = if name.to_str().map(|n| n.ends_with('/')).unwrap_or(false) {
                Ok(())
            } else {
                std::fs::rename(entry.path(), target)
            };
        }
    }
    let _ = std::fs::remove_dir_all(inner);
}

/// Ensure the engine executable sits at the install root under the canonical
/// name (`pulsar.exe` on Windows, `pulsar` elsewhere). Nightly archives ship
/// the binary as `pulsar_engine.exe`, possibly inside subdirectories, so we
/// locate it and rename it into place.
fn place_engine_binary_at_root(dest: &Path) {
    let canonical = if cfg!(windows) { "pulsar.exe" } else { "pulsar" };
    let root_target = dest.join(canonical);
    if root_target.exists() {
        return;
    }
    #[cfg(windows)]
    let matches_binary = |name: &str| {
        let lower = name.to_ascii_lowercase();
        lower.starts_with("pulsar") && lower.ends_with(".exe")
    };
    #[cfg(not(windows))]
    let matches_binary = |name: &str| {
        let lower = name.to_ascii_lowercase();
        lower == "pulsar" || (lower.starts_with("pulsar_") && !lower.contains('.'))
    };
    for entry in WalkDir::new(dest)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy();
        if matches_binary(name.as_ref()) {
            let _ = std::fs::rename(entry.path(), &root_target);
            return;
        }
    }
}

fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut ar = tar::Archive::new(dec);
    ar.unpack(dest_dir).map_err(|e| format!("Extract failed: {}", e))
}

/// Extract a `.zip` archive (e.g. a Nightly build containing the binary and
/// PDB/symbol files) into `dest_dir`.
fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    use std::io::Read;

    let file = std::fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip: {}", e))?;

    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("Read zip entry: {}", e))?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out = dest_dir.join(rel);

        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::io::copy(&mut entry, &mut std::fs::File::create(&out).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                if mode & 0o100 != 0 {
                    std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode))
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}

fn download_file_with_progress(
    mut reader: reqwest::blocking::Response,
    dest: PathBuf,
    progress: &Arc<Mutex<DownloadProgress>>,
) -> Result<(), String> {
    let mut file = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut last_bytes: u64 = 0;
    let mut last_time = Instant::now();
    use std::io::Read;
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| e.to_string())?;
        downloaded += n as u64;
        let now = Instant::now();
        let elapsed = now.duration_since(last_time).as_secs_f64();
        if elapsed >= 0.15 {
            let bytes_since = downloaded - last_bytes;
            let speed = (bytes_since as f64 / elapsed) as u64;
            let mut p = progress.lock();
            p.bytes_downloaded = downloaded;
            p.speed_bps = speed;
            last_bytes = downloaded;
            last_time = now;
        }
    }
    {
        let mut p = progress.lock();
        p.bytes_downloaded = downloaded;
        p.speed_bps = 0;
    }
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

// TODO: Ensure the launched instance doesnt also open a shell window on Windows. This is a known issue and may require a different approach to launching the executable on Windows.
pub fn launch_engine(install_dir: &Path) -> Result<(), String> {
    launch_engine_inner(install_dir, None)
}

/// Launch the installed engine, using a `pulsar://open_project/<encoded>`
/// URI argument so it instantly opens `project`.
pub fn launch_engine_for_project(install_dir: &Path, project: &Path) -> Result<(), String> {
    launch_engine_inner(install_dir, Some(project))
}

/// Launch an engine binary (from a local source build) with a project.
pub fn launch_engine_binary_for_project(
    binary: &Path,
    current_dir: &Path,
    project: &Path,
) -> Result<(), String> {
    launch_binary(binary, current_dir, Some(project))
}

/// Launch an engine binary (from a local source build) standalone.
pub fn launch_engine_binary(binary: &Path, current_dir: &Path) -> Result<(), String> {
    launch_binary(binary, current_dir, None)
}

/// Live progress of a `cargo build` running in the background, surfaced to the
/// source-build overlay.
#[derive(Clone, Debug)]
pub struct BuildProgress {
    /// Number of crates that have finished compiling.
    pub done: usize,
    /// Total number of crates expected to build.
    pub total: usize,
    /// The crate currently (or most recently) being compiled.
    pub current: String,
    /// Recent compiler output lines (newest last), capped.
    pub logs: Vec<String>,
    /// Distinct crates that have compiled (from `compiler-artifact` messages).
    pub crates: std::collections::HashSet<String>,
    /// True once the cargo process has exited.
    pub finished: bool,
    pub error: Option<String>,
}

impl Default for BuildProgress {
    fn default() -> Self {
        Self {
            done: 0,
            total: 0,
            current: String::new(),
            logs: Vec::new(),
            crates: std::collections::HashSet::new(),
            finished: false,
            error: None,
        }
    }
}

/// Number of compiler log lines we retain in [`BuildProgress::logs`].
const MAX_BUILD_LOGS: usize = 200;

/// Compile the engine from a local source checkout (`cargo build --release`),
/// streaming progress into `progress`, and return the produced binary path.
pub fn compile_engine_src_with_progress(
    src: &Path,
    progress: Arc<Mutex<BuildProgress>>,
) -> Result<PathBuf, String> {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio as S;

    tracing::info!("Building engine from source at {:?}", src);

    {
        let mut p = progress.lock();
        p.total = src_crate_count(src).unwrap_or(0);
    }

    let mut child = match std::process::Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--message-format=json-render-diagnostics")
        .current_dir(src)
        .stdin(S::null())
        .stdout(S::piped())
        .stderr(S::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let mut p = progress.lock();
            p.finished = true;
            p.error = Some(format!("Failed to run cargo: {}", e));
            return Err(format!("Failed to run cargo: {}", e));
        }
    };

    // stdout carries the machine-readable JSON messages (crate artifacts,
    // rendered diagnostics, build-finished).
    if let Some(stdout) = child.stdout.take() {
        let p = progress.clone();
        std::thread::spawn(move || stream_cargo_json(stdout, &p));
    }
    // stderr may still carry human-friendly noise (index updates, downloads).
    if let Some(stderr) = child.stderr.take() {
        let p = progress.clone();
        std::thread::spawn(move || stream_cargo_text(stderr, &p));
    }

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            let mut p = progress.lock();
            p.finished = true;
            p.error = Some(e.to_string());
            return Err(e.to_string());
        }
    };

    let mut p = progress.lock();
    p.finished = true;
    if !status.success() {
        let msg = p
            .error
            .clone()
            .or_else(|| p.logs.last().cloned())
            .unwrap_or_else(|| "cargo build failed".to_string());
        p.error = Some(msg.clone());
        return Err(msg);
    }
    drop(p);

    for candidate in src_binary_candidates(src) {
        if candidate.exists() {
            tracing::info!("Built engine binary: {:?}", candidate);
            return Ok(candidate);
        }
    }
    Err("cargo build succeeded but produced binary could not be found".to_string())
}

/// Read cargo's `--message-format=json` stream from stdout and fold the exact
/// per-crate events into `progress`.
fn stream_cargo_json(reader: impl std::io::Read + Send + 'static, progress: &Arc<Mutex<BuildProgress>>) {
    use std::io::{BufRead, BufReader};
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(msg) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                    // Not JSON (rare) — surface as-is.
                    push_log(progress, trimmed.to_string());
                    continue;
                };
                let reason = msg.get("reason").and_then(|r| r.as_str()).unwrap_or("");
                match reason {
                    "compiler-artifact" => {
                        // A crate finished compiling.
                        let name = msg
                            .get("target")
                            .and_then(|t| t.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("crate");
                        let mut p = progress.lock();
                        if p.crates.insert(name.to_string()) {
                            p.done += 1;
                        }
                        p.current = format!("Compiling {}", name);
                        push_log_locked(&mut p, format!("Compiled {}", name));
                    }
                    "build-finished" => {
                        let success = msg
                            .get("success")
                            .and_then(|s| s.as_bool())
                            .unwrap_or(false);
                        let mut p = progress.lock();
                        p.finished = true;
                        if !success {
                            p.error = Some(
                                p.logs
                                    .last()
                                    .cloned()
                                    .unwrap_or_else(|| "cargo build failed".to_string()),
                            );
                        }
                        p.current = if success { "Done".to_string() } else { "Build failed".to_string() };
                    }
                    "compiler-message" => {
                        // Rendered diagnostics (errors/warnings) as plain text.
                        if let Some(rendered) = msg
                            .get("message")
                            .and_then(|m| m.get("rendered"))
                            .and_then(|r| r.as_str())
                        {
                            push_log(progress, rendered.trim_end().to_string());
                        }
                    }
                    "build-script-executed" | "build-plan" => {}
                    _ => {
                        if let Some(m) = msg.get("message") {
                            push_log(progress, m.to_string());
                        }
                    }
                }
            }
        }
    }
}

/// Read human-facing cargo output (stderr) and surface it as activity/noise.
fn stream_cargo_text(reader: impl std::io::Read + Send + 'static, progress: &Arc<Mutex<BuildProgress>>) {
    use std::io::{BufRead, BufReader};
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                let mut p = progress.lock();
                if p.current.is_empty() || !p.current.starts_with("Compiling") {
                    p.current = trimmed.clone();
                }
                push_log_locked(&mut p, trimmed);
            }
        }
    }
}

fn push_log(progress: &Arc<Mutex<BuildProgress>>, line: String) {
    let mut p = progress.lock();
    push_log_locked(&mut p, line);
}

fn push_log_locked(p: &mut BuildProgress, line: String) {
    p.logs.push(line);
    if p.logs.len() > MAX_BUILD_LOGS {
        let excess = p.logs.len() - MAX_BUILD_LOGS;
        p.logs.drain(0..excess);
    }
}

/// Precompute the number of crates cargo will compile (workspace members +
/// resolved dependencies), via `cargo metadata`.
fn src_crate_count(src: &Path) -> Option<usize> {
    let out = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(src)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice::<serde_json::Value>(&out.stdout)
        .ok()
        .and_then(|v| v.get("packages").cloned())
        .and_then(|packages| packages.as_array().map(|arr| arr.len()))
}

fn src_binary_candidates(src: &Path) -> Vec<PathBuf> {
    let release = src.join("target").join("release");
    let mut names: Vec<&str> = if cfg!(windows) {
        vec!["pulsar_engine.exe", "pulsar.exe"]
    } else if cfg!(target_os = "macos") {
        vec!["pulsar_engine", "pulsar"]
    } else {
        vec!["pulsar_engine", "pulsar"]
    };
    names.dedup();
    names.iter().map(|n| release.join(n)).collect()
}

fn launch_engine_inner(install_dir: &Path, project: Option<&Path>) -> Result<(), String> {
    // On macOS prefer launching the `.app` bundle when there is no project to
    // pass (bundle launches via `open` can't take CLI args).
    #[cfg(target_os = "macos")]
    if project.is_none() {
        let bundle = install_dir.join("pulsar.app");
        if bundle.exists() {
            return open::that(&bundle).map_err(|e| e.to_string());
        }
    }

    // Resolve the engine binary, tolerating the alternate `pulsar_engine.*`
    // name that Nightly archives ship and older installs may still use.
    let candidate_names: &[&str] = if cfg!(windows) {
        &["pulsar.exe", "pulsar_engine.exe"]
    } else if cfg!(target_os = "macos") {
        &[
            "Contents/MacOS/pulsar",
            "pulsar",
            "pulsar_engine",
        ]
    } else {
        &["pulsar", "pulsar_engine"]
    };
    let Some(exe) = candidate_names
        .iter()
        .map(|n| install_dir.join(n))
        .find(|p| p.exists())
    else {
        return Err(format!("Binary not found in {}", install_dir.display()));
    };

    launch_binary(&exe, install_dir, project)
}

fn launch_binary(exe: &Path, current_dir: &Path, project: Option<&Path>) -> Result<(), String> {
    // The engine does not accept a positional project path. It opens projects
    // via the `pulsar://open_project/<url-encoded-path>` URI scheme.
    let project_arg: Option<String> = project
        .map(|p| format!("pulsar://open_project/{}", percent_encode(&p.to_string_lossy())));

    let mut cmd = std::process::Command::new(exe);
    cmd.current_dir(current_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(arg) = project_arg {
        cmd.arg(&arg);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
        cmd.creation_flags(0x0000_0008 | 0x0000_0200);
    }
    crate::service::launch_flags::LaunchFlags::apply_env(
        exe.parent().unwrap_or_else(|| Path::new("")),
        &mut cmd,
    );
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
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
    let arch = native_arch();
    let ext = if cfg!(windows) {
        "exe".to_string()
    } else {
        "tar.gz".to_string()
    };
    (os, arch, ext)
}

/// Percent-encode a path for use inside a `pulsar://open_project/…` URI.
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        let c = byte as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

/// The real OS/CPU architecture for asset selection.
///
/// On Windows this queries the *native* system architecture rather than the
/// Hub's compiled architecture, so that an x64 Hub running under emulation on
/// ARM64 Windows still selects ARM64 engine builds (which otherwise fail to
/// launch with `ERROR_EXE_MACHINE_TYPE_MISMATCH`).
fn native_arch() -> String {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::{
            GetNativeSystemInfo, SYSTEM_INFO, PROCESSOR_ARCHITECTURE_AMD64,
            PROCESSOR_ARCHITECTURE_ARM64, PROCESSOR_ARCHITECTURE_INTEL,
        };
        let mut info = SYSTEM_INFO::default();
        unsafe {
            GetNativeSystemInfo(&mut info);
        }
        let a = unsafe { info.Anonymous.Anonymous.wProcessorArchitecture };
        if a == PROCESSOR_ARCHITECTURE_ARM64 {
            "aarch64".to_string()
        } else if a == PROCESSOR_ARCHITECTURE_AMD64 {
            "x86_64".to_string()
        } else if a == PROCESSOR_ARCHITECTURE_INTEL {
            "x86".to_string()
        } else {
            std::env::consts::ARCH.to_string()
        }
    }
    #[cfg(not(windows))]
    {
        std::env::consts::ARCH.to_string()
    }
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
