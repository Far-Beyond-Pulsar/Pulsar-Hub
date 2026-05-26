//! Versions-manager page — card grid of installed engines with launch / remove actions.

use gpui::{
    Context, FontWeight, IntoElement, InteractiveElement as _,
    ParentElement, StatefulInteractiveElement as _, Styled, px,
};
use gpui_component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
    Icon, IconName,
};
use gpui::prelude::FluentBuilder;
use crate::installed_versions::InstalledVersion;
use super::super::{InstallerView, Page};

impl InstallerView {
    pub(crate) fn render_versions_manager(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let confirm_idx = self.uninstall_confirm;

        v_flex()
            .size_full()
            // ── Header bar ────────────────────────────────────────────────────
            .child(
                h_flex()
                    .w_full()
                    .px_4()
                    .py_3()
                    .justify_between()
                    .items_center()
                    .bg(cx.theme().sidebar)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex_shrink_0()
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                gpui::div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Installed Engines"),
                            )
                            .when(!self.installed_versions.is_empty(), |el| {
                                el.child(
                                    gpui::div()
                                        .px_2()
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(cx.theme().accent.opacity(0.15))
                                        .text_xs()
                                        .text_color(cx.theme().accent)
                                        .child(format!("{}", self.installed_versions.len())),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("vm-refresh-btn")
                                    .ghost()
                                    .label("Refresh")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.load_installed_versions(cx);
                                    })),
                            )
                            .child(
                                Button::new("vm-new-install-btn")
                                    .primary()
                                    .label("+ New Install")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.navigate_to(Page::License, window, cx);
                                        this.fetch_license(cx);
                                    })),
                            ),
                    ),
            )
            // ── Content ───────────────────────────────────────────────────────
            .child(
                gpui::div()
                    .flex_1()
                    .overflow_hidden()
                    .relative()
                    .map(|el| {
                        if self.loading_installed {
                            el.child(self.render_loading_state("Scanning for installed engines…", cx))
                        } else if self.installed_versions.is_empty() {
                            el.child(self.render_vm_empty(cx))
                        } else {
                            el.child(self.render_vm_list(cx))
                        }
                    })
                    // Uninstall confirmation modal
                    .when_some(confirm_idx, |el, idx| {
                        el.child(self.render_uninstall_confirm(idx, cx))
                    }),
            )
    }

    // ─── Empty state ──────────────────────────────────────────────────────────

    fn render_vm_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .p_8()
            .child(
                Icon::new(IconName::Inbox)
                    .with_size(px(48.0))
                    .text_color(cx.theme().muted_foreground.opacity(0.4)),
            )
            .child(
                v_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        gpui::div()
                            .text_base()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().muted_foreground)
                            .child("No installations found"),
                    )
                    .child(
                        gpui::div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground.opacity(0.6))
                            .child("Install Pulsar to manage versions here."),
                    ),
            )
            .child(
                Button::new("vm-install-cta")
                    .primary()
                    .label("Install Engine →")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate_to(Page::License, window, cx);
                        this.fetch_license(cx);
                    })),
            )
    }

    // ─── Version card grid ────────────────────────────────────────────────────

    fn render_vm_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        // Snapshot the versions list and all theme colors we'll need so the
        // iterator closure below doesn't need to borrow `self` or `cx`.
        let versions: Vec<InstalledVersion> = self.installed_versions.clone();

        // Theme tokens (all Copy).
        let border          = cx.theme().border;
        let sidebar         = cx.theme().sidebar;
        let foreground      = cx.theme().foreground;
        let muted           = cx.theme().muted_foreground;
        let accent          = cx.theme().accent;
        let warning         = cx.theme().warning;

        // Pre-build per-card listeners so we don't borrow cx inside the map closure.
        let listeners: Vec<_> = versions
            .iter()
            .enumerate()
            .map(|(idx, ver)| {
                let path = ver.metadata.install_path.clone();
                let path_open   = path.clone();
                let path_launch = path.clone();
                (
                    cx.listener(move |this, _, _, cx| {
                        this.launch_version_path(path_launch.clone(), cx);
                    }),
                    cx.listener(move |this, _, _, cx| {
                        this.open_folder_path(path_open.clone(), cx);
                    }),
                    cx.listener(move |this, _, _, cx| {
                        this.uninstall_confirm = Some(idx);
                        cx.notify();
                    }),
                )
            })
            .collect();

        gpui::div()
            .id("vm-grid-scroll")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .child(
                gpui::div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .children(
                        versions
                            .into_iter()
                            .zip(listeners)
                            .enumerate()
                            .map(|(idx, (ver, (on_launch, on_open, on_remove)))| {
                                let size_str    = InstallerView::format_bytes(ver.disk_size_bytes);
                                let version_str = ver.metadata.version.clone();
                                let date_str    = if ver.metadata.install_date.is_empty() {
                                    "Unknown date".to_string()
                                } else {
                                    ver.metadata.install_date.chars().take(10).collect()
                                };
                                let path_label = ver.metadata.install_path.display().to_string();
                                let has_update = ver.update_available;

                                v_flex()
                                    .w(px(280.0))
                                    .p_4()
                                    .gap_3()
                                    .rounded(px(10.0))
                                    .border_1()
                                    .border_color(border)
                                    .bg(sidebar.opacity(0.5))
                                    // ── Engine icon + version ─────────────────
                                    .child(
                                        h_flex()
                                            .gap_3()
                                            .items_center()
                                            .child(
                                                gpui::div()
                                                    .w(px(36.0))
                                                    .h(px(36.0))
                                                    .rounded(px(8.0))
                                                    .bg(accent.opacity(0.12))
                                                    .border_1()
                                                    .border_color(accent.opacity(0.3))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        Icon::new(IconName::HardDrive)
                                                            .with_size(px(18.0))
                                                            .text_color(accent),
                                                    ),
                                            )
                                            .child(
                                                v_flex()
                                                    .gap(px(1.0))
                                                    .child(
                                                        gpui::div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(foreground)
                                                            .child("Pulsar Engine"),
                                                    )
                                                    .child(
                                                        gpui::div()
                                                            .px_2()
                                                            .py(px(1.0))
                                                            .rounded(px(4.0))
                                                            .bg(accent.opacity(0.1))
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(accent)
                                                            .child(version_str),
                                                    ),
                                            )
                                            .when(has_update, |e| {
                                                e.child(
                                                    gpui::div()
                                                        .ml_auto()
                                                        .px_2()
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .bg(warning.opacity(0.15))
                                                        .text_xs()
                                                        .text_color(warning)
                                                        .child("Update"),
                                                )
                                            }),
                                    )
                                    // ── Path + metadata ───────────────────────
                                    .child(
                                        v_flex()
                                            .gap(px(2.0))
                                            .child(
                                                gpui::div()
                                                    .text_xs()
                                                    .text_color(muted)
                                                    .overflow_hidden()
                                                    .child(path_label),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .child(
                                                        gpui::div()
                                                            .text_xs()
                                                            .text_color(muted.opacity(0.6))
                                                            .child(size_str),
                                                    )
                                                    .child(
                                                        gpui::div()
                                                            .text_xs()
                                                            .text_color(muted.opacity(0.4))
                                                            .child("·"),
                                                    )
                                                    .child(
                                                        gpui::div()
                                                            .text_xs()
                                                            .text_color(muted.opacity(0.6))
                                                            .child(date_str),
                                                    ),
                                            ),
                                    )
                                    // Divider
                                    .child(gpui::div().w_full().h(px(1.0)).bg(border))
                                    // ── Actions ───────────────────────────────
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .child(
                                                Button::new(format!("vm-launch-{idx}"))
                                                    .primary()
                                                    .small()
                                                    .label("Launch")
                                                    .on_click(on_launch),
                                            )
                                            .child(
                                                Button::new(format!("vm-open-{idx}"))
                                                    .ghost()
                                                    .small()
                                                    .label("Show in Finder")
                                                    .on_click(on_open),
                                            )
                                            .child(
                                                Button::new(format!("vm-uninstall-{idx}"))
                                                    .danger()
                                                    .small()
                                                    .label("Remove")
                                                    .on_click(on_remove),
                                            ),
                                    )
                            }),
                    ),
            )
    }

    // ─── Uninstall confirmation modal ─────────────────────────────────────────

    fn render_uninstall_confirm(&self, idx: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let version = self
            .installed_versions
            .get(idx)
            .map(|v| v.metadata.version.clone())
            .unwrap_or_else(|| "this version".to_string());

        // Semi-transparent backdrop
        gpui::div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.5))
            .child(
                v_flex()
                    .w(px(360.0))
                    .p_6()
                    .gap_4()
                    .rounded(px(12.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                gpui::div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Confirm Uninstall"),
                            )
                            .child(
                                gpui::div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "Are you sure you want to remove {}? \
                                         This will permanently delete all files.",
                                        version
                                    )),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .justify_end()
                            .child(
                                Button::new("confirm-cancel-btn")
                                    .outline()
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.uninstall_confirm = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("confirm-uninstall-btn")
                                    .danger()
                                    .label("Uninstall")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.uninstall_version(idx, cx);
                                    })),
                            ),
                    ),
            )
    }
}
