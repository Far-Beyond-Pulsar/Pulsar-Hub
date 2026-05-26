//! Version-selection page — scrollable list of GitHub releases with pagination.

use gpui::{
    Context, FontWeight, IntoElement, InteractiveElement as _, MouseButton,
    ParentElement, Styled, px, StatefulInteractiveElement,
};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, v_flex,
    Disableable as _,
    Icon, IconName,
    Sizable,
};
use gpui::prelude::FluentBuilder;
use super::super::{InstallerView, Page, ReleaseInfo};

impl InstallerView {
    pub(crate) fn render_version_selection(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_idx  = self.selected_release_idx;
        let show_pre      = self.show_prereleases;
        let has_selection = selected_idx.is_some();

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Select Version", None, cx))
            // ── Filter bar ────────────────────────────────────────────────────
            .child(
                h_flex()
                    .px_6()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar.opacity(0.4))
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::Info)
                                    .with_size(px(13.0))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                gpui::div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Select a release to install. Architecture is detected automatically."),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Checkbox::new("show-pre-checkbox")
                                    .checked(show_pre)
                                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                        this.show_prereleases = *checked;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                gpui::div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Show pre-releases"),
                            ),
                    ),
            )
            // ── Release list (or loading / empty state) ───────────────────────
            .child(
                gpui::div()
                    .flex_1()
                    .overflow_hidden()
                    .map(|el| {
                        if self.loading_releases {
                            el.child(self.render_loading_state("Fetching releases from GitHub…", cx))
                        } else if self.releases.is_empty() {
                            el.child(self.render_empty_releases(cx))
                        } else {
                            el.child(self.render_release_list(cx))
                        }
                    }),
            )
            // ── Action bar ────────────────────────────────────────────────────
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .items_center()
                    .child(
                        Button::new("vs-back-btn")
                            .outline()
                            .label("← Back")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::License, window, cx);
                            })),
                    )
                    .child(
                        Button::new("vs-next-btn")
                            .primary()
                            .label("Next →")
                            .disabled(!has_selection)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::InstallOptions, window, cx);
                            })),
                    ),
            )
    }

    // ─── Release list ─────────────────────────────────────────────────────────

    fn render_release_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_idx = self.selected_release_idx;
        let show_pre     = self.show_prereleases;

        let visible: Vec<(usize, ReleaseInfo)> = self
            .releases
            .iter()
            .enumerate()
            .filter(|(_, r)| show_pre || !r.prerelease)
            .map(|(i, r)| (i, r.clone()))
            .collect();

        v_flex()
            .id("release-list-scroll")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .py_3()
            .gap_2()
            .children(visible.into_iter().map(|(idx, release)| {
                let selected = selected_idx == Some(idx);
                let is_pre   = release.prerelease;

                h_flex()
                    .px_4()
                    .py_3()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(if selected {
                        cx.theme().secondary.opacity(0.5)
                    } else {
                        cx.theme().border
                    })
                    .bg(if selected {
                        cx.theme().secondary.opacity(0.06)
                    } else {
                        cx.theme().sidebar.opacity(0.3)
                    })
                    .gap_3()
                    .items_center()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.select_release(idx, cx);
                    }))
                    // Radio dot
                    .child(
                        gpui::div()
                            .w(px(18.0))
                            .h(px(18.0))
                            .rounded_full()
                            .border_2()
                            .border_color(if selected {
                                cx.theme().secondary
                            } else {
                                cx.theme().border
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(selected, |e| {
                                e.child(
                                    gpui::div()
                                        .w(px(8.0))
                                        .h(px(8.0))
                                        .rounded_full()
                                        .bg(cx.theme().secondary),
                                )
                            }),
                    )
                    .child(
                        Icon::new(IconName::GalleryVerticalEnd)
                            .with_size(px(16.0))
                            .text_color(if selected {
                                cx.theme().secondary
                            } else {
                                cx.theme().muted_foreground
                            }),
                    )
                    // Release info
                    .child(
                        v_flex()
                            .flex_1()
                            .gap(px(2.0))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        gpui::div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child(release.name.clone()),
                                    )
                                    .when(is_pre, |e| {
                                        e.child(
                                            gpui::div()
                                                .px_2()
                                                .py(px(1.0))
                                                .rounded(px(4.0))
                                                .bg(cx.theme().warning.opacity(0.15))
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(cx.theme().warning)
                                                .child("pre-release"),
                                        )
                                    }),
                            )
                            .child(
                                gpui::div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(release.tag_name.clone()),
                            ),
                    )
            }))
            // Load More button
            .when(self.has_more_releases || self.loading_more, |el: gpui::Stateful<gpui::Div>| {
                el.child(
                    gpui::div()
                        .py_3()
                        .flex()
                        .justify_center()
                        .child(
                            Button::new("load-more-btn")
                                .outline()
                                .label(if self.loading_more { "Loading…" } else { "Load More" })
                                .disabled(self.loading_more)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.load_more_releases(window, cx);
                                })),
                        ),
                )
            })
    }

    // ─── Empty state ──────────────────────────────────────────────────────────

    fn render_empty_releases(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .p_8()
            .child(
                Icon::new(IconName::Github)
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
                            .child("No releases found"),
                    )
                    .child(
                        gpui::div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground.opacity(0.6))
                            .child("Could not fetch releases from GitHub"),
                    ),
            )
            .child(
                Button::new("vs-retry-btn")
                    .outline()
                    .label("Retry")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.fetch_releases(window, cx);
                    })),
            )
    }
}
