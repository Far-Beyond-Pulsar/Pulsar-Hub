//! License page — fetches and displays the project license for acceptance.

use gpui::{Context, IntoElement, InteractiveElement as _, ParentElement, Styled, px, StatefulInteractiveElement};
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
use super::super::{InstallerView, Page};

impl InstallerView {
    pub(crate) fn render_license(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let accepted = self.license_accepted;

        v_flex()
            .size_full()
            .child(Self::render_panel_header("License Agreement", None, cx))
            // Content area (loading / error / text)
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .map(|el| {
                        if self.loading_license {
                            el.child(self.render_loading_state("Fetching license from GitHub…", cx))
                        } else if let Some(err) = self.license_fetch_error.clone() {
                            el.child(Self::render_license_error_state(err, cx))
                        } else if let Some(text) = self.license_text.clone() {
                            el.child(Self::render_license_text(text, cx))
                        } else {
                            el.child(self.render_loading_state("Loading…", cx))
                        }
                    }),
            )
            // Bottom bar
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .items_center()
                    .child(
                        Button::new("license-back-btn")
                            .outline()
                            .label("← Back")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::Welcome, window, cx);
                            })),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            // Accept checkbox
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Checkbox::new("license-accept-checkbox")
                                            .checked(accepted)
                                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                                this.license_accepted = *checked;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        gpui::div()
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child("I accept the license agreement"),
                                    ),
                            )
                            // Next button (disabled until accepted)
                            .child(
                                Button::new("license-next-btn")
                                    .primary()
                                    .label("Next →")
                                    .disabled(!accepted)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.navigate_to(Page::VersionSelection, window, cx);
                                        this.fetch_releases(window, cx);
                                    })),
                            ),
                    ),
            )
    }

    fn render_license_text(text: String, cx: &mut Context<InstallerView>) -> impl IntoElement {
        v_flex()
            .id("license-scroll")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .child(
                gpui::div()
                    .text_xs()
                    .font_family("monospace")
                    .text_color(cx.theme().foreground)
                    .child(text),
            )
    }

    fn render_license_error_state(error: String, cx: &mut Context<InstallerView>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .p_8()
            .child(
                Icon::new(IconName::CircleX)
                    .with_size(px(40.0))
                    .text_color(cx.theme().danger),
            )
            .child(
                gpui::div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .text_center()
                    .child(format!("Failed to fetch license: {error}")),
            )
            .child(
                Button::new("license-retry-btn")
                    .outline()
                    .label("Retry")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.fetch_license(cx);
                    })),
            )
    }
}
