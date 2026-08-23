//! Disk-usage measurement for the Storage page.
//!
//! Walks a project directory once, classifying `.git` separately from
//! working files so the UI can visualize repository bloat.

use std::path::Path;

#[derive(Clone, Debug)]
pub struct ProjectDiskStats {
    pub path: String,
    /// Bytes outside `.git`.
    pub working_bytes: u64,
    /// Bytes inside `.git` (history, objects, packs).
    pub git_bytes: u64,
}

impl ProjectDiskStats {
    pub fn total_bytes(&self) -> u64 {
        self.working_bytes + self.git_bytes
    }

    /// Fraction (0.0–1.0) of the project that is `.git` history.
    pub fn git_ratio(&self) -> f32 {
        let total = self.total_bytes();
        if total == 0 {
            0.0
        } else {
            self.git_bytes as f32 / total as f32
        }
    }
}

/// Measure a single project directory synchronously (blocking). Call from a
/// background executor.
pub fn measure_project(path: &Path) -> ProjectDiskStats {
    let mut stats = ProjectDiskStats {
        path: path.to_string_lossy().to_string(),
        working_bytes: 0,
        git_bytes: 0,
    };

    for entry in walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        // A file belongs to `.git` when any component of its relative path is
        // exactly ".git" (covers nested worktrees' .git/worktrees/... too).
        let in_git = entry
            .path()
            .strip_prefix(path)
            .map(|rel| rel.components().any(|c| c.as_os_str() == ".git"))
            .unwrap_or(false);
        if in_git {
            stats.git_bytes += size;
        } else {
            stats.working_bytes += size;
        }
    }

    stats
}

/// Repo-health label derived from how much of the project is `.git` history.
/// Mirrors the heuristic of the old project-settings "Performance" tab.
pub fn repo_health(stats: &ProjectDiskStats) -> (&'static str, &'static str) {
    let ratio = stats.git_ratio();
    if stats.total_bytes() == 0 {
        ("Empty", "muted")
    } else if ratio < 0.2 {
        ("Healthy", "success")
    } else if ratio < 0.5 {
        ("Large history", "warning")
    } else {
        ("Bloaty", "danger")
    }
}
