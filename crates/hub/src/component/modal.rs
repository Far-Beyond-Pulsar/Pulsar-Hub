use std::rc::Rc;

use gpui::prelude::*;
use gpui::*;
use ui::{
    button::Button,
    button::ButtonVariants as _,
    h_flex,
    v_flex,
    ActiveTheme as _,
    IconName,
};

use crate::screen::EntryScreen;

pub type ModalOnClose = Rc<dyn Fn(&mut EntryScreen, &mut Window, &mut Context<EntryScreen>)>;

pub fn render_modal(
    title: impl IntoElement,
    content: impl IntoElement,
    on_close: Option<ModalOnClose>,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();

    div()
        .absolute()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.background.opacity(0.86))
        // Click on the dimmed backdrop closes the modal; clicks inside the
        // card stop propagation before reaching this handler.
        .when_some(on_close.clone(), |this, close| {
            this.on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                close(this, window, cx);
                cx.notify();
            }))
        })
        .child(
            v_flex()
                .w_full()
                .max_w(px(480.0))
                .p_6()
                .gap_4()
                .rounded_xl()
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .shadow_lg()
                .when_some(on_close, |this, close| {
                    this.on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child(title),
                            )
                            .child(
                                Button::new("modal-close")
                                    .ghost()
                                    .icon(IconName::Close)
                                    .compact()
                                    .tooltip("Close")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        close(this, window, cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                })
                .child(content),
        )
        .into_any_element()
}
