//! Installer view — root struct, constructors, Focusable/Render, and layout shells.

mod logic;
mod pages;
mod types;

// Re-export the whole types namespace so submodules can `use super::super::*`.
pub use types::*;

use gpui::{
    App, AppContext as _, Context, Entity, Focusable, FontWeight, IntoElement,
    ParentElement, Render, Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
    Icon, IconName,
};
use gpui::prelude::FluentBuilder;
use crate::InstallerConfig;
use crate::installed_versions::InstalledVersion;
use std::path::PathBuf;

// ─── View state ───────────────────────────────────────────────────────────────

pub struct InstallerView {
    pub focus_handle: gpui::FocusHandle,
    pub current_page: Page,

    // ── License ──────────────────────────────────────────────────────────────
    pub license_text: Option<String>,
    pub loading_license: bool,
    pub license_fetch_error: Option<String>,
    pub license_accepted: bool,

    // ── Version selection ────────────────────────────────────────────────────
    pub releases: Vec<ReleaseInfo>,
    pub loading_releases: bool,
    pub loading_more: bool,
    pub current_releases_page: u32,
    pub has_more_releases: bool,
    pub show_prereleases: bool,
    /// Index into `releases` of the currently selected release.
    pub selected_release_idx: Option<usize>,
    pub selected_asset_size: Option<u64>,

    // ── Install options ──────────────────────────────────────────────────────
    pub install_config: InstallerConfig,

    // ── Installing ───────────────────────────────────────────────────────────
    pub install_progress: f32,
    pub install_message: String,
    pub log_entries: Vec<LogEntry>,
    pub install_failed: bool,
    /// Set on successful installation.
    pub installed_path: Option<PathBuf>,

    // ── Versions manager ─────────────────────────────────────────────────────
    pub installed_versions: Vec<InstalledVersion>,
    pub loading_installed: bool,
    /// Index into `installed_versions` awaiting uninstall confirmation.
    pub uninstall_confirm: Option<usize>,

    /// IDs of optional sidecar packages to co-install (e.g. "pulsar-host").
    pub selected_sidecars: Vec<String>,
}

// ─── Constructor ──────────────────────────────────────────────────────────────

impl InstallerView {
    pub fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|cx| Self::new(cx));
        entity.update(cx, |this, cx| {
            this.load_installed_versions(cx);
        });
        entity
    }

    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            current_page: Page::VersionsManager,
            // License
            license_text: None,
            loading_license: false,
            license_fetch_error: None,
            license_accepted: false,
            // Version selection
            releases: Vec::new(),
            loading_releases: false,
            loading_more: false,
            current_releases_page: 0,
            has_more_releases: true,
            show_prereleases: false,
            selected_release_idx: None,
            selected_asset_size: None,
            // Install options
            install_config: Self::default_install_config(),
            // Installing
            install_progress: 0.0,
            install_message: String::new(),
            log_entries: Vec::new(),
            install_failed: false,
            installed_path: None,
            // Versions manager
            installed_versions: Vec::new(),
            loading_installed: true,
            uninstall_confirm: None,
            selected_sidecars: Vec::new(),
        }
    }
}

// ─── Focusable / Render ───────────────────────────────────────────────────────

impl Focusable for InstallerView {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InstallerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().background)
            .flex()
            .flex_col()
            .child(self.render_title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_sidebar(cx))
                    .child(self.render_main_content(window, cx)),
            )
    }
}

// ─── Layout regions ───────────────────────────────────────────────────────────

impl InstallerView {
    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .h(px(36.0))
            .bg(cx.theme().sidebar)
            .border_b_1()
            .border_color(cx.theme().border)
            .pl(px(80.0))
            .items_center()
            .justify_between()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("Pulsar Installer"),
                    ),
            )
            .child(
                div()
                    .pr_4()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.current_page.label()),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_manager = self.current_page == Page::VersionsManager;
        let current_wizard_idx = self.current_page.wizard_index();

        v_flex()
            .w(px(220.0))
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .flex_shrink_0()
            // ── Logo area ──────────────────────────────────────────────────────
            .child(
                v_flex()
                    .px_4()
                    .py_5()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .rounded(px(8.0))
                                    .bg(cx.theme().secondary)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().secondary_foreground)
                                            .child("P"),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_0()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("Pulsar Engine"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Installer"),
                                    ),
                            ),
                    ),
            )
            // ── Navigation ────────────────────────────────────────────────────
            .child(
                v_flex()
                    .flex_1()
                    .px_3()
                    .py_4()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted_foreground)
                            .px_2()
                            .pb_2()
                            .child(if is_manager { "MANAGEMENT" } else { "INSTALLATION STEPS" }),
                    )
                    // Wizard step list
                    .when(!is_manager, |el| {
                        el.children(WIZARD_STEPS.iter().enumerate().map(|(idx, &step)| {
                            let is_active = current_wizard_idx == Some(idx);
                            let is_done   = current_wizard_idx.map_or(false, |cur| idx < cur);

                            h_flex()
                                .gap_3()
                                .px_2()
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .items_center()
                                .when(is_active, |e| e.bg(cx.theme().secondary))
                                // Step bubble
                                .child(
                                    div()
                                        .w(px(22.0))
                                        .h(px(22.0))
                                        .rounded_full()
                                        .flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .justify_center()
                                        .when(is_done, |e| {
                                            e.bg(cx.theme().secondary).child(
                                                Icon::new(IconName::Check)
                                                    .with_size(px(12.0))
                                                    .text_color(cx.theme().secondary),
                                            )
                                        })
                                        .when(is_active, |e| {
                                            e.bg(cx.theme().secondary).child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(cx.theme().secondary_foreground)
                                                    .child(format!("{}", idx + 1)),
                                            )
                                        })
                                        .when(!is_active && !is_done, |e| {
                                            e.border_1()
                                                .border_color(cx.theme().border)
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(format!("{}", idx + 1)),
                                                )
                                        }),
                                )
                                // Step label
                                .child(
                                    h_flex()
                                        .flex_1()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Icon::new(step.icon())
                                                .with_size(px(14.0))
                                                .text_color(if is_active {
                                                    cx.theme().secondary
                                                } else if is_done {
                                                    cx.theme().secondary
                                                } else {
                                                    cx.theme().muted_foreground
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(if is_active {
                                                    FontWeight::SEMIBOLD
                                                } else {
                                                    FontWeight::NORMAL
                                                })
                                                .text_color(if is_active {
                                                    cx.theme().foreground
                                                } else if is_done {
                                                    cx.theme().muted_foreground
                                                } else {
                                                    cx.theme().muted_foreground
                                                })
                                                .child(step.label()),
                                        ),
                                )
                        }))
                    })
                    // Versions-manager active item
                    .when(is_manager, |el| {
                        el.child(
                            h_flex()
                                .gap_3()
                                .px_2()
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .items_center()
                                .bg(cx.theme().secondary)
                                .child(
                                    Icon::new(IconName::Inbox)
                                        .with_size(px(14.0))
                                    .text_color(cx.theme().secondary),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().foreground)
                                        .child("Installed Versions"),
                                ),
                        )
                    }),
            )
            // ── Footer ────────────────────────────────────────────────────────
            .child(
                v_flex()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .gap_2()
                    // Context-switch link (Install New ↔ Manage Installed)
                    .when(self.current_page != Page::Installing, |el| {
                        el.child(if is_manager {
                            Button::new("sidebar-switch-install-btn")
                                .ghost()
                                .small()
                                .label("← Install New")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.navigate_to(Page::Welcome, window, cx);
                                }))
                        } else {
                            Button::new("sidebar-switch-manager-btn")
                                .ghost()
                                .small()
                                .label("Manage Installed ▸")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.navigate_to(Page::VersionsManager, window, cx);
                                    this.load_installed_versions(cx);
                                }))
                        })
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Far Beyond Pulsar"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
                    ),
            )
    }

    fn render_main_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .map(|el| match self.current_page {
                Page::Welcome          => el.child(self.render_welcome(cx)),
                Page::License          => el.child(self.render_license(cx)),
                Page::VersionSelection => el.child(self.render_version_selection(cx)),
                Page::ReleaseNotes     => el.child(self.render_release_notes(window, cx)),
                Page::InstallOptions   => el.child(self.render_install_options(cx)),
                Page::Installing       => el.child(self.render_installing(cx)),
                Page::Complete         => el.child(self.render_complete(cx)),
                Page::VersionsManager  => el.child(self.render_versions_manager(cx)),
            })
    }
}
