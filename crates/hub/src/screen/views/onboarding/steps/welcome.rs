use gpui::prelude::*;
use gpui::*;
use ui::{v_flex, ActiveTheme, Icon, IconName, Sizable, StyledExt};

pub fn render(cx: &mut Context<crate::screen::EntryScreen>) -> AnyElement {
    let theme = cx.theme();
    v_flex().size_full().items_center().justify_center().gap_4()
        .bg(theme.background).border_1().border_color(theme.border).rounded_lg().p_8()
        .child(Icon::new(IconName::Package).size_8().text_color(theme.accent))
        .child(div().text_2xl().font_weight(FontWeight::BOLD).text_color(theme.foreground).child("Welcome to Pulsar Hub"))
        .child(div().max_w(px(560.)).text_center().text_base().text_color(theme.muted_foreground)
            .child("Pulsar Hub helps you manage engine versions, create projects, and add community plugins. You can skip any step and change these choices later."))
        .child(div().text_sm().text_color(theme.muted_foreground).child("Start by installing an engine version so you can open and create projects."))
        .into_any_element()
}
