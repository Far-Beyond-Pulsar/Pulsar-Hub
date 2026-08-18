use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants as _},
    h_flex, scroll::ScrollbarAxis, text::TextView, v_flex, ActiveTheme as _, Icon, IconName,
    StyledExt as _,
};

use crate::screen::EntryScreen;

/// A mostly-full-screen overlay that shows an engine version's release
/// notes as Markdown, centered on a dimmed backdrop.
pub fn render_release_notes_modal(
    screen: &mut EntryScreen,
    window: &mut Window,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let Some(modal) = screen.state.ui.release_notes_modal.clone() else {
        return div().into_any_element();
    };

    div()
        .id("engine-release-notes-overlay")
        .absolute()
        .size_full()
        .bg(theme.background.opacity(0.84))
        .flex()
        .items_center()
        .justify_center()
        .p_10()
        .child(
            v_flex()
                .id("engine-release-notes-modal")
                .size_full()
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
                        .p_4()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Icon::new(IconName::Label)
                                        .size(px(18.))
                                        .text_color(theme.accent),
                                )
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child(modal.title),
                                ),
                        )
                        .child(
                            Button::new("engine-release-notes-close")
                                .icon(IconName::Close)
                                .compact()
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_release_notes_modal(cx);
                                })),
                        ),
                )
                .child(
                    v_flex()
                        .id("engine-release-notes-body")
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .scrollable(ScrollbarAxis::Vertical)
                        .px_8()
                        .py_6()
                        .child(TextView::markdown(
                            "engine-release-notes-md",
                            modal.body,
                            window,
                            cx,
                        )),
                ),
        )
        .into_any_element()
}
