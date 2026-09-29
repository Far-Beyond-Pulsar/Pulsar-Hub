use gpui::prelude::*;
use gpui::*;
use ui::{h_flex, v_flex, ActiveTheme, Icon, IconName, StyledExt};

use crate::screen::EntryScreen;

pub fn render(screen: &mut EntryScreen, cx: &mut Context<EntryScreen>) -> AnyElement {
    let theme = cx.theme().clone();
    let installed_count = screen.state.installed_plugins.len();
    v_flex()
        .size_full()
        .gap_4()
        .child(
            h_flex()
                .items_center()
                .gap_3()
                .child(Icon::new(IconName::Package).size_6().text_color(theme.accent))
                .child(v_flex().gap_1()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(theme.foreground).child("Explore plugins"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Add community extensions when you need them. Plugins are optional and can be installed or removed later."))),
        )
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(theme.accent.opacity(0.1))
                .child(Icon::new(IconName::Check).size_4().text_color(theme.accent))
                .child(div().text_sm().text_color(theme.foreground).child(format!("{installed_count} installed"))),
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
                .child(super::super::render_plugin_content(screen, cx)),
        )
        .into_any_element()
}
