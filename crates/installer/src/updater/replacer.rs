use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub struct SelfReplacer {
    current_exe: PathBuf,
}

impl SelfReplacer {
    pub fn new() -> Result<Self> {
        let current_exe =
            std::env::current_exe().context("failed to get current executable path")?;
        Ok(Self { current_exe })
    }

    pub fn replace(&self, new_binary: &Path) -> Result<()> {
        let backup_path = self.current_exe.with_extension("bak");

        tracing::info!(
            "Self-replace: {} <- {}",
            self.current_exe.display(),
            new_binary.display()
        );

        self.do_replace(new_binary, &backup_path)
            .context("self-replace failed")?;

        if backup_path.exists() {
            let _ = std::fs::remove_file(&backup_path);
        }

        Ok(())
    }

    fn do_replace(&self, new_binary: &Path, backup_path: &Path) -> Result<()> {
        if backup_path.exists() {
            let _ = std::fs::remove_file(backup_path);
        }

        std::fs::rename(&self.current_exe, backup_path)
            .context("failed to backup current binary")?;

        match std::fs::copy(new_binary, &self.current_exe) {
            Ok(_) => {
                tracing::info!("New binary installed successfully");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to install new binary: {}", e);
                if std::fs::copy(backup_path, &self.current_exe).is_ok() {
                    tracing::info!("Rolled back to backup binary");
                }
                Err(e).context("failed to install new binary (rolled back)")
            }
        }
    }
}
