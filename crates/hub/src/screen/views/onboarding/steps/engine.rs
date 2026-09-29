use gpui::prelude::*;
use gpui::*;
use ui::{h_flex, v_flex, ActiveTheme, Icon, IconName, Sizable, StyledExt};

use crate::screen::EntryScreen;

pub fn render(screen: &mut EntryScreen, cx: &mut Context<EntryScreen>) -> AnyElement {
    let theme = cx.theme().clone();
    v_flex()
        .size_full()
        .gap_3()
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Icon::new(IconName::Download).size_6().text_color(theme.accent))
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground)
                                .child("Install your first engine"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("An engine is the runtime used to create and open Pulsar projects. Choose Stable for the recommended release, then install it below."),
                        ),
                ),
        )
        .child(
            div().flex_1().min_h_0().child(
                crate::screen::views::versions::render_install_panel(screen, cx, true),
            ),
        )
        .into_any_element()
}
