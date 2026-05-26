//! Release-notes page — markdown view of the selected engine release notes.

use gpui::{
    Context, FontWeight, IntoElement, InteractiveElement as _,
    ParentElement, Styled, Window, px,
};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
    text::TextView,
};
use super::super::{InstallerView, Page};

impl InstallerView {
    pub(crate) fn render_release_notes(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let notes_md = self
            .selected_release_idx
            .and_then(|idx| self.releases.get(idx).map(|r| r.body.clone()))
            .filter(|body| !body.trim().is_empty())
            .unwrap_or_else(|| "No release notes available for this version.".to_string());

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Release Notes", None, cx))
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .p_6()
                    .gap_3()
                    .child(
                        gpui::div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().muted_foreground)
                            .child("Review the notes before continuing."),
                    )
                    .child(
                        v_flex()
                            .id("release-notes-container")
                            .flex_1()
                            .overflow_y_scroll()
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().sidebar)
                            .p_4()
                            .child(TextView::markdown("release-notes-md", notes_md, window, cx)),
                    ),
            )
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .items_center()
                    .child(
                        Button::new("rn-back-btn")
                            .outline()
                            .label("← Back")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::VersionSelection, window, cx);
                            })),
                    )
                    .child(
                        Button::new("rn-next-btn")
                            .primary()
                            .label("Next →")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::InstallOptions, window, cx);
                            })),
                    ),
            )
    }
}
