//! Shared blocking streaming-download utility.
//!
//! Single implementation of the GET → chunked-write-to-disk loop used by
//! every downloader in this crate. Chunks are staged into a `<dest>.part`
//! sibling and renamed over `dest` only on completion, so `dest` never
//! holds a truncated file. With `resume` enabled, an existing `.part`
//! file continues via HTTP Range when the server supports it.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const CHUNK_SIZE: usize = 64 * 1024;

pub struct StreamOptions<'a> {
    /// Called per chunk: cumulative bytes on disk, total size when known.
    pub progress: &'a dyn Fn(u64, Option<u64>),
    /// Polled between chunks; returning true aborts with a "Cancelled" error.
    pub cancelled: &'a dyn Fn() -> bool,
    /// Resume from an existing `<dest>.part` file via HTTP Range.
    pub resume: bool,
}

pub fn stream_to_file(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
    opts: &StreamOptions,
) -> Result<(), String> {
    let part = part_path(dest)?;
    let result = stream_to_part(client, url, dest, &part, opts);
    // Non-resuming downloads have no future use for a partial staging file.
    if result.is_err() && !opts.resume {
        let _ = std::fs::remove_file(&part);
    }
    result
}

fn stream_to_part(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
    part: &Path,
    opts: &StreamOptions,
) -> Result<(), String> {
    let mut requested =
        initial_offset(std::fs::metadata(part).ok().map(|m| m.len()), opts.resume);

    let mut response = send(client, url, requested)?;
    if requested > 0 && response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        // The stored bytes no longer line up with the remote resource.
        let _ = std::fs::remove_file(part);
        requested = 0;
        response = send(client, url, requested)?;
    }

    let (offset, total) = negotiate(
        requested,
        response.status().as_u16(),
        response.content_length(),
    );

    let mut file = if offset > 0 {
        std::fs::OpenOptions::new()
            .append(true)
            .open(part)
            .map_err(|e| e.to_string())?
    } else {
        std::fs::File::create(part).map_err(|e| e.to_string())?
    };

    let mut received: u64 = 0;
    let mut buffer = [0u8; CHUNK_SIZE];
    loop {
        if (opts.cancelled)() {
            return Err("Cancelled".to_string());
        }
        let read = Read::read(&mut response, &mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
        received += read as u64;
        (opts.progress)(offset + received, total);
    }

    drop(file);
    std::fs::rename(part, dest).map_err(|e| e.to_string())
}

fn send(
    client: &reqwest::blocking::Client,
    url: &str,
    offset: u64,
) -> Result<reqwest::blocking::Response, String> {
    let mut request = client.get(url);
    if offset > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={}-", offset));
    }
    request
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())
}

/// Sibling staging path (`<name>.part`) used while the download is in flight.
fn part_path(dest: &Path) -> Result<PathBuf, String> {
    let name = dest
        .file_name()
        .ok_or_else(|| "Invalid destination path".to_string())?;
    let mut staged = name.to_os_string();
    staged.push(".part");
    Ok(dest.with_file_name(staged))
}

/// Byte offset an existing `.part` file lets us resume from.
fn initial_offset(part_len: Option<u64>, resume: bool) -> u64 {
    if resume {
        part_len.unwrap_or(0)
    } else {
        0
    }
}

/// Resolve the server's reply into the on-disk offset to continue at plus the
/// effective total size. A `200` despite a Range request means the server
/// ignored it — restart from zero. On `206`, the content length covers only
/// the remaining bytes, so the total includes what is already on disk.
fn negotiate(requested_offset: u64, status: u16, content_length: Option<u64>) -> (u64, Option<u64>) {
    if requested_offset > 0 && status == 206 {
        (
            requested_offset,
            content_length.map(|len| requested_offset + len),
        )
    } else {
        (0, content_length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_path_appends_suffix() {
        assert_eq!(
            part_path(Path::new("/tmp/archives/pulsar-1.2.tar.gz")).unwrap(),
            Path::new("/tmp/archives/pulsar-1.2.tar.gz.part")
        );
        assert_eq!(
            part_path(Path::new("pulsar.exe")).unwrap(),
            Path::new("pulsar.exe.part")
        );
    }

    #[test]
    fn part_path_rejects_nameless_dest() {
        assert!(part_path(Path::new("/")).is_err());
    }

    #[test]
    fn initial_offset_only_when_resume_enabled() {
        assert_eq!(initial_offset(Some(100), true), 100);
        assert_eq!(initial_offset(Some(100), false), 0);
        assert_eq!(initial_offset(None, true), 0);
        assert_eq!(initial_offset(Some(0), true), 0);
    }

    #[test]
    fn negotiate_resumes_on_partial_content() {
        assert_eq!(negotiate(500, 206, Some(300)), (500, Some(800)));
        assert_eq!(negotiate(500, 206, None), (500, None));
    }

    #[test]
    fn negotiate_restarts_when_range_unsupported() {
        assert_eq!(negotiate(500, 200, Some(1_000)), (0, Some(1_000)));
        assert_eq!(negotiate(500, 200, None), (0, None));
    }

    #[test]
    fn negotiate_starts_fresh_without_range_request() {
        assert_eq!(negotiate(0, 200, Some(1_000)), (0, Some(1_000)));
        assert_eq!(negotiate(0, 206, Some(1_000)), (0, Some(1_000)));
    }
}
