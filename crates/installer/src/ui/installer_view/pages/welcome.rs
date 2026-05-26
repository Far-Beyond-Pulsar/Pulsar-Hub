//! Welcome page — landing screen with "Install New" and "Manage Installed" cards.

use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, px};
use gpui_component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
    Icon, IconName,
};
use super::super::{InstallerView, Page};

impl InstallerView {
    pub(crate) fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(Self::render_panel_header("Welcome", None, cx))
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_8()
                    .p_8()
                    // ── Logo ──────────────────────────────────────────────────
                    .child(
                        gpui::div()
                            .w(px(80.0))
                            .h(px(80.0))
                            .rounded(px(20.0))
                            .bg(cx.theme().secondary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                gpui::div()
                                    .text_3xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().secondary_foreground)
                                    .child("P"),
                            ),
                    )
                    // ── Heading ───────────────────────────────────────────────
                    .child(
                        v_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                gpui::div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Pulsar Engine Installer"),
                            )
                            .child(
                                gpui::div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .text_center()
                                    .child(
                                        "Download and install Pulsar engine versions, \
                                         or manage existing installations.",
                                    ),
                            ),
                    )
                    // ── Feature pills ─────────────────────────────────────────
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .justify_center()
                            .child(Self::feature_pill("Cross-platform", cx))
                            .child(Self::feature_pill("Multi-version", cx))
                            .child(Self::feature_pill("Auto-detect arch", cx)),
                    )
                    // ── Action cards ──────────────────────────────────────────
                    .child(
                        h_flex()
                            .gap_6()
                            // Install New card
                            .child(
                                v_flex()
                                    .w(px(200.0))
                                    .p_5()
                                    .gap_3()
                                    .rounded(px(12.0))
                                    .border_1()
                                    .border_color(cx.theme().secondary)
                                    .bg(cx.theme().secondary)
                                    .cursor_pointer()
                                    .child(
                                        Icon::new(IconName::ArrowDown)
                                            .with_size(px(28.0))
                                            .text_color(cx.theme().secondary),
                                    )
                                    .child(
                                        gpui::div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("Install New"),
                                    )
                                    .child(
                                        gpui::div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Download and install a release from GitHub."),
                                    )
                                    .child(
                                        Button::new("welcome-install-btn")
                                            .primary()
                                            .label("Get Started →")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.navigate_to(Page::License, window, cx);
                                                this.fetch_license(cx);
                                            })),
                                    ),
                            )
                            // Manage Installed card
                            .child(
                                v_flex()
                                    .w(px(200.0))
                                    .p_5()
                                    .gap_3()
                                    .rounded(px(12.0))
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().sidebar)
                                    .cursor_pointer()
                                    .child(
                                        Icon::new(IconName::Inbox)
                                            .with_size(px(28.0))
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(
                                        gpui::div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("Manage Installed"),
                                    )
                                    .child(
                                        gpui::div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("View, launch, or remove existing installations."),
                                    )
                                    .child(
                                        Button::new("welcome-manage-btn")
                                            .outline()
                                            .label("Open Manager →")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.navigate_to(Page::VersionsManager, window, cx);
                                                this.load_installed_versions(cx);
                                            })),
                                    ),
                            ),
                    ),
            )
    }
}
