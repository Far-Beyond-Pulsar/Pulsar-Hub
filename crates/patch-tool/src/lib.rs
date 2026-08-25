use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Must match `MAX_PATCH_CHAIN_LENGTH` in crates/installer/src/updater/chain.rs.
pub const MAX_PATCH_CHAIN_LENGTH: usize = 4;

const ASSET_PREFIX: &str = "pulsar-installer-";
const PATCH_PREFIX: &str = "pulsar-installer-patch-";
const RELEASE_URL_BASE: &str =
    "https://github.com/Far-Beyond-Pulsar/Pulsar-Installer/releases/download";

const PLATFORM_SUFFIXES: &[(&str, &str)] = &[
    ("-windows-x86_64.exe", "x86_64-pc-windows-msvc"),
    ("-windows-i686.exe", "i686-pc-windows-msvc"),
    ("-windows-arm64.exe", "aarch64-pc-windows-msvc"),
    ("-macos-arm64", "aarch64-apple-darwin"),
    ("-macos-x86_64", "x86_64-apple-darwin"),
    ("-linux-x86_64", "x86_64-unknown-linux-gnu"),
    ("-linux-arm64", "aarch64-unknown-linux-gnu"),
    ("-linux-i686", "i686-unknown-linux-gnu"),
    ("-linux-armv7", "armv7-unknown-linux-gnueabihf"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub schema_version: u32,
    pub latest_version: String,
    pub generated_at: String,
    pub platforms: BTreeMap<String, PlatformUpdateInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformUpdateInfo {
    pub full_asset_name: String,
    pub full_url: String,
    pub full_sha256: String,
    pub full_size_bytes: u64,
    pub patches: Vec<PatchInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatchInfo {
    pub from_version: String,
    pub to_version: String,
    pub asset_name: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

pub fn generate_patch(old_path: &Path, new_path: &Path, patch_path: &Path) -> Result<String> {
    let old = std::fs::read(old_path)
        .with_context(|| format!("reading old binary: {}", old_path.display()))?;
    let new = std::fs::read(new_path)
        .with_context(|| format!("reading new binary: {}", new_path.display()))?;

    let mut uncompressed_patch = Vec::new();
    bsdiff::diff(&old, &new, &mut uncompressed_patch).context("bsdiff::diff failed")?;

    let compressed =
        zstd::encode_all(uncompressed_patch.as_slice(), 19).context("zstd compression failed")?;

    std::fs::write(patch_path, &compressed)
        .with_context(|| format!("writing patch: {}", patch_path.display()))?;

    let hash = sha256_file(patch_path)?;
    Ok(hash)
}

pub fn apply_patch(old_path: &Path, patch_path: &Path, new_path: &Path) -> Result<()> {
    let old = std::fs::read(old_path)
        .with_context(|| format!("reading old binary: {}", old_path.display()))?;
    let compressed = std::fs::read(patch_path)
        .with_context(|| format!("reading patch: {}", patch_path.display()))?;

    let uncompressed_patch =
        zstd::decode_all(compressed.as_slice()).context("zstd decompression failed")?;

    let mut new = Vec::with_capacity(old.len() + 1024);
    bsdiff::patch(&old, &mut uncompressed_patch.as_slice(), &mut new)
        .context("bsdiff::patch failed")?;

    std::fs::write(new_path, &new).with_context(|| format!("writing result: {}", new_path.display()))?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPatch {
    pub from_version: String,
    pub to_version: String,
    pub triple: &'static str,
}

pub fn parse_full_asset(filename: &str) -> Option<&'static str> {
    let rest = filename.strip_prefix(ASSET_PREFIX)?;
    PLATFORM_SUFFIXES
        .iter()
        .find(|(suffix, _)| rest == &suffix[1..])
        .map(|(_, triple)| *triple)
}

pub fn parse_patch_asset(filename: &str) -> Option<ParsedPatch> {
    let rest = filename.strip_prefix(PATCH_PREFIX)?.strip_suffix(".zst")?;
    for &(suffix, triple) in PLATFORM_SUFFIXES {
        if let Some(core) = rest.strip_suffix(suffix) {
            return split_versions(core).map(|(from_version, to_version)| ParsedPatch {
                from_version,
                to_version,
                triple,
            });
        }
    }
    None
}

fn split_versions(core: &str) -> Option<(String, String)> {
    core.match_indices('-').find_map(|(idx, _)| {
        let (from, to) = (&core[..idx], &core[idx + 1..]);
        (is_version(from) && is_version(to)).then(|| (from.to_string(), to.to_string()))
    })
}

pub fn is_version(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_digit())
        && s.contains('.')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'))
}

fn version_key(version: &str) -> Vec<u64> {
    version
        .split(['-', '+'])
        .next()
        .unwrap_or(version)
        .split('.')
        .map(|seg| {
            seg.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

fn max_version<'a>(versions: impl Iterator<Item = &'a str>) -> Option<String> {
    versions.max_by_key(|v| version_key(v)).map(String::from)
}

pub fn dedupe_patches(patches: Vec<PatchInfo>) -> Vec<PatchInfo> {
    let mut order: Vec<(String, String)> = Vec::new();
    let mut by_key: HashMap<(String, String), PatchInfo> = HashMap::new();
    for patch in patches {
        let key = (patch.from_version.clone(), patch.to_version.clone());
        if !by_key.contains_key(&key) {
            order.push(key.clone());
        }
        by_key.insert(key, patch);
    }
    order
        .into_iter()
        .filter_map(|key| by_key.remove(&key))
        .collect()
}

pub fn prune_patch_chain(
    patches: &[PatchInfo],
    latest_version: &str,
    max_len: usize,
) -> Vec<PatchInfo> {
    let mut chain: Vec<&PatchInfo> = Vec::new();
    let mut visited: HashSet<&str> = HashSet::new();
    visited.insert(latest_version);
    let mut current = latest_version;

    while chain.len() < max_len {
        let Some(hop) = patches
            .iter()
            .rev()
            .find(|p| p.to_version == current && !visited.contains(&p.from_version.as_str()))
        else {
            break;
        };
        chain.push(hop);
        current = &hop.from_version;
        visited.insert(current);
    }

    chain.reverse();
    chain.into_iter().cloned().collect()
}

pub fn generate_manifest(
    release_dir: &Path,
    prev_manifest: Option<&Path>,
    latest_version_override: Option<&str>,
) -> Result<UpdateManifest> {
    let mut fulls: BTreeMap<&'static str, (String, String, u64)> = BTreeMap::new();
    let mut new_patches: BTreeMap<&'static str, Vec<PatchInfo>> = BTreeMap::new();

    let entries = std::fs::read_dir(release_dir)
        .with_context(|| format!("reading release dir {}", release_dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("reading {}", release_dir.display()))?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(triple) = parse_full_asset(&name) {
            let sha256 = sha256_file(&path)?;
            let size_bytes = std::fs::metadata(&path)?.len();
            fulls.insert(triple, (name, sha256, size_bytes));
        } else if let Some(parsed) = parse_patch_asset(&name) {
            let sha256 = sha256_file(&path)?;
            let size_bytes = std::fs::metadata(&path)?.len();
            new_patches.entry(parsed.triple).or_default().push(PatchInfo {
                from_version: parsed.from_version,
                to_version: parsed.to_version,
                asset_name: name,
                url: String::new(),
                sha256,
                size_bytes,
            });
        }
    }

    if fulls.is_empty() {
        anyhow::bail!(
            "no full installer assets found in {}",
            release_dir.display()
        );
    }

    let prev_manifest = match prev_manifest {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("reading previous manifest {}", path.display()))?;
            let manifest: UpdateManifest = serde_json::from_str(&text)
                .with_context(|| format!("parsing previous manifest {}", path.display()))?;
            Some(manifest)
        }
        None => None,
    };

    if let Some(prev) = &prev_manifest {
        for triple in prev.platforms.keys() {
            if !fulls.contains_key(triple.as_str()) {
                anyhow::bail!(
                    "platform '{}' is present in the previous manifest but no matching \
                     full asset was found in {}",
                    triple,
                    release_dir.display()
                );
            }
        }
    }

    let latest_version = match latest_version_override {
        Some(v) => v.to_string(),
        None => max_version(
            new_patches
                .values()
                .flat_map(|patches| patches.iter())
                .map(|p| p.to_version.as_str())
                .chain(
                    prev_manifest
                        .iter()
                        .flat_map(|m| m.platforms.values())
                        .flat_map(|p| p.patches.iter())
                        .map(|p| p.to_version.as_str()),
                ),
        )
        .context("cannot infer latest version from patch targets; pass --latest-version")?,
    };

    let generated_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut platforms = BTreeMap::new();

    for (triple, (asset_name, sha256, size_bytes)) in fulls {
        let prev_patches = prev_manifest
            .as_ref()
            .and_then(|m| m.platforms.get(triple))
            .map(|p| p.patches.clone())
            .unwrap_or_default();

        let mut merged = prev_patches;
        if let Some(patches) = new_patches.get(triple) {
            merged.extend(patches.iter().cloned());
        }
        for patch in &mut merged {
            if patch.url.is_empty() {
                patch.url = format!(
                    "{}/v{}/{}",
                    RELEASE_URL_BASE, patch.to_version, patch.asset_name
                );
            }
        }

        let deduped = dedupe_patches(merged);
        let patches = prune_patch_chain(&deduped, &latest_version, MAX_PATCH_CHAIN_LENGTH);

        platforms.insert(
            triple.to_string(),
            PlatformUpdateInfo {
                full_asset_name: asset_name.clone(),
                full_url: format!("{}/v{}/{}", RELEASE_URL_BASE, latest_version, asset_name),
                full_sha256: sha256,
                full_size_bytes: size_bytes,
                patches,
            },
        );
    }

    Ok(UpdateManifest {
        schema_version: 1,
        latest_version,
        generated_at,
        platforms,
    })
}

/// Generate a fresh ed25519 keypair for manifest signing.
///
/// Returns `(seed_hex, pubkey_hex)` where the seed is the 32-byte private key
/// (store it as a secret — it can regenerate the whole keypair) and the
/// pubkey is the 32-byte verification key to pin in the updater binary.
pub fn generate_keypair() -> Result<(String, String)> {
    use rand_core::RngCore;

    let mut seed = [0u8; 32];
    rand_core::OsRng.fill_bytes(&mut seed);
    let signing = SigningKey::from_bytes(&seed);
    Ok((
        hex::encode(signing.to_bytes()),
        hex::encode(signing.verifying_key().to_bytes()),
    ))
}

/// Sign the file at `path` with an ed25519 private key given as a 32-byte hex
/// seed, writing a detached signature to `<path>.sig` (hex-encoded).
///
/// Returns the path of the written signature file.
pub fn sign_file(path: &Path, seed_hex: &str) -> Result<String> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let signing = signing_key_from_seed(seed_hex)?;
    let signature = signing.sign(&data);

    let file_name = path
        .file_name()
        .context("invalid manifest path (no file name)")?;
    let sig_path = path.with_file_name(format!("{}.sig", file_name.to_string_lossy()));
    std::fs::write(&sig_path, hex::encode(signature.to_bytes()))
        .with_context(|| format!("writing signature: {}", sig_path.display()))?;
    Ok(sig_path.display().to_string())
}

fn signing_key_from_seed(seed_hex: &str) -> Result<SigningKey> {
    let seed_bytes = hex::decode(seed_hex.trim()).context("signing key must be 64 hex chars")?;
    let seed: [u8; 32] = seed_bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("signing key seed must be 32 bytes"))?;
    Ok(SigningKey::from_bytes(&seed))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn parses_full_asset_names() {
        assert_eq!(
            parse_full_asset("pulsar-installer-windows-x86_64.exe"),
            Some("x86_64-pc-windows-msvc")
        );
        assert_eq!(
            parse_full_asset("pulsar-installer-windows-arm64.exe"),
            Some("aarch64-pc-windows-msvc")
        );
        assert_eq!(
            parse_full_asset("pulsar-installer-macos-arm64"),
            Some("aarch64-apple-darwin")
        );
        assert_eq!(
            parse_full_asset("pulsar-installer-linux-x86_64"),
            Some("x86_64-unknown-linux-gnu")
        );
        assert_eq!(
            parse_full_asset("pulsar-installer-linux-armv7"),
            Some("armv7-unknown-linux-gnueabihf")
        );
    }

    #[test]
    fn rejects_non_asset_filenames() {
        assert_eq!(parse_full_asset("pulsar-installer-windows-x86_64.exe.sha256"), None);
        assert_eq!(parse_full_asset("update-manifest.json"), None);
        assert_eq!(parse_full_asset("pulsar-installer-unknown-platform"), None);
    }

    #[test]
    fn parses_patch_names() {
        let parsed =
            parse_patch_asset("pulsar-installer-patch-0.1.5-0.1.6-windows-x86_64.exe.zst")
                .unwrap();
        assert_eq!(parsed.from_version, "0.1.5");
        assert_eq!(parsed.to_version, "0.1.6");
        assert_eq!(parsed.triple, "x86_64-pc-windows-msvc");

        let parsed = parse_patch_asset("pulsar-installer-patch-0.1.5-0.1.6-macos-arm64.zst")
            .unwrap();
        assert_eq!(parsed.triple, "aarch64-apple-darwin");
    }

    #[test]
    fn parses_patch_names_with_prerelease_versions() {
        let parsed = parse_patch_asset(
            "pulsar-installer-patch-0.2.0-beta.1-0.2.0-linux-armv7.zst",
        )
        .unwrap();
        assert_eq!(parsed.from_version, "0.2.0-beta.1");
        assert_eq!(parsed.to_version, "0.2.0");
        assert_eq!(parsed.triple, "armv7-unknown-linux-gnueabihf");
    }

    #[test]
    fn rejects_bad_patch_names() {
        assert_eq!(
            parse_patch_asset("pulsar-installer-patch-0.1.5-windows-x86_64.exe.zst"),
            None
        );
        assert_eq!(
            parse_patch_asset("pulsar-installer-patch-0.1.5-notaversion-linux-x86_64.zst"),
            None
        );
        assert_eq!(parse_patch_asset("pulsar-installer-windows-x86_64.exe"), None);
    }

    #[test]
    fn dedupe_keeps_newest_occurrence() {
        let older = PatchInfo {
            from_version: "0.1.5".into(),
            to_version: "0.1.6".into(),
            asset_name: "old".into(),
            url: "https://old".into(),
            sha256: "aaa".into(),
            size_bytes: 1,
        };
        let newer = PatchInfo {
            sha256: "bbb".into(),
            size_bytes: 2,
            ..older.clone()
        };
        let deduped = dedupe_patches(vec![older, newer.clone()]);
        assert_eq!(deduped, vec![newer]);
    }

    #[test]
    fn prune_keeps_most_recent_hops_ending_at_latest() {
        let mk = |from: &str, to: &str| PatchInfo {
            from_version: from.into(),
            to_version: to.into(),
            asset_name: format!("{from}-{to}"),
            url: String::new(),
            sha256: String::new(),
            size_bytes: 0,
        };

        let patches = vec![
            mk("0.1.0", "0.1.1"),
            mk("0.1.1", "0.1.2"),
            mk("0.1.2", "0.1.3"),
            mk("0.1.3", "0.1.4"),
            mk("0.1.4", "0.1.5"),
        ];
        let pruned = prune_patch_chain(&patches, "0.1.5", MAX_PATCH_CHAIN_LENGTH);
        assert_eq!(pruned.len(), MAX_PATCH_CHAIN_LENGTH);
        assert_eq!(pruned.first().unwrap().from_version, "0.1.1");
        assert_eq!(pruned.last().unwrap().to_version, "0.1.5");
    }

    #[test]
    fn prune_stops_at_gap_and_ignores_unlinked_branches() {
        let mk = |from: &str, to: &str| PatchInfo {
            from_version: from.into(),
            to_version: to.into(),
            asset_name: String::new(),
            url: String::new(),
            sha256: String::new(),
            size_bytes: 0,
        };

        let patches = vec![
            mk("0.9.0", "0.9.1"),
            mk("0.1.4", "0.1.5"),
        ];
        let pruned = prune_patch_chain(&patches, "0.1.5", MAX_PATCH_CHAIN_LENGTH);
        assert_eq!(pruned, vec![mk("0.1.4", "0.1.5")]);
    }

    #[test]
    fn prune_terminates_on_cycle() {
        let mk = |from: &str, to: &str| PatchInfo {
            from_version: from.into(),
            to_version: to.into(),
            asset_name: String::new(),
            url: String::new(),
            sha256: String::new(),
            size_bytes: 0,
        };

        let patches = vec![mk("0.1.1", "0.1.2"), mk("0.1.2", "0.1.1")];
        let pruned = prune_patch_chain(&patches, "0.1.2", MAX_PATCH_CHAIN_LENGTH);
        assert_eq!(pruned, vec![mk("0.1.1", "0.1.2")]);
    }

    #[test]
    fn manifest_json_matches_installer_schema() {
        let manifest: UpdateManifest = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "latest_version": "0.1.6",
                "generated_at": "2026-08-17T12:00:00Z",
                "platforms": {}
            }"#,
        )
        .unwrap();
        let json = serde_json::to_value(&manifest).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["latest_version"], "0.1.6");
        for key in ["generated_at", "platforms"] {
            assert!(json.get(key).is_some(), "missing key {key}");
        }
    }

    #[test]
    fn end_to_end_manifest_generation() {
        let dir = std::env::temp_dir().join("pulsar_gen_manifest_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("pulsar-installer-windows-x86_64.exe"), b"binary-v2").unwrap();
        std::fs::write(dir.join("pulsar-installer-linux-x86_64"), b"binary-v2").unwrap();
        std::fs::write(
            dir.join("pulsar-installer-patch-0.1.5-0.1.6-windows-x86_64.exe.zst"),
            b"patchdata",
        )
        .unwrap();

        let prev_json = r#"{
            "schema_version": 1,
            "latest_version": "0.1.5",
            "generated_at": "2026-07-01T00:00:00Z",
            "platforms": {
                "x86_64-pc-windows-msvc": {
                    "full_asset_name": "pulsar-installer-windows-x86_64.exe",
                    "full_url": "https://example.com/old.exe",
                    "full_sha256": "deadbeef",
                    "full_size_bytes": 10,
                    "patches": [
                        {
                            "from_version": "0.1.4",
                            "to_version": "0.1.5",
                            "asset_name": "pulsar-installer-patch-0.1.4-0.1.5-windows-x86_64.exe.zst",
                            "url": "https://example.com/v0.1.5/pulsar-installer-patch-0.1.4-0.1.5-windows-x86_64.exe.zst",
                            "sha256": "cafe",
                            "size_bytes": 11
                        }
                    ]
                }
            }
        }"#;
        let prev_path = dir.join("prev-update-manifest.json");
        std::fs::write(&prev_path, prev_json).unwrap();

        let manifest =
            generate_manifest(&dir, Some(&prev_path), Some("0.1.6")).unwrap();

        assert_eq!(manifest.latest_version, "0.1.6");
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.platforms.len(), 2);

        let windows = &manifest.platforms["x86_64-pc-windows-msvc"];
        assert_eq!(windows.full_asset_name, "pulsar-installer-windows-x86_64.exe");
        assert_eq!(
            windows.full_url,
            "https://github.com/Far-Beyond-Pulsar/Pulsar-Installer/releases/download/v0.1.6/pulsar-installer-windows-x86_64.exe"
        );
        assert_eq!(windows.full_sha256, hex::encode(Sha256::digest(b"binary-v2")));
        assert_eq!(windows.full_size_bytes, 9);
        assert_eq!(windows.patches.len(), 2);
        assert_eq!(windows.patches[0].from_version, "0.1.4");
        assert_eq!(windows.patches[0].url, "https://example.com/v0.1.5/pulsar-installer-patch-0.1.4-0.1.5-windows-x86_64.exe.zst");
        assert_eq!(windows.patches[1].to_version, "0.1.6");
        assert_eq!(
            windows.patches[1].url,
            "https://github.com/Far-Beyond-Pulsar/Pulsar-Installer/releases/download/v0.1.6/pulsar-installer-patch-0.1.5-0.1.6-windows-x86_64.exe.zst"
        );

        let linux = &manifest.platforms["x86_64-unknown-linux-gnu"];
        assert!(linux.patches.is_empty());

        let serialized = serde_json::to_string_pretty(&manifest).unwrap();
        let reparsed: UpdateManifest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reparsed.latest_version, "0.1.6");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_prev_platform_fails() {
        let dir = std::env::temp_dir().join("pulsar_gen_manifest_missing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("pulsar-installer-windows-x86_64.exe"), b"bin").unwrap();

        let prev_json = r#"{
            "schema_version": 1,
            "latest_version": "0.1.5",
            "generated_at": "2026-07-01T00:00:00Z",
            "platforms": {
                "aarch64-apple-darwin": {
                    "full_asset_name": "pulsar-installer-macos-arm64",
                    "full_url": "https://example.com/old",
                    "full_sha256": "deadbeef",
                    "full_size_bytes": 10,
                    "patches": []
                }
            }
        }"#;
        let prev_path = dir.join("prev.json");
        std::fs::write(&prev_path, prev_json).unwrap();

        let err = generate_manifest(&dir, Some(&prev_path), Some("0.1.6"))
            .expect_err("should fail on missing platform");
        assert!(err.to_string().contains("aarch64-apple-darwin"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sign_verify_round_trip() {
        let dir = std::env::temp_dir().join("pulsar_patch_sign_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let manifest_path = dir.join("update-manifest.json");
        fs::write(&manifest_path, br#"{"schema_version":1,"latest_version":"0.0.0"}"#).unwrap();

        let (seed_hex, pubkey_hex) = generate_keypair().unwrap();
        assert_eq!(seed_hex.len(), 64);
        assert_eq!(pubkey_hex.len(), 64);

        let sig_path = sign_file(&manifest_path, &seed_hex).unwrap();
        assert!(sig_path.ends_with("update-manifest.json.sig"));
        let sig_hex = fs::read_to_string(&sig_path).unwrap();
        assert_eq!(sig_hex.len(), 128);

        let sig_bytes = hex::decode(sig_hex.trim()).unwrap();
        let signature = ed25519_dalek::Signature::from_slice(&sig_bytes).unwrap();
        let verifying = VerifyingKey::from_bytes(
            &hex::decode(pubkey_hex).unwrap().as_slice().try_into().unwrap(),
        )
        .unwrap();
        verifying
            .verify(b"{\"schema_version\":1,\"latest_version\":\"0.0.0\"}", &signature)
            .expect("signature must verify");

        let mut tampered = b"{\"schema_version\":1,\"latest_version\":\"0.0.0\"}".to_vec();
        tampered[20] ^= 1;
        assert!(verifying.verify(&tampered, &signature).is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
