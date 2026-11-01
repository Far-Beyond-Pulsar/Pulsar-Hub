use gpui::prelude::*;
use gpui::*;
use ui::{
    button::Button, button::ButtonVariants as _, h_flex, scroll::ScrollbarAxis, v_flex,
    ActiveTheme as _, Icon, IconName, StyledExt,
};

use crate::core::types::InstallStatus;
use crate::screen::EntryScreen;
use wgpui_motion::prelude::MotionExt as _;

pub fn render_dependency_setup(
    screen: &mut EntryScreen,
    _window: &mut Window,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let progress = screen.state.install_progress.as_ref();
    let install_progress_val = progress.map(|p| p.progress).unwrap_or(0.0);
    let install_logs: Vec<String> = progress.map(|p| p.logs.clone()).unwrap_or_default();
    let is_complete = progress
        .map(|p| p.status == InstallStatus::Complete)
        .unwrap_or(false);
    let is_error = progress
        .map(|p| matches!(p.status, InstallStatus::Error(_)))
        .unwrap_or(false);
    let error_message = progress.and_then(|progress| match &progress.status {
        InstallStatus::Error(message) => Some(message.clone()),
        _ => None,
    });
    let status_text = progress
        .map(|p| match &p.status {
            InstallStatus::Idle => "Ready",
            InstallStatus::Downloading => "Downloading...",
            InstallStatus::Installing => "Installing...",
            InstallStatus::Complete => "Complete!",
            InstallStatus::Error(_) => "Error",
        })
        .unwrap_or("Checking Rust toolchain");

    div()
        .absolute()
        .size_full()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::black().opacity(0.48))
        .child(
            v_flex()
                .w_full()
                .max_w(px(500.))
                .p_5()
                .gap_4()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(theme.sidebar.opacity(0.98))
                .shadow_lg()
                .motion("dependency-setup-modal-motion")
                .transition(wgpui_motion::prelude::Transition::token(
                    wgpui_motion::prelude::MotionToken::SpatialFast,
                ))
                .enter(|style| style.opacity(0.).mt(px(8.)))
                .base(|style| style.opacity(1.).mt(px(0.)))
                .child(
                    h_flex()
                        .items_start()
                        .gap_3()
                        .child(
                            Icon::new(IconName::Package)
                                .size(px(20.))
                                .text_color(theme.foreground),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .gap_1()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child("Dependency Setup"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("Setting up required build tools for Pulsar Engine"),
                                ),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(8.))
                        .bg(theme.secondary.opacity(0.3))
                        .rounded_sm()
                        .child(
                            div()
                                .h_full()
                                .rounded_sm()
                                .bg(if is_error {
                                    gpui::red()
                                } else if is_complete {
                                    theme.success_foreground
                                } else {
                                    theme.accent
                                })
                                .w(relative(install_progress_val.max(0.0).min(1.0))),
                        ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .p_3()
                        .rounded_md()
                        .bg(theme.secondary.opacity(0.10))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(
                                    match progress.as_ref().map(|p| &p.status) {
                                        Some(InstallStatus::Complete) => theme.success_foreground,
                                        Some(InstallStatus::Error(_)) => gpui::red(),
                                        _ => theme.accent,
                                    },
                                ))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.foreground)
                                        .child("Rust toolchain"),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(
                                    match progress.as_ref().map(|p| &p.status) {
                                        Some(InstallStatus::Complete) => theme.success_foreground,
                                        _ => theme.muted_foreground,
                                    },
                                ))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(if install_progress_val > 0.0 {
                                            theme.foreground
                                        } else {
                                            theme.muted_foreground
                                        })
                                        .child("C/C++ build tools"),
                                ),
                        ),
                )
                .when(!install_logs.is_empty(), |this| {
                    this.child(
                        div()
                            .id("dependency-setup-logs")
                            .w_full()
                            .max_h(px(140.))
                            .p_3()
                            .rounded_md()
                            .bg(theme.background.opacity(0.55))
                            .scrollable(ScrollbarAxis::Vertical)
                            .child(v_flex().gap_1().children(
                                install_logs.iter().rev().take(30).rev().map(|log| {
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .font_family(SharedString::from("monospace"))
                                        .child(log.clone())
                                }),
                            )),
                    )
                })
                .when_some(error_message, |this, message| {
                    this.child(
                        v_flex()
                            .gap_1()
                            .p_3()
                            .rounded_md()
                            .bg(gpui::red().opacity(0.10))
                            .border_1()
                            .border_color(gpui::red().opacity(0.35))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(gpui::red())
                                    .child("Setup failed"),
                            )
                            .child(div().text_xs().text_color(theme.foreground).child(message)),
                    )
                })
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(status_text),
                        )
                        .child(
                            Button::new("close-dependency-setup")
                                .label(if is_complete { "Done" } else { "Close" })
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state.ui.show_dependency_setup = false;
                                    cx.notify();
                                })),
                        ),
                ),
        )
}
