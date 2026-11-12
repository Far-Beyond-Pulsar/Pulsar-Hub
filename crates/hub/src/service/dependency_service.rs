//! Rust and C/C++ build tool detection and setup.
//!
//! Detection asks the tools themselves instead of guessing from `PATH`:
//!
//! * Rust is found on `PATH` *and* in `CARGO_HOME/bin` (a Hub started from
//!   Explorer, Finder or a desktop launcher does not inherit a shell's `PATH`),
//!   and must actually run (`rustc -vV` succeeds), which also tells us the host
//!   triple (x86_64, aarch64, msvc, gnu, ...).
//! * Build tools are verified by building and linking a trivial program with
//!   that `rustc`. Whatever linker the toolchain needs, MSVC found through
//!   `vswhere` or the registry, `cc`, MinGW, is then right by construction.
//!   The platform checks only name the tools and say what to install.
//!
//! Setup never removes anything. An existing rustup gets `rustup toolchain
//! install stable`; a missing one gets `rustup-init` for this host, downloaded
//! over HTTPS and verified against the published SHA-256, run for the current
//! user only (no elevation).

use crate::core::types::{DependencyStatus, InstallProgress, InstallStatus};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct DependencyService;

impl DependencyService {
    /// Check Rust and the build tools. Runs `rustc` and links a test program,
    /// so it takes a moment: call it off the UI thread.
    pub fn check() -> DependencyStatus {
        let rustup = find_tool("rustup");
        let scratch = std::env::temp_dir();
        // With rustup, ask it: `rustup which rustc` is the real compiler of the
        // active toolchain, wherever it lives, with no PATH guessing.
        let rustup_rustc = rustup
            .as_ref()
            .and_then(|rustup| run(rustup, &["which", "rustc"], &scratch, Duration::from_secs(20)))
            .filter(|out| out.ok)
            .map(|out| PathBuf::from(first_line(&out.stdout)))
            .filter(|path| path.is_file());
        let rustc = rustup_rustc.or_else(|| find_tool("rustc"));

        let mut status = DependencyStatus {
            rustup_installed: rustup.is_some(),
            toolchain: rustup
                .as_ref()
                .and_then(|rustup| {
                    run(rustup, &["show", "active-toolchain"], &scratch, Duration::from_secs(20))
                })
                .filter(|out| out.ok)
                .map(|out| first_line(&out.stdout))
                .and_then(|line| parse_active_toolchain(&line)),
            ..Default::default()
        };

        // ── Rust ──
        let mut rust_problem: Option<String> = None;
        if let Some(rustup) = &rustup {
            status.rust_details.push(("rustup".into(), path_and_version(rustup, "--version")));
        }
        if let Some(rustc) = &rustc {
            status.rust_details.push(("rustc".into(), rustc.display().to_string()));
        }
        if let Some(cargo) = find_tool("cargo") {
            status.rust_details.push(("cargo".into(), path_and_version(&cargo, "--version")));
        }
        if let Some(home) = cargo_bin_dir().and_then(|bin| bin.parent().map(Path::to_path_buf)) {
            status.rust_details.push(("CARGO_HOME".into(), home.display().to_string()));
        }
        if let Some(rustc) = &rustc {
            match run(rustc, &["-vV"], &scratch, Duration::from_secs(30)) {
                Some(out) if out.ok => {
                    if let Some(info) = parse_rustc_vv(&out.stdout) {
                        status.rust_installed = true;
                        status.rust_version = Some(info.release.clone());
                        status.host_triple = info.host.clone();
                        status.rust_details.insert(
                            0,
                            ("Version".into(), format!("rustc {}", info.release)),
                        );
                        if let Some(host) = &info.host {
                            status.rust_details.insert(1, ("Host".into(), host.clone()));
                        }
                        if let Some(toolchain) = &status.toolchain {
                            status.rust_details.insert(2, ("Toolchain".into(), toolchain.clone()));
                        }
                        status.rust_note = Some(format!(
                            "rustc {}{}{}",
                            info.release,
                            info.host
                                .as_ref()
                                .map(|host| format!(" for {host}"))
                                .unwrap_or_default(),
                            match (&status.toolchain, status.rustup_installed) {
                                (Some(toolchain), _) => format!(", toolchain {toolchain}"),
                                (None, true) => String::new(),
                                (None, false) =>
                                    " (no rustup: installed by a package manager or by hand)"
                                        .to_string(),
                            }
                        ));
                    }
                }
                Some(out) => rust_problem = Some(first_line(&out.stderr)),
                None => rust_problem = Some("it did not respond".to_string()),
            }
        }
        if !status.rust_installed {
            status.needs_default_toolchain = status.rustup_installed;
            status.rust_note = Some(match (&rustc, status.rustup_installed, rust_problem) {
                (Some(path), true, Some(problem)) => format!(
                    "rustup is installed but rustc ({}) did not run: {problem}. A default \
                     toolchain is probably not set.",
                    path.display()
                ),
                (Some(path), false, Some(problem)) => {
                    format!("rustc was found at {} but did not run: {problem}", path.display())
                }
                (_, true, _) => "rustup is installed but has no usable default toolchain.".into(),
                _ => format!(
                    "Not found. Looked on PATH and in {}.",
                    cargo_bin_dir()
                        .map(|dir| dir.display().to_string())
                        .unwrap_or_else(|| "the cargo bin directory".into())
                ),
            });
        }

        if let Some(rustup) = &rustup {
            let listed = run(rustup, &["toolchain", "list"], &scratch, Duration::from_secs(20))
                .filter(|out| out.ok)
                .map(|out| {
                    out.stdout
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|list| !list.is_empty());
            if let Some(list) = listed {
                status.rust_details.push(("Installed toolchains".into(), list));
            }
        }

        // ── Build tools ──
        let host = status.host_triple.clone();
        let (label, platform_details) = platform_toolchain(host.as_deref());
        status.tools_details = platform_details;
        match (&rustc, status.rust_installed) {
            (Some(rustc), true) => match {
                let started = Instant::now();
                let result = link_test(rustc);
                status.tools_details.push((
                    "Test program".into(),
                    format!(
                        "{} in {:.1} s",
                        if result.is_ok() { "built and linked" } else { "failed to build" },
                        started.elapsed().as_secs_f32()
                    ),
                ));
                result
            } {
                Ok(()) => {
                    status.build_tools_installed = true;
                    status.link_verified = true;
                    status.compiler_info = Some(match label {
                        Some(label) => format!("{label}: builds and links"),
                        None => "Builds and links".to_string(),
                    });
                }
                Err(problem) => {
                    status.build_tools_installed = false;
                    status.compiler_info = label;
                    status.build_tools_problem = Some(problem);
                }
            },
            _ => {
                // No working rustc to ask: fall back to what we can see.
                status.build_tools_installed = label.is_some();
                status.compiler_info = label;
            }
        }
        if !status.build_tools_installed {
            status.build_tools_hint = Some(build_tools_hint(host.as_deref()));
        }
        status
    }

    /// Install or repair Rust without removing anything already there.
    pub fn install_rust(progress: Arc<Mutex<InstallProgress>>) -> Result<(), String> {
        let log = |line: &str, fraction: f32, status: Option<InstallStatus>| {
            let mut p = progress.lock().unwrap();
            p.logs.push(line.to_string());
            p.progress = fraction;
            if let Some(status) = status {
                p.status = status;
            }
        };
        let scratch = std::env::temp_dir();

        if let Some(rustup) = find_tool("rustup") {
            log(
                &format!("Using the existing rustup at {}", rustup.display()),
                0.05,
                Some(InstallStatus::Installing),
            );
            for (args, from, to) in [
                (&["toolchain", "install", "stable", "--profile", "minimal"][..], 0.1, 0.8),
                (&["default", "stable"][..], 0.8, 0.95),
            ] {
                log(&format!("rustup {}", args.join(" ")), from, None);
                let out = run(&rustup, args, &scratch, Duration::from_secs(30 * 60))
                    .ok_or_else(|| "rustup did not start".to_string())?;
                if !out.ok {
                    return Err(format!(
                        "rustup {} failed: {}",
                        args.join(" "),
                        tail(&out.stderr, 6)
                    ));
                }
                log(&tail(&out.stderr, 3), to, None);
            }
            log("Rust is ready.", 1.0, Some(InstallStatus::Complete));
            return Ok(());
        }

        // No rustup: fetch and verify rustup-init for this host.
        if is_alpine() {
            return Err(
                "Alpine Linux uses musl, which the official rustup-init does not support. \
                 Install Rust with `apk add rustup` (then `rustup-init`) or `apk add rust cargo`."
                    .into(),
            );
        }
        let triple = rustup_triple(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| {
            format!(
                "There is no rustup installer for {} {}. See https://rustup.rs",
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        })?;
        let exe = if cfg!(windows) { ".exe" } else { "" };
        let url = format!("https://static.rust-lang.org/rustup/dist/{triple}/rustup-init{exe}");
        log(
            &format!("Downloading rustup-init for {triple}..."),
            0.1,
            Some(InstallStatus::Downloading),
        );
        let client =
            reqwest_client::apply_bundled_tls_blocking(reqwest::blocking::Client::builder())
                .timeout(Duration::from_secs(300))
                .build()
                .map_err(|e| e.to_string())?;
        let bytes = client
            .get(&url)
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.bytes())
            .map_err(|e| format!("Could not download rustup-init: {e}"))?;
        let expected = client
            .get(format!("{url}.sha256"))
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.text())
            .map_err(|e| format!("Could not download the installer's checksum: {e}"))?;
        let expected = parse_sha256_file(&expected)
            .ok_or_else(|| "The installer's checksum file is not a SHA-256".to_string())?;
        let actual = hex::encode(Sha256::digest(&bytes));
        if !actual.eq_ignore_ascii_case(&expected) {
            return Err(format!(
                "The downloaded rustup-init does not match its published SHA-256 \
                 (expected {expected}, got {actual}); not running it."
            ));
        }
        log(
            &format!("Downloaded {} bytes; checksum verified.", bytes.len()),
            0.3,
            None,
        );

        let dir = std::env::temp_dir().join(format!("pulsar-hub-rustup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let installer = dir.join(format!("rustup-init{exe}"));
        std::fs::write(&installer, &bytes).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&installer, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        log(
            "Running rustup-init for the current user (stable, minimal profile)...",
            0.4,
            Some(InstallStatus::Installing),
        );
        let out = run(
            &installer,
            &["-y", "--default-toolchain", "stable", "--profile", "minimal"],
            &dir,
            Duration::from_secs(30 * 60),
        );
        let _ = std::fs::remove_dir_all(&dir);
        let out = out.ok_or_else(|| "rustup-init did not start".to_string())?;
        if !out.ok {
            return Err(format!("rustup-init failed: {}", tail(&out.stderr, 6)));
        }
        log(&tail(&out.stderr, 3), 0.95, None);
        log(
            "Rust is installed. New terminals pick it up; the Hub looks in ~/.cargo/bin itself.",
            1.0,
            Some(InstallStatus::Complete),
        );
        Ok(())
    }
}

// ── Finding and running tools ─────────────────────────────────────────────

fn home_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}

fn cargo_bin_dir() -> Option<PathBuf> {
    std::env::var_os("CARGO_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".cargo")))
        .map(|dir| dir.join("bin"))
}

/// `PATH` plus the places a GUI app does not inherit from a shell.
fn search_path() -> OsString {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    dirs.extend(cargo_bin_dir());
    if cfg!(target_os = "macos") {
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    }
    if cfg!(unix) {
        dirs.extend(home_dir().map(|home| home.join(".local").join("bin")));
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

fn find_tool(name: &str) -> Option<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
    which::which_in(name, Some(search_path()), cwd).ok()
}

struct Output {
    ok: bool,
    stdout: String,
    stderr: String,
}

/// Run a program with the widened `PATH`, no console window, in `cwd` (a
/// directory with no `rust-toolchain` file to surprise rustup), killing it
/// after `timeout`. `None` if it could not start or timed out.
fn run(program: &Path, args: &[&str], cwd: &Path, timeout: Duration) -> Option<Output> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .env("PATH", search_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().ok()?;
    let read = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut text);
            }
            String::from_utf8_lossy(&text).into_owned()
        })
    };
    let stdout = read(child.stdout.take().map(|p| Box::new(p) as _));
    let stderr = read(child.stderr.take().map(|p| Box::new(p) as _));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let (stdout, stderr) = (stdout.join().unwrap_or_default(), stderr.join().unwrap_or_default());
    status.map(|status| Output { ok: status.success(), stdout, stderr })
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no output")
        .to_string()
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    all[all.len().saturating_sub(lines)..].join(" | ")
}

// ── Rust ──────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
struct RustcInfo {
    release: String,
    host: Option<String>,
}

/// The `release:` and `host:` lines of `rustc -vV`.
fn parse_rustc_vv(text: &str) -> Option<RustcInfo> {
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    Some(RustcInfo { release: field("release:")?, host: field("host:") })
}

/// The toolchain name in the first line of `rustup show active-toolchain`
/// ("stable-x86_64-pc-windows-msvc (default)"); `None` for "no active toolchain".
fn parse_active_toolchain(line: &str) -> Option<String> {
    let name = line.split_whitespace().next()?;
    (!line.to_lowercase().contains("no active toolchain") && !name.eq_ignore_ascii_case("error:"))
        .then(|| name.to_string())
}

/// A trivial program, built and linked with `rustc`: the authoritative test
/// that this machine can build Rust code.
fn link_test(rustc: &Path) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("pulsar-hub-linktest-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let source = dir.join("main.rs");
    let binary = dir.join(if cfg!(windows) { "linktest.exe" } else { "linktest" });
    let result = std::fs::write(&source, "fn main() {}\n")
        .map_err(|e| e.to_string())
        .and_then(|()| {
            run(
                rustc,
                &[
                    "--edition",
                    "2021",
                    "-o",
                    &binary.to_string_lossy(),
                    &source.to_string_lossy(),
                ],
                &dir,
                Duration::from_secs(120),
            )
            .ok_or_else(|| "rustc did not finish building a test program".to_string())
        });
    let _ = std::fs::remove_dir_all(&dir);
    let out = result?;
    if out.ok {
        Ok(())
    } else {
        Err(tail(&out.stderr, 5))
    }
}

// ── Build tools ───────────────────────────────────────────────────────────

/// What toolchain this machine appears to have, for display, with the facts
/// behind it. Not the test of whether it works: that is [`link_test`].
fn platform_toolchain(host: Option<&str>) -> (Option<String>, Vec<(String, String)>) {
    let mut details = Vec::new();
    if cfg!(windows) {
        if host.is_some_and(|host| host.ends_with("-gnu")) {
            let version = which_version("gcc", &["--version"]);
            if let Some(gcc) = find_tool("gcc") {
                details.push(("gcc".into(), gcc.display().to_string()));
            }
            return (version.map(|v| format!("MinGW-w64 {v}")), details);
        }
        return match visual_studio(host) {
            Some(vs) => {
                details.push(("Installed at".into(), vs.path.clone()));
                if let Some(msvc) = &vs.msvc {
                    details.push(("MSVC tools".into(), msvc.clone()));
                }
                (
                    Some(match &vs.msvc {
                        Some(msvc) => format!("{} (MSVC {msvc})", vs.name),
                        None => vs.name.clone(),
                    }),
                    details,
                )
            }
            None => (None, details),
        };
    }
    if cfg!(target_os = "macos") {
        let found = run(
            Path::new("/usr/bin/xcode-select"),
            &["-p"],
            &std::env::temp_dir(),
            Duration::from_secs(10),
        )
        .filter(|out| out.ok && !out.stdout.trim().is_empty());
        return match found {
            Some(out) => {
                details.push(("Developer directory".into(), out.stdout.trim().to_string()));
                (Some("Xcode command line tools".to_string()), details)
            }
            None => (None, details),
        };
    }
    // Linux and others: rustc links through `cc` by default.
    for tool in ["cc", "gcc", "clang"] {
        if let Some(version) = which_version(tool, &["--version"]) {
            if let Some(path) = find_tool(tool) {
                details.push((tool.to_string(), path.display().to_string()));
            }
            return (Some(format!("{tool} {version}")), details);
        }
    }
    (None, details)
}

/// `path` and the first line of `path <flag>`.
fn path_and_version(path: &Path, flag: &str) -> String {
    match run(path, &[flag], &std::env::temp_dir(), Duration::from_secs(15))
        .filter(|out| out.ok)
        .map(|out| first_line(&out.stdout))
    {
        Some(version) => format!("{version}  ({})", path.display()),
        None => path.display().to_string(),
    }
}

fn which_version(tool: &str, args: &[&str]) -> Option<String> {
    let path = find_tool(tool)?;
    let out = run(&path, args, &std::env::temp_dir(), Duration::from_secs(10))?;
    out.ok.then(|| first_line(&out.stdout))
}

struct VisualStudio {
    name: String,
    path: String,
    msvc: Option<String>,
}

/// The newest Visual Studio (or Build Tools) with the C++ tools for this
/// host's architecture, from `vswhere`, which the VS installer places at a
/// fixed path. `cl.exe` is rarely on `PATH`; rustc finds the same install.
#[cfg(windows)]
fn visual_studio(host: Option<&str>) -> Option<VisualStudio> {
    let program_files = std::env::var_os("ProgramFiles(x86)").or_else(|| std::env::var_os("ProgramFiles"))?;
    let vswhere = PathBuf::from(program_files)
        .join("Microsoft Visual Studio")
        .join("Installer")
        .join("vswhere.exe");
    if !vswhere.is_file() {
        return None;
    }
    let arm = host.map_or(cfg!(target_arch = "aarch64"), |host| host.starts_with("aarch64"));
    let component = if arm {
        "Microsoft.VisualStudio.Component.VC.Tools.ARM64"
    } else {
        "Microsoft.VisualStudio.Component.VC.Tools.x86.x64"
    };
    let ask = |property: &str| -> Option<String> {
        let out = run(
            &vswhere,
            &["-latest", "-products", "*", "-requires", component, "-property", property],
            &std::env::temp_dir(),
            Duration::from_secs(20),
        )?;
        out.ok.then(|| first_line(&out.stdout)).filter(|line| line != "no output")
    };
    let path = ask("installationPath")?;
    let name = ask("displayName").unwrap_or_else(|| "Visual Studio".to_string());
    let msvc = std::fs::read_dir(PathBuf::from(&path).join("VC").join("Tools").join("MSVC"))
        .ok()
        .and_then(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .max()
        });
    Some(VisualStudio { name, path, msvc })
}

#[cfg(not(windows))]
fn visual_studio(_host: Option<&str>) -> Option<VisualStudio> {
    None
}

/// What to install when the build tools are missing or fail to link.
fn build_tools_hint(host: Option<&str>) -> String {
    if cfg!(windows) {
        let arm = host.map_or(cfg!(target_arch = "aarch64"), |host| host.starts_with("aarch64"));
        return format!(
            "Install the Visual Studio Build Tools with the \"Desktop development with C++\" \
             workload{}. From a terminal: winget install Microsoft.VisualStudio.2022.BuildTools \
             --override \"--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools \
             --includeRecommended{}\". Reopen the Hub afterwards.",
            if arm { " and the \"MSVC ARM64 build tools\" component" } else { "" },
            if arm { " --add Microsoft.VisualStudio.Component.VC.Tools.ARM64" } else { "" },
        );
    }
    if cfg!(target_os = "macos") {
        return "Install the Xcode command line tools: xcode-select --install".to_string();
    }
    let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    linux_install_hint(&os_release)
}

/// The command that installs a C toolchain on the distribution described by
/// `/etc/os-release`.
fn linux_install_hint(os_release: &str) -> String {
    let ids = os_release_ids(os_release);
    let is = |names: &[&str]| ids.iter().any(|id| names.contains(&id.as_str()));
    let command = if is(&["debian", "ubuntu", "linuxmint", "pop", "raspbian"]) {
        "sudo apt install build-essential"
    } else if is(&["fedora", "rhel", "centos", "rocky", "almalinux"]) {
        "sudo dnf install gcc gcc-c++ make"
    } else if is(&["arch", "manjaro", "endeavouros"]) {
        "sudo pacman -S base-devel"
    } else if is(&["opensuse", "opensuse-leap", "opensuse-tumbleweed", "suse", "sles"]) {
        "sudo zypper install -t pattern devel_basis"
    } else if is(&["alpine"]) {
        "sudo apk add build-base"
    } else if is(&["nixos"]) {
        "nix-shell -p gcc (or add gcc to your configuration)"
    } else {
        return "Install a C compiler and linker (gcc or clang with make) from your \
                distribution's package manager."
            .to_string();
    };
    format!("Install a C toolchain: {command}")
}

/// `ID` and `ID_LIKE` of `/etc/os-release`, lower-cased.
fn os_release_ids(os_release: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in os_release.lines() {
        for key in ["ID=", "ID_LIKE="] {
            if let Some(value) = line.strip_prefix(key) {
                ids.extend(
                    value
                        .trim_matches(|c| c == '"' || c == '\'')
                        .split_whitespace()
                        .map(str::to_lowercase),
                );
            }
        }
    }
    ids
}

fn is_alpine() -> bool {
    cfg!(target_os = "linux")
        && os_release_ids(&std::fs::read_to_string("/etc/os-release").unwrap_or_default())
            .iter()
            .any(|id| id == "alpine")
}

// ── rustup-init ───────────────────────────────────────────────────────────

/// The host triple rustup publishes an installer for.
fn rustup_triple(os: &str, arch: &str) -> Option<&'static str> {
    Some(match (os, arch) {
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86") => "i686-pc-windows-msvc",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        _ => return None,
    })
}

/// The hash of a `<sha256>  <file name>` checksum file.
fn parse_sha256_file(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    (token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| token.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rustc_vv_gives_release_and_host() {
        let text = "rustc 1.91.0 (f8297e351 2025-10-28)\nbinary: rustc\nhost: aarch64-pc-windows-msvc\nrelease: 1.91.0\nLLVM version: 21.1.2\n";
        assert_eq!(
            parse_rustc_vv(text),
            Some(RustcInfo {
                release: "1.91.0".into(),
                host: Some("aarch64-pc-windows-msvc".into())
            })
        );
        assert_eq!(parse_rustc_vv("error: no default toolchain"), None);
    }

    #[test]
    fn the_active_toolchain_is_the_first_word_unless_there_is_none() {
        assert_eq!(
            parse_active_toolchain("stable-aarch64-pc-windows-msvc (default)"),
            Some("stable-aarch64-pc-windows-msvc".into())
        );
        assert_eq!(parse_active_toolchain("error: no active toolchain configured"), None);
        assert_eq!(parse_active_toolchain("no active toolchain"), None);
    }

    #[test]
    fn a_rustup_installer_exists_for_each_supported_host_and_no_other() {
        assert_eq!(rustup_triple("windows", "aarch64"), Some("aarch64-pc-windows-msvc"));
        assert_eq!(rustup_triple("macos", "aarch64"), Some("aarch64-apple-darwin"));
        assert_eq!(rustup_triple("linux", "aarch64"), Some("aarch64-unknown-linux-gnu"));
        assert_eq!(rustup_triple("freebsd", "x86_64"), None);
    }

    #[test]
    fn checksum_files_are_read_and_validated() {
        let hash = "a".repeat(64);
        assert_eq!(parse_sha256_file(&format!("{hash} *rustup-init.exe\n")), Some(hash.clone()));
        assert_eq!(parse_sha256_file("<html>not found</html>"), None);
        assert_eq!(parse_sha256_file(&"g".repeat(64)), None);
    }

    #[test]
    fn linux_hints_follow_the_distribution_family() {
        let ubuntu = "NAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\n";
        assert!(linux_install_hint(ubuntu).contains("apt install build-essential"));
        let rocky = "ID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n";
        assert!(linux_install_hint(rocky).contains("dnf"));
        assert!(linux_install_hint("ID=endeavouros\nID_LIKE=arch\n").contains("base-devel"));
        assert!(linux_install_hint("ID=nixos\n").contains("nix-shell"));
        assert!(linux_install_hint("ID=plan9\n").contains("package manager"));
        assert!(linux_install_hint("").contains("package manager"));
    }

    #[test]
    fn the_search_path_includes_the_cargo_bin_directory() {
        let dirs: Vec<PathBuf> = std::env::split_paths(&search_path()).collect();
        assert!(dirs.iter().any(|dir| dir.ends_with("bin") && dir.to_string_lossy().contains("cargo")));
    }

    #[test]
    fn a_missing_program_is_none_not_a_panic() {
        assert!(run(
            Path::new("definitely-not-a-real-program-xyz"),
            &[],
            &std::env::temp_dir(),
            Duration::from_secs(2)
        )
        .is_none());
    }

    /// Runs the real detection on this machine: `cargo test -- --ignored --nocapture live`.
    #[test]
    #[ignore]
    fn live_check_prints_what_this_machine_has() {
        println!("{:#?}", DependencyService::check());
    }
}
