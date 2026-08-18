use gpui::prelude::*;
use gpui::*;
use ui::{h_flex, ActiveTheme as _, StyledExt as _, spinner::Spinner};

/// A full-screen overlay shown inline while the local "src" engine is being
/// compiled from a source checkout.
pub fn render_src_build_overlay(cx: &mut Context<crate::screen::EntryScreen>) -> AnyElement {
    let theme = cx.theme();
    h_flex()
        .id("src-build-overlay")
        .absolute()
        .size_full()
        .bg(theme.background.opacity(0.85))
        .items_center()
        .justify_center()
        .gap_3()
        .child(Spinner::new().color(theme.muted_foreground))
        .child(
            div()
                .text_sm()
                .text_color(theme.foreground)
                .child("Building engine from source…"),
        )
        .into_any_element()
}
