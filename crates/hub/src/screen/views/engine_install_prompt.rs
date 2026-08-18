use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Icon, IconName,
};

use crate::component::render_modal;
use crate::screen::EntryScreen;

pub fn render_engine_install_prompt(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> gpui::AnyElement {
    let theme = cx.theme();
    let Some(prompt) = screen.state.ui.engine_prompt.clone() else {
        return div().into_any_element();
    };

    render_modal(
        h_flex()
            .gap_2()
            .items_center()
            .child(
                Icon::new(IconName::WarningTriangle)
                    .size(px(18.))
                    .text_color(theme.warning),
            )
            .child(div().child("Engine Required")),
        v_flex()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child(format!(
                        "“{}” requires engine “{}”, which isn’t installed.",
                        prompt.project_name, prompt.required
                    )),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Would you like to install it automatically?"),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("engine-prompt-cancel")
                            .label("Not now")
                            .compact()
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_engine_prompt(cx);
                            })),
                    )
                    .child(
                        Button::new("engine-prompt-install")
                            .label("Install")
                            .primary()
                            .compact()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.install_engine_from_prompt(cx);
                            })),
                    ),
            ),
        Some(Box::new(|_window, cx| {
            let screen = cx.entity();
            let _ = screen.update(cx, |this, cx| this.close_engine_prompt(cx));
        })),
        cx,
    )
    .into_any_element()
}
