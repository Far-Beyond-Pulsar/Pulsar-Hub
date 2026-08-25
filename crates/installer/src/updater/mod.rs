pub mod chain;
pub mod downloader;
pub mod manifest;
pub mod platform;
pub mod replacer;

use std::time::Duration;

use anyhow::{Context, Result};

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

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn manifest_url() -> String {
    format!(
        "https://github.com/{}/{}/releases/latest/download/update-manifest.json",
        GITHUB_OWNER, GITHUB_REPO
    )
}

pub async fn check_and_update() -> Result<bool> {
    let version = current_version();
    tracing::info!("Current version: v{}", version);

    let client = update_client();

    tracing::info!("Fetching update manifest from {}", manifest_url());
    let manifest_text = client
        .get(manifest_url())
        .send()
        .await
        .context("failed to fetch update manifest")?
        .error_for_status()
        .context("update manifest HTTP error")?
        .text()
        .await
        .context("failed to read update manifest body")?;

    let manifest: manifest::UpdateManifest =
        serde_json::from_str(&manifest_text).context("failed to parse update manifest")?;

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
