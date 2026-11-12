//! The build tools dialog: what the Hub found (Rust, and whether this machine
//! can build and link), what to do about what is missing, and a button to set
//! Rust up. Only needed to build the engine or a plugin from source; installed
//! engines need none of it.

use gpui::prelude::*;
use gpui::*;
use ui::{
    ActiveTheme as _, Disableable as _, Icon, IconName, StyledExt,
    button::{Button, ButtonVariants as _},
    h_flex,
    scroll::ScrollbarAxis,
    spinner::Spinner,
    v_flex,
};
use wgpui_motion::prelude::MotionExt as _;

use crate::core::types::{DependencyStatus, InstallStatus};
use crate::screen::EntryScreen;

pub fn render_dependency_setup(
    screen: &mut EntryScreen,
    window: &mut Window,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let checking = screen.state.dependency_checking;
    let status: Option<DependencyStatus> = screen.state.dependency_status.clone();
    let progress = screen.state.install_progress.clone();
    let installing = progress.as_ref().is_some_and(|p| {
        matches!(p.status, InstallStatus::Downloading | InstallStatus::Installing)
    });
    let error = progress.as_ref().and_then(|p| match &p.status {
        InstallStatus::Error(message) => Some(message.clone()),
        _ => None,
    });
    let busy = checking || installing;

    let rust_ok = status.as_ref().is_some_and(|s| s.rust_installed);
    let needs_default = status.as_ref().is_some_and(|s| s.needs_default_toolchain);
    let action_label = if needs_default {
        "Repair Rust"
    } else if status.is_some() && !rust_ok {
        "Install Rust"
    } else {
        "Update Rust"
    };

    let rust_row = row(
        "Rust toolchain",
        if status.is_none() {
            Mark::Working
        } else if rust_ok {
            Mark::Ok
        } else {
            Mark::Missing
        },
        status.as_ref().and_then(|s| s.rust_note.clone()),
        None,
        status.as_ref().map(|s| s.rust_details.clone()).unwrap_or_default(),
        &theme,
    );
    let tools_hint = status
        .as_ref()
        .filter(|s| !s.build_tools_installed)
        .and_then(|s| match (&s.build_tools_problem, &s.build_tools_hint) {
            (Some(problem), Some(hint)) => Some(format!("rustc said: {problem}

{hint}")),
            (None, Some(hint)) => Some(hint.clone()),
            (Some(problem), None) => Some(format!("rustc said: {problem}")),
            (None, None) => None,
        });
    let tools_row = row(
        "C/C++ build tools",
        match &status {
            None => Mark::Working,
            Some(s) if s.build_tools_installed => Mark::Ok,
            Some(_) => Mark::Missing,
        },
        status.as_ref().and_then(|s| {
            s.compiler_info.clone().or_else(|| {
                (!s.build_tools_installed).then(|| "Not found, or it could not link.".to_string())
            })
        }),
        tools_hint,
        status.as_ref().map(|s| s.tools_details.clone()).unwrap_or_default(),
        &theme,
    );

    div()
        .absolute()
        .size_full()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p_6()
        .bg(gpui::black().opacity(0.48))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            v_flex()
                .w_full()
                .max_w(px(560.))
                .max_h_full()
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
                            Icon::new(IconName::Settings)
                                .size(px(20.))
                                .text_color(theme.foreground),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                // Without `min_w_0` a flex child is as wide as its
                                // text, and the subtitle runs out of the dialog.
                                .min_w_0()
                                .gap_1()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child("Build tools"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child(
                                            "Only needed to build the engine or a plugin from \
                                             source. Installed engine versions need none of this.",
                                        ),
                                ),
                        ),
                )
                .child(
                    v_flex()
                        .id("dependency-rows")
                        .gap_2()
                        .min_h_0()
                        .scrollable(ScrollbarAxis::Vertical)
                        .child(rust_row)
                        .child(tools_row),
                )
                .when_some(progress.clone().filter(|p| !p.logs.is_empty()), |this, p| {
                    this.child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .w_full()
                                    .h(px(6.))
                                    .rounded_sm()
                                    .bg(theme.secondary.opacity(0.3))
                                    .child(
                                        div()
                                            .h_full()
                                            .rounded_sm()
                                            .bg(if matches!(p.status, InstallStatus::Error(_)) {
                                                theme.danger
                                            } else if p.status == InstallStatus::Complete {
                                                theme.success_foreground
                                            } else {
                                                theme.accent
                                            })
                                            .w(relative(p.progress.clamp(0.0, 1.0))),
                                    ),
                            )
                            .child(
                                div()
                                    .id("dependency-setup-logs")
                                    .w_full()
                                    .max_h(px(120.))
                                    .p_3()
                                    .rounded_md()
                                    .bg(theme.background.opacity(0.55))
                                    .scrollable(ScrollbarAxis::Vertical)
                                    .child(v_flex().gap_1().children(
                                        p.logs.iter().rev().take(30).rev().map(|log| {
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .font_family(SharedString::from("monospace"))
                                                .child(log.clone())
                                        }),
                                    )),
                            ),
                    )
                })
                .when_some(error, |this, message| {
                    this.child(
                        div()
                            .p_3()
                            .rounded_md()
                            .bg(theme.danger.opacity(0.10))
                            .border_1()
                            .border_color(theme.danger.opacity(0.35))
                            .text_xs()
                            .text_color(theme.foreground)
                            .child(format!("Setup failed: {message}")),
                    )
                })
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(if checking {
                                    "Checking: this builds a small test program..."
                                } else if installing {
                                    "Setting up Rust. Nothing already installed is removed."
                                } else {
                                    ""
                                }),
                        )
                        .child(
                            Button::new("dependency-recheck")
                                .label("Check again")
                                .icon(IconName::Refresh)
                                .ghost()
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.check_dependencies_async(cx)
                                })),
                        )
                        .when(status.is_some() && (!rust_ok || needs_default), |this| {
                            this.child(
                                Button::new("dependency-install-rust")
                                    .label(action_label)
                                    .primary()
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.start_dependency_setup(cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("close-dependency-setup")
                                .label("Close")
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state.ui.show_dependency_setup = false;
                                    cx.notify();
                                })),
                        ),
                ),
        )
        .child(crate::screen::views::confetti::render_confetti(
            screen.state.confetti_started,
            window,
        ))
}

enum Mark {
    Working,
    Ok,
    Missing,
}

fn row(
    name: &str,
    mark: Mark,
    detail: Option<String>,
    hint: Option<String>,
    details: Vec<(String, String)>,
    theme: &ui::Theme,
) -> impl IntoElement {
    let (icon, color): (AnyElement, Hsla) = match mark {
        Mark::Working => (
            Spinner::new().color(theme.muted_foreground).into_any_element(),
            theme.muted_foreground,
        ),
        Mark::Ok => (
            Icon::new(IconName::Check)
                .size_4()
                .text_color(theme.success_foreground)
                .into_any_element(),
            theme.success_foreground,
        ),
        Mark::Missing => (
            Icon::new(IconName::WarningTriangle)
                .size_4()
                .text_color(theme.warning)
                .into_any_element(),
            theme.warning,
        ),
    };
    v_flex()
        .gap_2()
        .p_3()
        .rounded_md()
        .bg(theme.secondary.opacity(0.12))
        .border_1()
        .border_color(theme.border)
        .child(
            h_flex()
                .gap_3()
                .items_start()
                .child(div().pt(px(2.)).child(icon))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.foreground)
                                .child(name.to_string()),
                        )
                        .when_some(detail, |this, detail| {
                            this.child(div().text_xs().text_color(color).child(detail))
                        }),
                ),
        )
        .when(!details.is_empty(), |this| {
            this.child(
                v_flex()
                    .gap_1()
                    .pt_2()
                    .border_t_1()
                    .border_color(theme.border)
                    .children(details.into_iter().map(|(label, value)| {
                        h_flex()
                            .gap_3()
                            .items_start()
                            .child(
                                div()
                                    .w(px(120.))
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(label),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(theme.foreground)
                                    .child(value),
                            )
                    })),
            )
        })
        .when_some(hint, |this, hint| {
            this.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(theme.background.opacity(0.55))
                    .text_xs()
                    .text_color(theme.foreground)
                    .font_family(SharedString::from("monospace"))
                    .child(hint),
            )
        })
}
