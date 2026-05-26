//! Installer UI module.
//!
//! The UI is split into focused sub-modules:
//!   - `installer_view/mod.rs`    — root struct, Focusable/Render, layout shells
//!   - `installer_view/types.rs`  — Page enum, data structs, constants
//!   - `installer_view/logic/`    — async tasks: license, releases, install, etc.
//!   - `installer_view/pages/`    — render implementations, one file per page

mod installer_view;

pub use installer_view::InstallerView;
