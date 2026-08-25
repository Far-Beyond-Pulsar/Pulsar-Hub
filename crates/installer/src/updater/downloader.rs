use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::updater::chain::{UpdateChain, UpdateStep};
use crate::updater::manifest::PlatformUpdateInfo;

pub struct UpdateDownloader {
    temp_dir: PathBuf,
}

impl UpdateDownloader {
    pub fn new() -> Self {
        let temp_dir = std::env::temp_dir().join("pulsar-update");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);
        Self { temp_dir }
    }

    pub async fn apply_chain(
        &self,
        chain: &UpdateChain,
        _platform: &PlatformUpdateInfo,
    ) -> Result<PathBuf> {
        let mut current_file: Option<PathBuf> = None;

        for step in &chain.steps {
            match step {
                UpdateStep::Patch {
                    from_version,
                    to_version,
                    patch_info,
                } => {
                    tracing::info!("Applying patch {} -> {}", from_version, to_version);

                    let patch_path = self.temp_dir.join(&patch_info.asset_name);
                    self.download_file(&patch_info.url, &patch_path)
                        .await
                        .context("downloading patch")?;

                    let actual_hash = compute_file_hash(&patch_path)?;
                    if actual_hash != patch_info.sha256 {
                        anyhow::bail!(
                            "Patch SHA256 mismatch for {} -> {}: expected {}, got {}",
                            from_version,
                            to_version,
                            patch_info.sha256,
                            actual_hash
                        );
                    }

                    let old_file = current_file
                        .as_ref()
                        .context("no base binary for patch application")?;
                    let patched_file = self.temp_dir.join(format!(
                        "patched-{}{}",
                        to_version,
                        crate::updater::platform::platform_extension()
                    ));

                    let compressed = std::fs::read(&patch_path)
                        .context("reading patch file")?;
                    let uncompressed = zstd::decode_all(compressed.as_slice())
                        .context("zstd decompression failed")?;

                    let old = std::fs::read(old_file).context("reading base binary")?;
                    let mut new = Vec::with_capacity(old.len() + 1024);
                    bsdiff::patch(&old, &mut uncompressed.as_slice(), &mut new)
                        .context("bsdiff::patch failed")?;

                    std::fs::write(&patched_file, &new)
                        .context("writing patched binary")?;

                    current_file = Some(patched_file);
                }
                UpdateStep::FullDownload {
                    version,
                    url,
                    sha256,
                    size_bytes,
                } => {
                    tracing::info!(
                        "Full download for v{} ({} bytes)",
                        version,
                        size_bytes
                    );

                    let binary_name = format!(
                        "pulsar-installer{}",
                        crate::updater::platform::platform_extension()
                    );
                    let dest = self.temp_dir.join(&binary_name);
                    self.download_file(url, &dest)
                        .await
                        .context("downloading full binary")?;

                    let actual_hash = compute_file_hash(&dest)?;
                    if actual_hash != *sha256 {
                        anyhow::bail!(
                            "Binary SHA256 mismatch: expected {}, got {}",
                            sha256,
                            actual_hash
                        );
                    }

                    current_file = Some(dest);
                }
            }
        }

        current_file.context("update chain produced no output file")
    }

    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        tracing::info!("Downloading {} -> {}", url, dest.display());

        let mut response = reqwest::get(url)
            .await
            .context("HTTP request failed")?
            .error_for_status()
            .context("HTTP error response")?;

        let mut staged_name = dest
            .file_name()
            .context("invalid destination path")?
            .to_os_string();
        staged_name.push(".part");
        let part = dest.with_file_name(staged_name);

        let mut file = tokio::fs::File::create(&part)
            .await
            .with_context(|| format!("creating staging file {}", part.display()))?;
        while let Some(chunk) = response.chunk().await.context("reading response body")? {
            file.write_all(&chunk)
                .await
                .context("writing downloaded file")?;
        }
        file.flush().await.context("flushing downloaded file")?;
        drop(file);
        tokio::fs::rename(&part, dest)
            .await
            .context("finalizing downloaded file")?;

        Ok(())
    }
}

fn compute_file_hash(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}
