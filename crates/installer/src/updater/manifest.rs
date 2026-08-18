use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub schema_version: u32,
    pub latest_version: String,
    pub generated_at: String,
    pub platforms: HashMap<String, PlatformUpdateInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformUpdateInfo {
    pub full_asset_name: String,
    pub full_url: String,
    pub full_sha256: String,
    pub full_size_bytes: u64,
    pub patches: Vec<PatchInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchInfo {
    pub from_version: String,
    pub to_version: String,
    pub asset_name: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let manifest = UpdateManifest {
            schema_version: 1,
            latest_version: "0.1.6".into(),
            generated_at: "2026-08-17T12:00:00Z".into(),
            platforms: HashMap::from([(
                "x86_64-pc-windows-msvc".into(),
                PlatformUpdateInfo {
                    full_asset_name: "pulsar-installer-windows-x86_64.exe".into(),
                    full_url: "https://github.com/Far-Beyond-Pulsar/Pulsar-Installer/releases/download/v0.1.6/pulsar-installer-windows-x86_64.exe".into(),
                    full_sha256: "abc123".into(),
                    full_size_bytes: 52428800,
                    patches: vec![PatchInfo {
                        from_version: "0.1.5".into(),
                        to_version: "0.1.6".into(),
                        asset_name: "pulsar-installer-patch-0.1.5-0.1.6-windows-x86_64.exe.zst".into(),
                        url: "https://github.com/Far-Beyond-Pulsar/Pulsar-Installer/releases/download/v0.1.6/pulsar-installer-patch-0.1.5-0.1.6-windows-x86_64.exe.zst".into(),
                        sha256: "def456".into(),
                        size_bytes: 1048576,
                    }],
                },
            )]),
        };

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let parsed: UpdateManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.latest_version, "0.1.6");
        assert_eq!(parsed.platforms.len(), 1);
    }
}
