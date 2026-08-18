use gpui::prelude::*;
use gpui::*;
use ui::{
    h_flex, v_flex, ActiveTheme as _, StyledExt as _, scroll::ScrollbarAxis, spinner::Spinner,
};

use crate::screen::EntryScreen;

/// A full-screen overlay shown while the local "src" engine is being compiled.
/// Streams cargo output into a log box and shows crate done/remaining with a
/// progress bar.
pub fn render_src_build_overlay(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> AnyElement {
    let theme = cx.theme();
    let Some(progress) = screen.state.ui.build_progress.clone() else {
        return h_flex()
            .id("src-build-overlay")
            .absolute()
            .size_full()
            .bg(theme.background.opacity(0.85))
            .items_center()
            .justify_center()
            .gap_3()
            .child(Spinner::new().color(theme.muted_foreground))
            .child(div().text_sm().text_color(theme.foreground).child("Building engine from source…"))
            .into_any_element();
    };

    let (done, total, current, logs, error) = {
        let p = progress.lock();
        (
            p.done,
            p.total,
            p.current.clone(),
            p.logs.clone(),
            p.error.clone(),
        )
    };
    let remaining = total.saturating_sub(done);
    let fraction = if total > 0 {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let done_text = if done == 0 { "…".to_string() } else { done.to_string() };
    let status_text = match &error {
        Some(e) => format!("Build failed: {}", e),
        None if done == 0 => "Preparing…".to_string(),
        None if done >= total && total > 0 => "Done".to_string(),
        None => "Building".to_string(),
    };

    h_flex()
        .id("src-build-overlay")
        .absolute()
        .size_full()
        .bg(theme.background.opacity(0.9))
        .items_center()
        .justify_center()
        .p_16()
        .child(
            v_flex()
                .w_full()
                .max_w(px(780.))
                .p_6()
                .gap_4()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .shadow_lg()
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(Spinner::new().color(theme.accent))
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child("Building engine from source"),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(format!("{} · {} done / {} remaining", status_text, done_text, remaining)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(if current.is_empty() {
                            "Waiting for compiler…".to_string()
                        } else {
                            current
                        }),
                )
                // Compiler output log box
                .child(
                    v_flex()
                        .id("src-build-log")
                        .w_full()
                        .h(px(260.))
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.background.opacity(0.4))
                        .scrollable(ScrollbarAxis::Vertical)
                        .px_3()
                        .py_2()
                        .text_size(px(12.0))
                        .text_color(
                            if error.is_some() {
                                theme.danger
                            } else {
                                theme.foreground
                            },
                        )
                        .children({
                            let lines: Vec<AnyElement> = logs
                                .iter()
                                .map(|line| div().child(line.clone()).into_any_element())
                                .collect();
                            if lines.is_empty() {
                                vec![div()
                                    .text_color(theme.muted_foreground)
                                    .child("—")
                                    .into_any_element()]
                            } else {
                                lines
                            }
                        }),
                )
                // Progress bar at the bottom
                .child(
                    div()
                        .w_full()
                        .h(px(6.))
                        .rounded_full()
                        .bg(theme.border)
                        .relative()
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left_0()
                                .w(relative(fraction))
                                .rounded_full()
                                .bg(if error.is_some() {
                                    theme.danger
                                } else {
                                    theme.accent
                                }),
                        ),
                ),
        )
        .into_any_element()
}
