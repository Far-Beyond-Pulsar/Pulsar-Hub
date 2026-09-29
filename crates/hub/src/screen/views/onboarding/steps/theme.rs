use gpui::prelude::*;
use gpui::*;
use ui::{h_flex, v_flex, ActiveTheme, Icon, IconName, StyledExt};

use crate::screen::EntryScreen;

pub fn render(screen: &mut EntryScreen, cx: &mut Context<EntryScreen>) -> AnyElement {
    let theme = cx.theme().clone();
    let active_theme = cx.theme().theme_name().clone();
    v_flex()
        .size_full()
        .gap_4()
        .child(
            h_flex()
                .items_center()
                .gap_3()
                .child(Icon::new(IconName::Palette).size_6().text_color(theme.accent))
                .child(v_flex().gap_1()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(theme.foreground).child("Choose a look"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child(format!("Pick a theme for Pulsar Hub. You can change it any time in settings. Current theme: {active_theme}.")))),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .bg(theme.secondary.opacity(0.12))
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .child(super::super::render_theme_content(screen, cx)),
        )
        .into_any_element()
}
