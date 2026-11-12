use std::path::Path;

use crate::core::types::*;
use crate::service::git_service::GitService;

pub(crate) fn registry_local_path(registries_root: &Path, url: &str) -> std::path::PathBuf {
    let slug = url
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .replace(['/', ':'], "_");
    registries_root.join(slug)
}

#[cfg(target_os = "windows")]
fn native_plugin_ext() -> &'static str {
    "dll"
}
#[cfg(target_os = "macos")]
fn native_plugin_ext() -> &'static str {
    "dylib"
}
#[cfg(target_os = "linux")]
fn native_plugin_ext() -> &'static str {
    "so"
}

pub struct PluginService;

/// The release asset to install on this platform. Plugin releases publish one
/// library per platform, named `<crate>-<os>-<arch>.<ext>` (`...-windows-arm64.dll`),
/// beside a `.sha256` file and a `-package.zip`. Choosing the first `.dll` (as
/// this once did) installs another architecture's library. A release whose
/// libraries carry no platform in their names is taken as it is, if it has
/// exactly one.
pub(crate) fn pick_plugin_asset(
    assets: &[(String, String)],
    os: &str,
    arch: &str,
    ext: &str,
) -> Option<String> {
    let os = match os {
        "macos" => "macos",
        "windows" => "windows",
        _ => "linux",
    };
    let arch = match arch {
        "aarch64" | "arm64" => "arm64",
        "x86_64" | "amd64" => "x86_64",
        other => other,
    };
    let suffix = format!("-{os}-{arch}.{ext}");
    let libraries: Vec<&(String, String)> = assets
        .iter()
        .filter(|(name, _)| name.ends_with(&format!(".{ext}")))
        .collect();
    if let Some((_, url)) = libraries
        .iter()
        .find(|(name, _)| name.to_lowercase().ends_with(&suffix))
    {
        return Some(url.clone());
    }
    let names_a_platform = libraries.iter().any(|(name, _)| {
        let name = name.to_lowercase();
        ["-windows-", "-linux-", "-macos-"].iter().any(|p| name.contains(p))
    });
    match libraries.as_slice() {
        [(_, url)] if !names_a_platform => Some(url.clone()),
        _ => None,
    }
}

/// The Hub's view of what an install's plugin lock lists.
pub fn installed_plugins_of(layout: &pulsar_install_layout::InstallLayout) -> Vec<InstalledPlugin> {
    let dir = layout.plugins_dir();
    layout
        .load_plugin_lock()
        .plugins
        .into_iter()
        .map(|record| InstalledPlugin {
            name: record.name,
            repo_url: record.repo_url,
            version: record.version,
            installed_at: record.installed_at,
            install_method: if record.method == "source" {
                PluginInstallMethod::BuiltFromSource
            } else {
                PluginInstallMethod::BinaryDownload
            },
            library_path: dir.join(&record.file).to_string_lossy().into_owned(),
        })
        .collect()
}

impl PluginService {
    /// Clone or pull each plugin registry via git, then return the list of plugins found.
    /// Falls back to HTTP-fetching the manifest files from GitHub if the local copy
    /// does not exist (e.g. first launch, git unavailable, clone failure).
    pub fn clone_or_pull_registries(
        registries: &[PluginRegistry],
        root: &Path,
    ) -> Result<(), String> {
        let _ = std::fs::create_dir_all(root);
        for reg in registries {
            let local = registry_local_path(root, &reg.url);
            if local.join(".git").exists() {
                if let Err(e) = GitService::pull_updates(&local) {
                    tracing::warn!("git pull failed for {}: {e}", reg.url);
                }
            } else if let Err(e) = git2::Repository::clone(&reg.url, &local) {
                tracing::error!(
                    "git clone failed for {}: {e} — will fall back to HTTP fetch",
                    reg.url
                );
            }
        }
        Ok(())
    }

    /// Load all `RegistryPlugin` entries from the locally-cloned registry repos.
    /// If a registry hasn't been cloned yet, fetches its manifest JSON files over
    /// HTTPS from GitHub's raw content server as a fallback.
    pub fn load_plugins_from_registries(
        registries: &[PluginRegistry],
        registries_path: &Path,
    ) -> Vec<RegistryPlugin> {
        let http_client =
            reqwest_client::apply_bundled_tls_blocking(reqwest::blocking::Client::builder())
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("Pulsar-Native/1.0")
                .build()
                .ok();
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for reg in registries {
            let local_dir = registry_local_path(registries_path, &reg.url);
            let plugins_dir = local_dir.join("plugins");
            if plugins_dir.exists() {
                // Read from locally-cloned repo
                if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        if let Some(plugin) =
                            Self::load_single_plugin_file(entry.path(), &reg.url, &mut seen)
                        {
                            result.push(plugin);
                        }
                    }
                    continue;
                }
            }
            // Fallback: fetch manifest files over HTTPS
            if let Some(ref client) = http_client {
                if let Some(list) = Self::fetch_registry_via_http(&reg.url, client) {
                    for plugin in list {
                        if seen.insert(plugin.repo_url.clone()) {
                            result.push(plugin);
                        }
                    }
                }
            }
        }
        result
    }

    fn load_single_plugin_file(
        path: std::path::PathBuf,
        registry_url: &str,
        seen: &mut std::collections::HashSet<String>,
    ) -> Option<RegistryPlugin> {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            return None;
        }
        let text = std::fs::read_to_string(&path).ok()?;
        let mut plugin: RegistryPlugin = serde_json::from_str(&text).ok()?;
        plugin.registry_url = registry_url.to_string();
        if !seen.insert(plugin.repo_url.clone()) {
            return None;
        }
        Some(plugin)
    }

    /// Fetch all plugin manifest `.json` files from a registry GitHub repo via
    /// the GitHub Contents API and raw file downloads.
    fn fetch_registry_via_http(
        registry_url: &str,
        client: &reqwest::blocking::Client,
    ) -> Option<Vec<RegistryPlugin>> {
        // Extract owner/repo from a URL like https://github.com/owner/repo
        let trimmed = registry_url.trim_end_matches('/').trim_end_matches(".git");
        let (owner, repo) = trimmed
            .trim_start_matches("https://github.com/")
            .trim_start_matches("http://github.com/")
            .split_once('/')?;
        // List JSON files in the plugins/ directory via GitHub Contents API
        let list_url = format!("https://api.github.com/repos/{owner}/{repo}/contents/plugins");
        let resp = client.get(&list_url).send().ok()?;
        if !resp.status().is_success() {
            tracing::warn!("GitHub API returned {} for {list_url}", resp.status());
            return None;
        }
        let files: Vec<serde_json::Value> = resp.json().ok()?;
        let mut plugins = Vec::new();
        for file in files {
            let name = file.get("name")?.as_str()?;
            if !name.ends_with(".json") {
                continue;
            }
            let download_url = file.get("download_url")?.as_str()?;
            let raw_resp = client.get(download_url).send().ok()?;
            if !raw_resp.status().is_success() {
                continue;
            }
            let text: String = raw_resp.text().ok()?;
            if let Ok(mut plugin) = serde_json::from_str::<RegistryPlugin>(&text) {
                plugin.registry_url = registry_url.to_string();
                plugins.push(plugin);
            }
        }
        Some(plugins)
    }

    pub fn parse_github_owner_repo(url: &str) -> Option<(String, String)> {
        let stripped = url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("github.com/");
        let mut parts = stripped.splitn(2, '/');
        let owner = parts.next()?.to_string();
        let repo = parts.next()?.trim_end_matches(".git").to_string();
        if owner.is_empty() || repo.is_empty() {
            None
        } else {
            Some((owner, repo))
        }
    }

    pub fn fetch_latest_release(
        owner: &str,
        repo: &str,
    ) -> Result<Option<(String, Option<String>)>, String> {
        let url = format!("https://api.github.com/repos/{}/{}/releases", owner, repo);
        let client =
            reqwest_client::apply_bundled_tls_blocking(reqwest::blocking::Client::builder())
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("Pulsar-Native/1.0")
                .build()
                .map_err(|e| e.to_string())?;
        let resp = client.get(&url).send().map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("GitHub API returned {}", resp.status()));
        }
        let releases: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
        let arr = match releases.as_array() {
            Some(a) if !a.is_empty() => a,
            _ => return Ok(None),
        };
        let latest = &arr[0];
        let tag = latest
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let assets: Vec<(String, String)> = latest
            .get("assets")
            .and_then(|a| a.as_array())
            .map(|assets| {
                assets
                    .iter()
                    .filter_map(|a| {
                        Some((
                            a.get("name")?.as_str()?.to_string(),
                            a.get("browser_download_url")?.as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let binary_url = pick_plugin_asset(
            &assets,
            std::env::consts::OS,
            std::env::consts::ARCH,
            native_plugin_ext(),
        );
        Ok(Some((tag, binary_url)))
    }

    /// Record a library already in the install's plugin directory: hash it
    /// into the integrity manifest the engine requires and list it in the
    /// lock. Without this the engine refuses to load the library.
    pub fn register_installed(
        layout: &pulsar_install_layout::InstallLayout,
        plugin: &RegistryPlugin,
        version: &str,
        library_path: &str,
        method: &PluginInstallMethod,
    ) -> Result<InstalledPlugin, String> {
        let file = Path::new(library_path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("Not a library path: {library_path}"))?
            .to_string();
        let installed_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        layout
            .register_plugin(pulsar_install_layout::PluginRecord {
                name: plugin.name.clone(),
                repo_url: plugin.repo_url.clone(),
                version: version.to_string(),
                file,
                sha256: String::new(),
                registry_url: plugin.registry_url.clone(),
                installed_at: installed_at.clone(),
                method: match method {
                    PluginInstallMethod::BinaryDownload => "binary",
                    PluginInstallMethod::BuiltFromSource => "source",
                }
                .to_string(),
            })
            .map_err(|e| format!("Could not record the plugin: {e}"))?;
        Ok(InstalledPlugin {
            name: plugin.name.clone(),
            repo_url: plugin.repo_url.clone(),
            version: version.to_string(),
            installed_at,
            install_method: method.clone(),
            library_path: library_path.to_string(),
        })
    }

    pub fn download_binary(
        url: &str,
        plugins_dir: &Path,
        lib_name: &str,
    ) -> Result<String, String> {
        let client =
            reqwest_client::apply_bundled_tls_blocking(reqwest::blocking::Client::builder())
                .timeout(std::time::Duration::from_secs(120))
                .user_agent("Pulsar-Native/1.0")
                .build()
                .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(plugins_dir).map_err(|e| e.to_string())?;
        let dest = plugins_dir.join(lib_name);
        let report_progress = |_: u64, _: Option<u64>| {};
        let not_cancelled = || false;
        let options = crate::service::download::StreamOptions {
            progress: &report_progress,
            cancelled: &not_cancelled,
            resume: true,
        };
        crate::service::download::stream_to_file(&client, url, &dest, &options)?;
        verify_published_sha256(&client, url, &dest)?;
        Ok(dest.to_string_lossy().to_string())
    }

    pub fn build_from_source(
        repo_url: &str,
        tag: Option<&str>,
        plugins_dir: &Path,
        version: &str,
    ) -> Result<(String, Vec<String>), String> {
        use std::process::Command;
        let mut logs: Vec<String> = Vec::new();
        let tmp = std::env::temp_dir().join(format!(
            "pulsar_plugin_build_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        logs.push(format!("Cloning {}\u{2026}", repo_url));
        {
            let mut fo = git2::FetchOptions::new();
            fo.download_tags(git2::AutotagOption::None);
            let mut builder = git2::build::RepoBuilder::new();
            builder.fetch_options(fo);
            if let Some(t) = tag {
                builder.branch(t);
            }
            match builder.clone(repo_url, &tmp) {
                Ok(_) => logs.push("Clone succeeded".into()),
                Err(e) => return Err(format!("git clone failed: {e}")),
            }
        }
        logs.push("Building with cargo\u{2026}".to_string());
        let build = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&tmp)
            .output()
            .map_err(|e| format!("cargo build: {e}"))?;
        logs.push(String::from_utf8_lossy(&build.stderr).into_owned());
        if !build.status.success() {
            return Err(format!("cargo build failed:\n{}", logs.join("\n")));
        }
        let ext = native_plugin_ext();
        let release = tmp.join("target").join("release");
        let lib_file = std::fs::read_dir(&release)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.extension().and_then(|e| e.to_str()) == Some(ext)
                    && !p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .starts_with('.')
            })
            .ok_or_else(|| format!("No .{} file found in target/release/", ext))?;
        let plugin_name = repo_url
            .split('/')
            .last()
            .unwrap_or("plugin")
            .replace(['-', '.'], "_");
        let dest_name = format!("{plugin_name}_{version}.{ext}");
        std::fs::create_dir_all(plugins_dir).map_err(|e| e.to_string())?;
        let dest = plugins_dir.join(&dest_name);
        std::fs::copy(&lib_file, &dest).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&tmp);
        logs.push(format!("Installed to {}", dest.display()));
        Ok((dest.to_string_lossy().to_string(), logs))
    }
}

/// Check a downloaded library against the `<url>.sha256` the release publishes,
/// deleting it on a mismatch. A release with no checksum file is accepted:
/// the engine's own integrity manifest still pins the file afterwards.
fn verify_published_sha256(
    client: &reqwest::blocking::Client,
    url: &str,
    file: &Path,
) -> Result<(), String> {
    let Ok(response) = client.get(format!("{url}.sha256")).send() else {
        return Ok(());
    };
    if !response.status().is_success() {
        return Ok(());
    }
    let Some(expected) = response
        .text()
        .ok()
        .and_then(|text| text.split_whitespace().next().map(str::to_lowercase))
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
    else {
        return Ok(());
    };
    let actual = pulsar_install_layout::hash_file(file).map_err(|e| e.to_string())?;
    if actual != expected {
        let _ = std::fs::remove_file(file);
        return Err(format!(
            "The downloaded plugin does not match its published SHA-256 (expected {expected}, \
             got {actual}); it was deleted."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod asset_tests {
    use super::pick_plugin_asset;

    fn assets(names: &[&str]) -> Vec<(String, String)> {
        names
            .iter()
            .map(|n| (n.to_string(), format!("https://example.test/{n}")))
            .collect()
    }

    #[test]
    fn the_library_for_this_os_and_architecture_is_chosen() {
        let release = assets(&[
            "p-linux-x86_64.so",
            "p-linux-x86_64.so.sha256",
            "p-windows-x86_64-package.zip",
            "p-windows-x86_64.dll",
            "p-windows-arm64.dll",
            "p-windows-arm64.dll.sha256",
            "p.h",
        ]);
        assert_eq!(
            pick_plugin_asset(&release, "windows", "aarch64", "dll").as_deref(),
            Some("https://example.test/p-windows-arm64.dll")
        );
        assert_eq!(
            pick_plugin_asset(&release, "windows", "x86_64", "dll").as_deref(),
            Some("https://example.test/p-windows-x86_64.dll")
        );
        assert_eq!(
            pick_plugin_asset(&release, "linux", "x86_64", "so").as_deref(),
            Some("https://example.test/p-linux-x86_64.so")
        );
    }

    #[test]
    fn another_architectures_library_is_never_taken() {
        let release = assets(&["p-windows-x86_64.dll", "p-linux-x86_64.so"]);
        assert_eq!(pick_plugin_asset(&release, "windows", "aarch64", "dll"), None);
        assert_eq!(pick_plugin_asset(&release, "macos", "aarch64", "dylib"), None);
    }

    #[test]
    fn an_unlabelled_single_library_is_taken_as_it_is() {
        assert_eq!(
            pick_plugin_asset(&assets(&["my_plugin.dll"]), "windows", "aarch64", "dll").as_deref(),
            Some("https://example.test/my_plugin.dll")
        );
        assert_eq!(
            pick_plugin_asset(&assets(&["a.dll", "b.dll"]), "windows", "x86_64", "dll"),
            None
        );
    }
}
