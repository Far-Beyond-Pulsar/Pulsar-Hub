#[cfg(target_os = "windows")]
pub fn current_target_triple() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    { "x86_64-pc-windows-msvc" }
    #[cfg(target_arch = "aarch64")]
    { "aarch64-pc-windows-msvc" }
    #[cfg(target_arch = "x86")]
    { "i686-pc-windows-msvc" }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86")))]
    { "x86_64-pc-windows-msvc" }
}

#[cfg(target_os = "macos")]
pub fn current_target_triple() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    { "aarch64-apple-darwin" }
    #[cfg(target_arch = "x86_64")]
    { "x86_64-apple-darwin" }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    { "aarch64-apple-darwin" }
}

#[cfg(target_os = "linux")]
pub fn current_target_triple() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    { "x86_64-unknown-linux-gnu" }
    #[cfg(target_arch = "aarch64")]
    { "aarch64-unknown-linux-gnu" }
    #[cfg(target_arch = "x86")]
    { "i686-unknown-linux-gnu" }
    #[cfg(target_arch = "arm")]
    { "armv7-unknown-linux-gnueabihf" }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86", target_arch = "arm")))]
    { "x86_64-unknown-linux-gnu" }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn current_target_triple() -> &'static str {
    "x86_64-unknown-linux-gnu"
}

pub fn platform_extension() -> &'static str {
    if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    }
}

#[cfg(target_os = "windows")]
pub fn platform_asset_suffix() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    { "-windows-x86_64.exe" }
    #[cfg(target_arch = "aarch64")]
    { "-windows-arm64.exe" }
    #[cfg(target_arch = "x86")]
    { "-windows-i686.exe" }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86")))]
    { "-windows-x86_64.exe" }
}

#[cfg(target_os = "macos")]
pub fn platform_asset_suffix() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    { "-macos-arm64" }
    #[cfg(target_arch = "x86_64")]
    { "-macos-x86_64" }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    { "-macos-arm64" }
}

#[cfg(target_os = "linux")]
pub fn platform_asset_suffix() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    { "-linux-x86_64" }
    #[cfg(target_arch = "aarch64")]
    { "-linux-arm64" }
    #[cfg(target_arch = "x86")]
    { "-linux-i686" }
    #[cfg(target_arch = "arm")]
    { "-linux-armv7" }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86", target_arch = "arm")))]
    { "-linux-x86_64" }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn platform_asset_suffix() -> &'static str {
    "-linux-x86_64"
}

pub fn package_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}
