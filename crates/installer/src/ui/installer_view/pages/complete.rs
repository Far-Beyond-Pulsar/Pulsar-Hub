//! Complete page — success / failure summary after installation finishes.

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
use super::super::{InstallerView, LogLevel, Page};

impl InstallerView {
    pub(crate) fn render_complete(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let success = !self.install_failed;

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Installation Complete", None, cx))
            // ── Result body ───────────────────────────────────────────────────
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_6()
                    .p_8()
                    // Result icon circle
                    .child(
                        gpui::div()
                            .w(px(72.0))
                            .h(px(72.0))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(if success {
                                cx.theme().success
                            } else {
                                cx.theme().danger
                            })
                            .border_2()
                            .border_color(if success {
                                cx.theme().success
                            } else {
                                cx.theme().danger
                            })
                            .child(
                                Icon::new(if success {
                                    IconName::CircleCheck
                                } else {
                                    IconName::CircleX
                                })
                                .with_size(px(36.0))
                                .text_color(if success {
                                    cx.theme().success
                                } else {
                                    cx.theme().danger
                                }),
                            ),
                    )
                    // Heading + sub-text
                    .child(
                        v_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                gpui::div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(if success {
                                        "Installation Complete!"
                                    } else {
                                        "Installation Finished with Errors"
                                    }),
                            )
                            .child(
                                gpui::div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .text_center()
                                    .child(if success {
                                        "Pulsar engine has been successfully installed on your system."
                                    } else {
                                        "Some steps failed. Review the output log for details."
                                    }),
                            ),
                    )
                    // Summary log card
                    .child(self.render_summary_log(cx)),
            )
            // ── Action bar ────────────────────────────────────────────────────
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_end()
                    .gap_3()
                    .when(success && self.installed_path.is_some(), |el| {
                        el.child(
                            Button::new("open-folder-btn")
                                .outline()
                                .label("Open Folder")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.open_install_folder(cx);
                                })),
                        )
                        .child(
                            Button::new("launch-btn")
                                .outline()
                                .label("Launch Pulsar")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.launch_pulsar(cx);
                                })),
                        )
                    })
                    .child(
                        Button::new("finish-btn")
                            .primary()
                            .label("Finish")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.current_page = Page::VersionsManager;
                                this.load_installed_versions(cx);
                                cx.notify();
                            })),
                    ),
            )
    }

    // ─── Summary log ──────────────────────────────────────────────────────────

    fn render_summary_log(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(400.0))
            .max_h(px(160.0))
            .rounded(px(8.0))
            .bg(cx.theme().sidebar)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            // Card header
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        gpui::div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("SUMMARY"),
                    ),
            )
            // Filtered log entries (Success / Warning / Error only)
            .child(
                v_flex()
                    .id("log-entries-scroll")
                    .overflow_y_scroll()
                    .px_3()
                    .py_2()
                    .gap(px(2.0))
                    .children(
                        self.log_entries
                            .iter()
                            .filter(|e| {
                                matches!(
                                    e.level,
                                    LogLevel::Success | LogLevel::Error | LogLevel::Warning
                                )
                            })
                            .map(|entry| {
                                let color = match entry.level {
                                    LogLevel::Success => cx.theme().success,
                                    LogLevel::Error   => cx.theme().danger,
                                    LogLevel::Warning => cx.theme().warning,
                                    LogLevel::Info    => cx.theme().muted_foreground,
                                };
                                gpui::div()
                                    .text_xs()
                                    .text_color(color)
                                    .child(entry.message.clone())
                            }),
                    ),
            )
    }
}
