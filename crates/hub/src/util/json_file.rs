//! JSON files read and written through the engine's virtual file system, so a
//! remote or P2P mount works for the Hub's own data files too.

use std::path::Path;

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let content = engine_fs::virtual_fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(Into::into)
}

/// Pretty-printed, creating parent directories.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        engine_fs::virtual_fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)?;
    engine_fs::virtual_fs::write_file(path, json.as_bytes())?;
    Ok(())
}
