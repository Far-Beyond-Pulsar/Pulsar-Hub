use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

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
    bsdiff::diff(&old, &new, &mut uncompressed_patch)
        .context("bsdiff::diff failed")?;

    let compressed = zstd::encode_all(uncompressed_patch.as_slice(), 19)
        .context("zstd compression failed")?;

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

    let uncompressed_patch = zstd::decode_all(compressed.as_slice())
        .context("zstd decompression failed")?;

    let mut new = Vec::with_capacity(old.len() + 1024);
    bsdiff::patch(&old, &mut uncompressed_patch.as_slice(), &mut new)
        .context("bsdiff::patch failed")?;

    std::fs::write(new_path, &new)
        .with_context(|| format!("writing result: {}", new_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join("pulsar_patch_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let old_path = dir.join("old.bin");
        let new_path = dir.join("new.bin");
        let patch_path = dir.join("patch.zst");
        let result_path = dir.join("result.bin");

        let old_data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
        let mut new_data = old_data.clone();
        new_data[100] = 42;
        new_data[5000] = 99;
        new_data.extend_from_slice(&[1, 2, 3, 4, 5]);

        fs::write(&old_path, &old_data).unwrap();
        fs::write(&new_path, &new_data).unwrap();

        let hash = generate_patch(&old_path, &new_path, &patch_path).unwrap();
        assert_eq!(hash.len(), 64);

        apply_patch(&old_path, &patch_path, &result_path).unwrap();

        let result = fs::read(&result_path).unwrap();
        assert_eq!(result, new_data);

        let _ = fs::remove_dir_all(&dir);
    }
}
