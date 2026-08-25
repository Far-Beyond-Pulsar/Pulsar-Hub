use std::path::Path;

use anyhow::{Context, Result};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
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
