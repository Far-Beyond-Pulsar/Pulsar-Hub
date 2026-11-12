//! Themes from the theme registries (`Far-Beyond-Pulsar/Themes`, laid out like
//! the plugin registry: a `themes/` directory of files).
//!
//! A registry file is a complete theme file, exactly what the engine's theme
//! watcher loads. Installing one copies it into the one themes directory every
//! engine install shares and records it in `themes.lock.json`
//! (`pulsar_install_layout::ThemeLayout`); the engine's watcher picks it up.

use std::path::Path;

use pulsar_install_layout::{ThemeLayout, ThemeRecord};

use crate::core::types::PluginRegistry;
use crate::service::plugin_service::{registry_local_path, PluginService};

/// The official theme registry.
pub const DEFAULT_THEME_REGISTRY_URL: &str = "https://github.com/Far-Beyond-Pulsar/Themes";

/// A theme offered by a registry.
#[derive(Clone, Debug)]
pub struct RegistryTheme {
    /// The theme file's display name (its `name` field).
    pub name: String,
    pub author: String,
    /// The variants the file holds (`"Default Light"`, `"Default Dark"`).
    pub variants: Vec<String>,
    /// The file name it installs under.
    pub file: String,
    pub registry_url: String,
    /// The file's content.
    pub json: String,
}

pub struct ThemeService;

impl ThemeService {
    pub fn default_registries() -> Vec<PluginRegistry> {
        vec![PluginRegistry {
            name: "Official Pulsar Themes".to_string(),
            url: DEFAULT_THEME_REGISTRY_URL.to_string(),
        }]
    }

    /// Clone or update each registry, then list its themes. A registry that
    /// cannot be cloned is read over HTTPS instead.
    pub fn refresh(registries: &[PluginRegistry], registries_root: &Path) -> Vec<RegistryTheme> {
        let _ = PluginService::clone_or_pull_registries(registries, registries_root);
        let http = reqwest_client::apply_bundled_tls_blocking(reqwest::blocking::Client::builder())
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("Pulsar-Hub/1.0")
            .build()
            .ok();
        let mut seen = std::collections::HashSet::new();
        let mut themes = Vec::new();
        for registry in registries {
            let dir = registry_local_path(registries_root, &registry.url).join("themes");
            let found = if dir.is_dir() {
                Self::read_dir(&dir, &registry.url)
            } else {
                http.as_ref()
                    .map(|client| Self::fetch_over_http(&registry.url, client))
                    .unwrap_or_default()
            };
            for theme in found {
                if seen.insert(theme.file.clone()) {
                    themes.push(theme);
                }
            }
        }
        themes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        themes
    }

    fn read_dir(dir: &Path, registry_url: &str) -> Vec<RegistryTheme> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let file = entry.file_name().to_string_lossy().into_owned();
                let json = std::fs::read_to_string(entry.path()).ok()?;
                Self::parse(&file, json, registry_url)
            })
            .collect()
    }

    fn fetch_over_http(registry_url: &str, client: &reqwest::blocking::Client) -> Vec<RegistryTheme> {
        let Some((owner, repo)) = PluginService::parse_github_owner_repo(registry_url) else {
            return Vec::new();
        };
        let list_url = format!("https://api.github.com/repos/{owner}/{repo}/contents/themes");
        let Ok(response) = client.get(&list_url).send() else {
            return Vec::new();
        };
        if !response.status().is_success() {
            tracing::warn!("GitHub API returned {} for {list_url}", response.status());
            return Vec::new();
        }
        let Ok(files) = response.json::<Vec<serde_json::Value>>() else {
            return Vec::new();
        };
        files
            .into_iter()
            .filter_map(|file| {
                let name = file.get("name")?.as_str()?.to_string();
                let url = file.get("download_url")?.as_str()?;
                let json = client.get(url).send().ok()?.text().ok()?;
                Self::parse(&name, json, registry_url)
            })
            .collect()
    }

    /// A registry file as a [`RegistryTheme`], or `None` if it is not a theme
    /// file (a theme file has a `themes` array of variants).
    fn parse(file: &str, json: String, registry_url: &str) -> Option<RegistryTheme> {
        if !is_plain_json_file_name(file) {
            return None;
        }
        let value: serde_json::Value = serde_json::from_str(&json).ok()?;
        let variants: Vec<String> = value
            .get("themes")?
            .as_array()?
            .iter()
            .filter_map(|variant| variant.get("name")?.as_str().map(str::to_string))
            .collect();
        if variants.is_empty() {
            return None;
        }
        Some(RegistryTheme {
            name: value
                .get("name")
                .and_then(|name| name.as_str())
                .unwrap_or(file)
                .to_string(),
            author: value
                .get("author")
                .and_then(|author| author.as_str())
                .unwrap_or_default()
                .to_string(),
            variants,
            file: file.to_string(),
            registry_url: registry_url.to_string(),
            json,
        })
    }

    /// Write the theme into the shared themes directory and record it.
    pub fn install(layout: &ThemeLayout, theme: &RegistryTheme) -> Result<ThemeRecord, String> {
        if !is_plain_json_file_name(&theme.file) {
            return Err(format!("Not a theme file name: {}", theme.file));
        }
        std::fs::create_dir_all(layout.dir()).map_err(|e| e.to_string())?;
        let dest = layout.dir().join(&theme.file);
        // Through a temporary name the watcher ignores, then renamed into
        // place, so it never loads half a file.
        let temp = layout.dir().join(format!(".{}.tmp", theme.file));
        std::fs::write(&temp, theme.json.as_bytes()).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, &dest).map_err(|e| e.to_string())?;
        layout
            .register_theme(ThemeRecord {
                name: theme.name.clone(),
                file: theme.file.clone(),
                sha256: String::new(),
                registry_url: theme.registry_url.clone(),
                installed_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            })
            .map_err(|e| format!("Could not record the theme: {e}"))
    }

    pub fn remove(layout: &ThemeLayout, file: &str) -> Result<(), String> {
        if !is_plain_json_file_name(file) {
            return Err(format!("Not a theme file name: {file}"));
        }
        layout.unregister_theme(file).map_err(|e| e.to_string())
    }

    pub fn installed(layout: &ThemeLayout) -> Vec<ThemeRecord> {
        layout.load_lock().themes
    }
}

/// `name.json` with no directory part: a registry file name is never trusted
/// as a path.
fn is_plain_json_file_name(name: &str) -> bool {
    name.ends_with(".json")
        && name.len() > ".json".len()
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':'])
        && !name.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_json_names_are_accepted() {
        assert!(is_plain_json_file_name("nord.json"));
        for bad in ["../x.json", "a/b.json", "a\\b.json", ".hidden.json", "x.txt", ".json", "c:x.json"] {
            assert!(!is_plain_json_file_name(bad), "{bad}");
        }
    }

    #[test]
    fn a_file_without_variants_is_not_a_theme() {
        assert!(ThemeService::parse("a.json", r#"{"name":"A"}"#.into(), "r").is_none());
        let theme = ThemeService::parse(
            "a.json",
            r#"{"name":"A","author":"me","themes":[{"name":"A Dark"}]}"#.into(),
            "r",
        )
        .unwrap();
        assert_eq!(theme.variants, ["A Dark"]);
    }

    #[test]
    fn install_writes_the_file_and_records_it_outside_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let layout = ThemeLayout::at(dir.path().join("themes"), dir.path().join("themes.lock.json"));
        let theme = ThemeService::parse(
            "a.json",
            r#"{"name":"A","themes":[{"name":"A Dark"}]}"#.into(),
            "r",
        )
        .unwrap();
        ThemeService::install(&layout, &theme).unwrap();
        assert!(layout.dir().join("a.json").is_file());
        assert_eq!(ThemeService::installed(&layout).len(), 1);
        ThemeService::remove(&layout, "a.json").unwrap();
        assert!(ThemeService::installed(&layout).is_empty());
    }
}
