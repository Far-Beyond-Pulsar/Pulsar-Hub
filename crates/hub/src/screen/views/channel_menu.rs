use gpui::prelude::*;
use gpui::*;
use ui::{
    checkbox::Checkbox, h_flex, v_flex, ActiveTheme as _, StyledExt as _,
};

use crate::service::installer_service::ReleaseChannel;
use crate::EntryScreen;

/// A managed view shown inside a `Popover` that lets the user toggle which
/// release channels feed the version list. Stays open so any combination of
/// channels can be selected.
pub struct ChannelMenuView {
    focus_handle: FocusHandle,
    screen: WeakEntity<EntryScreen>,
}

impl ChannelMenuView {
    pub fn new(screen: WeakEntity<EntryScreen>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            screen,
        }
    }
}

impl EventEmitter<DismissEvent> for ChannelMenuView {}

impl Focusable for ChannelMenuView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChannelMenuView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let weak = self.screen.clone();
        let selected: Vec<ReleaseChannel> = self
            .screen
            .upgrade()
            .map(|s| s.read(cx).state.versions.selected_channels.clone())
            .unwrap_or_default();

        v_flex()
            .id("channel-menu-panel")
            .w(px(220.))
            .p_2()
            .gap_1()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .shadow_lg()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child("Channels"),
            )
            .children(ReleaseChannel::ALL.iter().map(|channel| {
                let checked = selected.contains(channel);
                let weak = weak.clone();
                let channel = *channel;
                h_flex()
                    .id(format!("channel-row-{}", channel.label()))
                    .w_full()
                    .h(px(30.))
                    .gap_2()
                    .items_center()
                    .rounded_md()
                    .hover(|this| this.bg(theme.accent.opacity(0.06)))
                    .px_1()
                    .child(
                        Checkbox::new(format!("channel-check-{}", channel.label()))
                            .label(channel.label())
                            .checked(checked)
                            .on_click(move |selected, _, cx| {
                                if let Some(e) = weak.upgrade() {
                                    let _ = e.update(cx, |this, cx| {
                                        this.toggle_channel(channel, *selected, cx);
                                    });
                                }
                            }),
                    )
            }))
    }
}
