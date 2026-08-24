pub fn normalize_project_path(path: &str) -> String {
    let buf = std::path::PathBuf::from(path);
    if let (Some(file_name), Some(parent)) = (buf.file_name(), buf.parent()) {
        if let Some(parent_name) = parent.file_name() {
            if file_name == parent_name {
                return parent.to_string_lossy().to_string();
            }
        }
    }
    path.to_string()
}

pub fn appdata_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("com", "Pulsar", "Pulsar_Engine")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

pub fn recent_projects_path() -> std::path::PathBuf {
    directories::ProjectDirs::from("com", "Pulsar", "Pulsar_Engine")
        .map(|d| d.data_dir().join("recent_projects.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("recent_projects.json"))
}

pub fn cloud_servers_path() -> std::path::PathBuf {
    directories::ProjectDirs::from("com", "Pulsar", "Pulsar_Engine")
        .map(|d| d.data_dir().join("cloud_servers.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("cloud_servers.json"))
}

pub fn plugins_dir() -> std::path::PathBuf {
    appdata_dir().join("plugins")
}

pub fn registries_dir() -> std::path::PathBuf {
    appdata_dir().join("registries")
}

pub fn template_cache_dir() -> std::path::PathBuf {
    appdata_dir().join("TemplateCache")
}

pub fn thumbnail_cache_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("com", "Pulsar", "Pulsar_Engine")
        .map(|d| d.cache_dir().join("template_thumbnails"))
        .unwrap_or_else(|| std::path::PathBuf::from("template_thumbnails"))
}

pub fn cloud_intro_seen_path() -> std::path::PathBuf {
    appdata_dir().join("cloud_intro_seen")
}

pub fn is_cloud_intro_seen() -> bool {
    cloud_intro_seen_path().exists()
}

pub fn mark_cloud_intro_seen() {
    let path = cloud_intro_seen_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, "1");
}
