//! Shared UI helpers reused across multiple pages.

use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, px};
use gpui_component::{ActiveTheme, spinner::Spinner, h_flex, v_flex};
use gpui::prelude::FluentBuilder;
use super::super::InstallerView;

impl InstallerView {
    // ─── Panel header ─────────────────────────────────────────────────────────

    /// Standard two-column panel header with a title and an optional badge chip.
    pub fn render_panel_header(
        title: &str,
        badge: Option<&str>,
        cx: &mut Context<InstallerView>,
    ) -> impl IntoElement {
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
                            .child(title.to_string()),
                    )
                    .map(|el| {
                        if let Some(b) = badge {
                            el.child(
                                gpui::div()
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .bg(cx.theme().secondary)
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().secondary)
                                    .child(b.to_string()),
                            )
                        } else {
                            el
                        }
                    }),
            )
    }

    // ─── Loading state ────────────────────────────────────────────────────────

    /// Centred spinner + message, used while async data is being fetched.
    pub fn render_loading_state(
        &self,
        message: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(Spinner::new().color(cx.theme().secondary))
            .child(
                gpui::div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(message.to_string()),
            )
    }

    // ─── Feature pill ─────────────────────────────────────────────────────────

    /// Small rounded label used on the Welcome page to highlight features.
    pub fn feature_pill(label: &str, cx: &mut Context<InstallerView>) -> impl IntoElement {
        gpui::div()
            .px_3()
            .py_1()
            .rounded_full()
            .bg(cx.theme().secondary)
            .border_1()
            .border_color(cx.theme().secondary)
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(cx.theme().secondary)
            .child(label.to_string())
    }
}
