//! # Pulsar Installer
//!
//! Cross-platform GPUI installer/launcher for the Pulsar game engine.
//!
//! ## Entry flow
//!
//! The binary starts by running the self-update check
//! ([`updater::check_and_update`]): it fetches `update-manifest.json` from the
//! latest GitHub release, computes a patch chain from the running version to
//! the latest, applies it, and self-replaces the binary before relaunching
//! itself with `--updated`. It then opens the GPUI hub window via
//! [`pulsar_hub::EntryWindow`].
//!
//! ## Updater module
//!
//! - [`updater::manifest`] — `UpdateManifest` schema (`schema_version`,
//!   `latest_version`, per-platform [`updater::manifest::PlatformUpdateInfo`]
//!   with full-binary URL/hash/size and [`updater::manifest::PatchInfo`]
//!   entries)
//! - [`updater::chain`] — computes the update path between versions using a
//!   chain of bsdiff patches (zstd-compressed), capped at
//!   [`updater::chain::MAX_PATCH_CHAIN_LENGTH`] hops before falling back to a
//!   full download
//! - [`updater::downloader`] — downloads each step and verifies SHA256 hashes
//! - [`updater::replacer`] — atomic self-replace of the running executable
//!
//! ## Hub split
//!
//! Installation itself does not live in this crate. The UI and all real
//! install logic live in the `pulsar-hub` crate (`crates/hub`), whose services
//! perform the work: `installer_service`, `dependency_service`,
//! `template_cache_service`, and `plugin_service`.

pub mod updater;
