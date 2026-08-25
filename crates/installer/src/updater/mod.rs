pub mod chain;
pub mod downloader;
pub mod manifest;
pub mod platform;
pub mod replacer;

use std::time::Duration;

use anyhow::{Context, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

const GITHUB_OWNER: &str = "Far-Beyond-Pulsar";
const GITHUB_REPO: &str = "Pulsar-Installer";

pub use chain::MAX_PATCH_CHAIN_LENGTH;

fn update_client() -> reqwest::Client {
    reqwest_client::apply_bundled_tls(
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120)),
    )
    .build()
    .expect("failed to build update HTTP client")
}

// Placeholder pinned ed25519 public key for update-manifest signatures.
// Provisioned with `pulsar-patch-tool gen-keypair`; the matching seed is held
// as the UPDATE_SIGNING_KEY_SEED CI secret. Rotate both together.
// Local testing override: set PULSAR_UPDATE_PUBKEY (hex) to replace this key.
const MANIFEST_SIGNING_PUBKEY: &str =
    "b21da5b200b66883a14d2c11b5f49a4664b8517e67a8d6078df02bca8fb017bc";

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn manifest_url() -> String {
    format!(
        "https://github.com/{}/{}/releases/latest/download/update-manifest.json",
        GITHUB_OWNER, GITHUB_REPO
    )
}

fn manifest_verifying_key() -> Result<VerifyingKey> {
    let hex_key = std::env::var("PULSAR_UPDATE_PUBKEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| MANIFEST_SIGNING_PUBKEY.to_string());
    let bytes = hex::decode(hex_key.trim()).context("manifest public key is not valid hex")?;
    let bytes: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("manifest public key must be 32 bytes"))?;
    VerifyingKey::from_bytes(&bytes).context("invalid ed25519 manifest public key")
}

async fn fetch_manifest_signature(client: &reqwest::Client) -> Result<Vec<u8>> {
    let sig_url = format!("{}.sig", manifest_url());
    let sig_hex = client
        .get(&sig_url)
        .send()
        .await
        .context("failed to fetch update manifest signature")?
        .error_for_status()
        .context("update manifest signature not published (missing .sig)")?
        .text()
        .await
        .context("failed to read update manifest signature body")?;
    hex::decode(sig_hex.trim()).context("update manifest signature is not valid hex")
}

fn verify_manifest_signature(manifest_bytes: &[u8], sig_bytes: &[u8]) -> Result<()> {
    let verifying_key = manifest_verifying_key()?;
    let signature = Signature::from_slice(sig_bytes)
        .context("update manifest signature has invalid length (expected 64 bytes)")?;
    verifying_key
        .verify(manifest_bytes, &signature)
        .context("update manifest signature verification failed (tampered or signed by unknown key)")
}

pub async fn check_and_update() -> Result<bool> {
    let version = current_version();
    tracing::info!("Current version: v{}", version);

    let client = update_client();

    tracing::info!("Fetching update manifest from {}", manifest_url());
    let manifest_bytes = client
        .get(manifest_url())
        .send()
        .await
        .context("failed to fetch update manifest")?
        .error_for_status()
        .context("update manifest HTTP error")?
        .bytes()
        .await
        .context("failed to read update manifest body")?;

    if let Err(e) = verify_manifest_signature(
        &manifest_bytes,
        &fetch_manifest_signature(&client).await?,
    ) {
        tracing::error!("Rejecting update manifest: {:#}", e);
        return Err(e);
    }

    let manifest: manifest::UpdateManifest =
        serde_json::from_slice(&manifest_bytes).context("failed to parse update manifest")?;

    if manifest.schema_version != 1 {
        tracing::warn!(
            "Unknown manifest schema version: {}",
            manifest.schema_version
        );
    }

    if manifest.latest_version == version {
        tracing::info!("Already on latest version (v{})", version);
        return Ok(false);
    }

    tracing::info!(
        "Update available: v{} -> v{}",
        version,
        manifest.latest_version
    );

    let target_triple = platform::current_target_triple();
    let platform_info = manifest.platforms.get(target_triple).with_context(|| {
        format!(
            "no update info for platform '{}' in manifest",
            target_triple
        )
    })?;

    let update_chain =
        chain::compute_update_chain(version, &manifest.latest_version, platform_info);

    tracing::info!(
        "Update chain: {} steps, full_download={}, total_bytes={}",
        update_chain.steps.len(),
        update_chain.has_full_download,
        update_chain.total_download_bytes
    );

    let downloader = downloader::UpdateDownloader::new(client);
    let new_binary_path = downloader
        .apply_chain(&update_chain, platform_info)
        .await
        .context("failed to apply update chain")?;

    let replacer = replacer::SelfReplacer::new().context("failed to initialize self-replacer")?;
    replacer
        .replace(&new_binary_path)
        .context("failed to replace binary")?;

    tracing::info!("Update to v{} applied successfully", manifest.latest_version);
    Ok(true)
}
