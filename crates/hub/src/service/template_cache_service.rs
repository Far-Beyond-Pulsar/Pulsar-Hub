use std::path::{Path, PathBuf};

use crate::core::types::SharedCloneProgress;
use crate::service::git_service::GitService;

/// Local on-disk cache for project templates.
///
/// Templates are cloned once into `<appdata>/TemplateCache/<entry>`; new
/// projects are then instantiated by copying the cached checkout, so repeated
/// creation from the same template is fast and works offline.
pub struct TemplateCacheService;

impl TemplateCacheService {
    pub fn cache_dir() -> PathBuf {
        crate::util::path_helpers::template_cache_dir()
    }

    /// Deterministic per-URL directory inside the cache.
    pub fn entry_dir(repo_url: &str) -> PathBuf {
        Self::cache_dir().join(entry_name(repo_url))
    }

    pub fn is_cached(repo_url: &str) -> bool {
        Self::entry_dir(repo_url).join(".git").exists()
    }

    /// Clone a template repository into the cache (no-op if already cached).
    pub fn clone_to_cache(
        repo_url: &str,
        progress: SharedCloneProgress,
    ) -> Result<git2::Repository, git2::Error> {
        let entry = Self::entry_dir(repo_url);
        if entry.join(".git").exists() {
            return git2::Repository::open(&entry);
        }
        let _ = std::fs::create_dir_all(Self::cache_dir());
        // Clean up any partial download left behind by a previous failure.
        force_remove_dir_all(&entry);
        GitService::clone_repository(repo_url.to_string(), entry, progress)
    }

    pub fn fetch_tracking_snapshot(
        repo_url: &str,
    ) -> Result<ui_git_manager::AutoFetchOutcome, git2::Error> {
        ui_git_manager::fetch_tracking_snapshot(&Self::entry_dir(repo_url))
    }

    pub fn pull_cached(repo_url: &str) -> Result<(), git2::Error> {
        ui_git_manager::pull_from_remote(&Self::entry_dir(repo_url), None)
    }

    pub fn remove_cached(repo_url: &str) -> std::io::Result<()> {
        let entry = Self::entry_dir(repo_url);
        if entry.exists() {
            force_remove_dir_all(&entry)
        } else {
            Ok(())
        }
    }

    /// Create a new project from a cached template by copying the whole
    /// checkout (history included). The copy's `origin` remote is renamed to
    /// `template` (push-disabled) so the user's own fork can become `origin`.
    pub fn instantiate_project(
        cache_entry: &Path,
        target: &Path,
        template_url: &str,
    ) -> std::io::Result<()> {
        std::fs::create_dir_all(target)?;
        copy_dir_recursive(cache_entry, target)?;
        if let Err(error) = GitService::setup_template_remotes(target, template_url) {
            tracing::warn!(
                "Failed to set up template remote for {}: {}",
                target.display(),
                error
            );
        }
        Ok(())
    }
}

/// Stable FNV-1a hash so cache entries survive Rust toolchain changes.
fn fnv1a(data: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn entry_name(repo_url: &str) -> String {
    let slug: String = repo_url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("template")
        .trim_end_matches(".git")
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let slug = if slug.is_empty() { "template" } else { &slug };
    format!("{}-{:08x}", slug, fnv1a(repo_url) as u32)
}

/// `std::fs::remove_dir_all` fails on Windows when files are read-only, and
/// git packfiles/objects are read-only — clear attributes while walking.
fn force_remove_dir_all(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        if entry.file_type()?.is_dir() {
            force_remove_dir_all(&p)?;
        } else {
            #[cfg(windows)]
            {
                // Windows-only path: clearing FILE_ATTRIBUTE_READONLY so
                // read-only git objects can be deleted (the clippy lint about
                // Unix world-writable files does not apply).
                #[allow(clippy::permissions_set_readonly_false)]
                if let Ok(meta) = std::fs::metadata(&p) {
                    let mut perms = meta.permissions();
                    if perms.readonly() {
                        perms.set_readonly(false);
                        let _ = std::fs::set_permissions(&p, perms);
                    }
                }
            }
            std::fs::remove_file(&p)?;
        }
    }
    std::fs::remove_dir(path)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), dest)?;
        } else if file_type.is_symlink() {
            #[cfg(unix)]
            {
                let _ = std::os::unix::fs::symlink(std::fs::read_link(entry.path())?, dest);
            }
            #[cfg(not(unix))]
            {
                let _ = dest;
            }
        }
    }
    Ok(())
}
