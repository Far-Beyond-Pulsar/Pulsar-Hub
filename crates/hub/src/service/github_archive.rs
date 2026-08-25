//! HTTPS archive fallback for GitHub repository clones.
//!
//! When the native git transport (libgit2 → WinHTTP) cannot reach GitHub —
//! broken machine proxy state, WinHTTP issues, TLS interception — we fetch a
//! plain `tar.gz` snapshot over our own self-contained reqwest/rustls stack
//! instead. The snapshot is extracted, turned into a fresh git repository
//! with an `origin` remote, so downstream flows (template remotes, upstream
//! prompts) behave exactly like a real clone, minus history.

use std::fs::File;
use std::path::{Path, PathBuf};

use crate::core::types::SharedCloneProgress;

/// Owner/name (+ optional ref) parsed out of a GitHub repository URL.
struct GitHubRepoRef {
    owner: String,
    name: String,
    requested_ref: Option<String>,
}

/// Parse `https://github.com/{owner}/{repo}[.git][/tree/{ref}]`.
fn parse_github_url(url: &str) -> Option<GitHubRepoRef> {
    let trimmed = url.trim().trim_end_matches('/');
    let rest = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("git@github.com:"))?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);

    let mut segments = rest.split('/');
    let owner = segments.next()?.trim().to_string();
    let name = segments.next()?.trim().to_string();
    if owner.is_empty() || name.is_empty() {
        return None;
    }

    let requested_ref = match (segments.next(), segments.next()) {
        (Some("tree"), Some(first)) => {
            let mut full = first.to_string();
            for extra in segments {
                full.push('/');
                full.push_str(extra);
            }
            Some(full)
        }
        _ => None,
    };

    Some(GitHubRepoRef {
        owner,
        name,
        requested_ref,
    })
}

/// Candidate codeload archive URLs, most likely first.
fn archive_urls(r: &GitHubRepoRef) -> Vec<String> {
    let base = format!(
        "https://codeload.github.com/{}/{}",
        r.owner,
        r.name.trim_end_matches('/')
    );
    match &r.requested_ref {
        None => vec![format!("{}/tar.gz/HEAD", base)],
        Some(reference) => vec![
            format!("{}/tar.gz/refs/heads/{}", base, reference),
            format!("{}/tar.gz/refs/tags/{}", base, reference),
            format!("{}/tar.gz/{}", base, reference),
        ],
    }
}

/// Download a repo snapshot over HTTPS and materialize it at `target` as a
/// freshly-initialized git repository whose `origin` points back at
/// `repo_url`. Returns a human-readable error string on failure.
pub fn download_repo_snapshot(
    repo_url: &str,
    target: &Path,
    progress: &SharedCloneProgress,
) -> Result<(), String> {
    let reference = parse_github_url(repo_url)
        .ok_or_else(|| "Not a GitHub URL — HTTPS archive fallback unavailable".to_string())?;

    let parent = target
        .parent()
        .ok_or_else(|| "Invalid destination path".to_string())?;
    let staging = parent.join(format!(
        ".pulsar-archive-{}",
        std::process::id()
    ));
    let archive_path = staging.with_extension("tar.gz");
    let _ = std::fs::create_dir_all(parent);

    let result = (|| -> Result<(), String> {
        let client = reqwest_client::apply_bundled_tls_blocking(
            reqwest::blocking::Client::builder(),
        )
        .connect_timeout(std::time::Duration::from_secs(10))
        .user_agent("Pulsar-Hub/1.0")
        .build()
        .map_err(|e| e.to_string())?;

        let mut last_error = String::from("no archive URL attempted");
        let mut downloaded = false;
        for url in archive_urls(&reference) {
            if progress.lock().cancelled {
                return Err("Cancelled".to_string());
            }
            progress.lock().message = format!("Downloading archive: {}", url);
            match download_archive(&client, &url, &archive_path, progress) {
                Ok(()) => {
                    downloaded = true;
                    break;
                }
                Err(e) => {
                    last_error = e;
                    let _ = std::fs::remove_file(&archive_path);
                }
            }
        }
        if !downloaded {
            return Err(last_error);
        }

        progress.lock().message = "Extracting archive...".to_string();
        let _ = std::fs::create_dir_all(&staging);
        extract_tar_gz(&archive_path, &staging)?;
        move_extracted_contents(&staging, target)?;

        let repo = git2::Repository::init(target)
            .map_err(|e| format!("Failed to initialize repository: {}", e))?;
        repo.remote("origin", repo_url)
            .map_err(|e| format!("Failed to set origin remote: {}", e))?;
        Ok(())
    })();

    let _ = std::fs::remove_file(&archive_path);
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// Download via the shared streaming utility, mapping byte progress onto the
/// clone-progress channel and honoring its cancellation flag.
fn download_archive(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
    progress: &SharedCloneProgress,
) -> Result<(), String> {
    {
        let mut p = progress.lock();
        p.current = 0;
        p.total = 0;
    }
    let report = |received: u64, total: Option<u64>| {
        let mut p = progress.lock();
        p.current = received.min(usize::MAX as u64) as usize;
        p.total = total.unwrap_or(0) as usize;
        p.message = format!("Downloading archive: {:.1} MB", received as f32 / 1_048_576.0);
    };
    let options = crate::service::download::StreamOptions {
        progress: &report,
        cancelled: &|| progress.lock().cancelled,
        resume: false,
    };
    crate::service::download::stream_to_file(client, url, dest, &options)
}

fn extract_tar_gz(archive_path: &Path, staging: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|e| e.to_string())?;
    let decompressed = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decompressed);
    archive.set_preserve_permissions(false);
    archive.unpack(staging).map_err(|e| e.to_string())
}

/// GitHub archives unpack into a single `{repo}-{sha}` directory; move its
/// contents up into `target`.
fn move_extracted_contents(staging: &Path, target: &Path) -> Result<(), String> {
    let root = std::fs::read_dir(staging)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .find(|entry| entry.path().is_dir())
        .map(|entry| entry.path())
        .ok_or_else(|| "Archive contained no top-level directory".to_string())?;

    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
    {
        let dest = target.join(entry.file_name());
        if std::fs::rename(entry.path(), &dest).is_err() {
            copy_recursive(&entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn copy_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(src)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
        {
            copy_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(src, dst)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_repo_url() {
        let r = parse_github_url("https://github.com/foo/bar.git").unwrap();
        assert_eq!(r.owner, "foo");
        assert_eq!(r.name, "bar");
        assert!(r.requested_ref.is_none());
    }

    #[test]
    fn parses_tree_ref() {
        let r = parse_github_url("https://github.com/foo/bar/tree/release-1.0").unwrap();
        assert_eq!(r.requested_ref.as_deref(), Some("release-1.0"));
    }

    #[test]
    fn rejects_non_github() {
        assert!(parse_github_url("https://gitlab.com/foo/bar").is_none());
    }

    #[test]
    fn builds_head_url_when_no_ref() {
        let r = parse_github_url("https://github.com/foo/bar").unwrap();
        assert_eq!(
            archive_urls(&r)[0],
            "https://codeload.github.com/foo/bar/tar.gz/HEAD"
        );
    }

    #[test]
    fn builds_branch_then_tag_urls() {
        let r = parse_github_url("https://github.com/foo/bar/tree/v2").unwrap();
        let urls = archive_urls(&r);
        assert!(urls[0].ends_with("/refs/heads/v2"));
        assert!(urls[1].ends_with("/refs/tags/v2"));
    }
}
