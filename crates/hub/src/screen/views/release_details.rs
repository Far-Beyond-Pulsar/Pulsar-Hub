use gpui::prelude::*;
use gpui::*;
use ui::{
    h_flex, scroll::ScrollbarAxis, v_flex, ActiveTheme as _, Icon, IconName, StyledExt as _,
    text::TextView,
};

/// A lightweight managed view shown inside a `Popover` that displays a full
/// release's release notes (rendered as Markdown) in an internally scrollable
/// panel. Each release row shares this single view; the active release's
/// content is set right before the popover opens.
pub struct ReleaseDetailsView {
    focus_handle: FocusHandle,
    title: String,
    body: String,
}

impl ReleaseDetailsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            title: String::new(),
            body: String::new(),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_content(&mut self, title: String, body: String, cx: &mut Context<Self>) {
        self.title = title;
        self.body = body;
        cx.notify();
    }
}

impl EventEmitter<DismissEvent> for ReleaseDetailsView {}

impl Focusable for ReleaseDetailsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ReleaseDetailsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let title = self.title.clone();
        let body = self.body.clone();

        v_flex()
            .id("release-details-panel")
            .w(px(600.))
            .max_h(px(560.))
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
                    .p_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::Label)
                                    .size(px(14.))
                                    .text_color(theme.accent),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child(if title.is_empty() {
                                        "Release Notes".to_string()
                                    } else {
                                        format!("Release Notes · {}", title)
                                    }),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .id("release-details-body")
                    .w_full()
                    .p_4()
                    .gap_2()
                    .scrollable(ScrollbarAxis::Vertical)
                    .child(TextView::markdown(
                        "release-details-md",
                        body,
                        window,
                        cx,
                    )),
            )
    }
}
