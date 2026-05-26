//! # Pulsar Installer
//!
//! A modern, cross-platform installer framework for the Pulsar game engine.
//!
//! ## Features
//!
//! - **Cross-Platform**: Works seamlessly on Windows, macOS, and Linux
//! - **Modular Architecture**: Trait-based design for easy customization
//! - **Beautiful UI**: Built with GPUI component library
//! - **Progress Tracking**: Real-time installation progress with detailed feedback
//! - **Async Operations**: Non-blocking downloads and installation
//! - **Verification**: Checksum validation for downloaded files
//! - **Rollback Support**: Automatic cleanup on installation failure
//! - **OS-Native Installation**: Follows platform conventions for each OS
//!
//! ## Architecture
//!
//! The installer is built around several core traits:
//!
//! - [`InstallStep`]: Defines individual installation steps
//! - [`SystemDetector`]: Detects system information and requirements
//! - [`DownloadManager`]: Handles file downloads with progress tracking
//! - [`ComponentInstaller`]: Installs individual components
//! - [`ConfigManager`]: Manages installation configuration
//!
//! ## Platform-Specific Installation
//!
//! ### Windows
//! - Install location: `%LOCALAPPDATA%\Programs\Pulsar`
//! - Start Menu shortcut creation
//! - Add/Remove Programs registration via registry
//! - Proper uninstall metadata
//!
//! ### macOS
//! - Creates valid .app bundle with Info.plist
//! - Install location: `~/Applications/Pulsar.app` (user) or `/Applications/Pulsar.app` (system)
//! - Launch Services handles registration automatically
//!
//! ### Linux
//! - Binary: `~/.local/bin/pulsar` (user) or `/usr/bin/pulsar` (system)
//! - Desktop entry: `~/.local/share/applications/pulsar.desktop`
//! - Icons: `~/.local/share/icons/hicolor/<size>/apps/`
//! - Follows freedesktop.org specifications

pub mod traits;
pub mod platform;
pub mod download;
pub mod config;
pub mod ui;
pub mod error;
pub mod installed_versions;

pub use traits::*;
pub use config::InstallerConfig;
pub use error::{InstallerError, Result};