//! Installing page — progress bar + scrollable output log.

use gpui::{
    Context, FontWeight, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, px, InteractiveElement,
};
use gpui_component::{
    ActiveTheme, Sizable as _,
    h_flex, v_flex,
    Icon, IconName,
    progress::Progress,
    spinner::Spinner,
};
use gpui::prelude::FluentBuilder;
use super::super::{InstallerView, LogLevel};

impl InstallerView {
    pub(crate) fn render_installing(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_done = self.install_progress >= 100.0;

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Installing", None, cx))
            // ── Progress section ──────────────────────────────────────────────
            .child(
                v_flex()
                    .px_6()
                    .py_5()
                    .gap_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar.opacity(0.3))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .when(!is_done, |el| el.child(Spinner::new().color(cx.theme().accent)))
                            .when(is_done, |el| {
                                el.child(
                                    Icon::new(IconName::CircleCheck)
                                        .with_size(px(20.0))
                                        .text_color(cx.theme().success),
                                )
                            })
                            .child(
                                gpui::div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().foreground)
                                    .child(self.install_message.clone()),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Progress::new().value(self.install_progress))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child(
                                        gpui::div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Overall progress"),
                                    )
                                    .child(
                                        gpui::div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().accent)
                                            .child(format!("{:.0}%", self.install_progress)),
                                    ),
                            ),
                    ),
            )
            // ── Output log ────────────────────────────────────────────────────
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .px_4()
                            .py_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::SquareTerminal)
                                    .with_size(px(13.0))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                gpui::div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().muted_foreground)
                                    .child("OUTPUT"),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("log-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .px_4()
                            .py_3()
                            .gap(px(3.0))
                            .children(self.log_entries.iter().map(|entry| {
                                let (icon, color) = log_level_style(&entry.level, cx);
                                h_flex()
                                    .gap_2()
                                    .items_start()
                                    .child(Icon::new(icon).with_size(px(12.0)).text_color(color))
                                    .child(
                                        gpui::div()
                                            .text_xs()
                                            .text_color(color)
                                            .child(entry.message.clone()),
                                    )
                            }))
                            .when(
                                self.log_entries.is_empty(),
                                |el: gpui::Stateful<gpui::Div>| {
                                    el.child(
                                        gpui::div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground.opacity(0.5))
                                            .child("Waiting for output\u{2026}"),
                                    )
                                },
                            ),
                    ),
            )
    }
}

fn log_level_style(
    level: &LogLevel,
    cx: &gpui::Context<InstallerView>,
) -> (IconName, gpui::Hsla) {
    match level {
        LogLevel::Info    => (IconName::Info,          cx.theme().muted_foreground),
        LogLevel::Success => (IconName::CircleCheck,   cx.theme().success),
        LogLevel::Warning => (IconName::TriangleAlert, cx.theme().warning),
        LogLevel::Error   => (IconName::CircleX,       cx.theme().danger),
    }
}
