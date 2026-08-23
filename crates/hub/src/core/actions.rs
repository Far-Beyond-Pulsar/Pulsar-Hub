//! Hub-wide actions bound to keyboard shortcuts.
//!
//! Bound in `installer/src/main.rs` with the `"Hub"` key context, handled on
//! the `EntryWindow` root element so they work wherever focus sits inside the
//! hub window.

use gpui::*;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, Action)]
#[action(namespace = hub, no_json)]
pub struct NewProjectModal;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, Action)]
#[action(namespace = hub, no_json)]
pub struct OpenFolderDialog;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, Action)]
#[action(namespace = hub, no_json)]
pub struct CloneGitModal;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, Action)]
#[action(namespace = hub, no_json)]
pub struct CheckGitUpdates;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, Action)]
#[action(namespace = hub, no_json)]
pub struct OpenAppSettings;
