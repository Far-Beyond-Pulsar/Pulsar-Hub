use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants as _},
    h_flex, scroll::ScrollbarAxis, v_flex, ActiveTheme as _, Icon, IconName,
    StyledExt as _,
};

use crate::core::types::{DownloadItem, DownloadKind, DownloadStatus, format_bytes};

pub struct DownloadManagerView {
    focus_handle: FocusHandle,
    pub items: Vec<DownloadItem>,
}

impl DownloadManagerView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            items: Vec::new(),
        }
    }

    pub fn active_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| matches!(i.status, DownloadStatus::Downloading { .. }))
            .count()
    }

    pub fn add_item(&mut self, item: DownloadItem) {
        self.items.insert(0, item);
    }

    pub fn update_progress(
        &mut self,
        id: &str,
        bytes_downloaded: u64,
        total_bytes: u64,
        speed_bps: u64,
    ) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.status = DownloadStatus::Downloading {
                bytes_downloaded,
                total_bytes,
                speed_bps,
            };
        }
    }

    pub fn complete(&mut self, id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.status = DownloadStatus::Complete;
        }
    }

    pub fn fail(&mut self, id: &str, error: String) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.status = DownloadStatus::Failed(error);
        }
    }

    pub fn clear_completed(&mut self) {
        self.items.retain(|i| {
            !matches!(i.status, DownloadStatus::Complete | DownloadStatus::Failed(_))
        });
    }
}

impl EventEmitter<DismissEvent> for DownloadManagerView {}

impl Focusable for DownloadManagerView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DownloadManagerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let items = self.items.clone();
        let active_count = self.active_count();

        v_flex()
            .id("download-manager-panel")
            .w(px(400.))
            .max_h(px(520.))
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .shadow_lg()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.clear_completed();
                if this.items.is_empty() {
                    cx.emit(DismissEvent);
                }
            }))
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
                                Icon::new(IconName::Download)
                                    .size(px(14.))
                                    .text_color(theme.accent),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child(if active_count > 0 {
                                        format!("Downloads ({})", active_count)
                                    } else {
                                        "Downloads".to_string()
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("dm-clear")
                                    .label("Clear")
                                    .compact()
                                    .ghost()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.clear_completed();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("dm-close")
                                    .icon(IconName::Close)
                                    .compact()
                                    .ghost()
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(DismissEvent);
                                    })),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .id("dm-items-list")
                    .w_full()
                    .flex_1()
                    .scrollable(ScrollbarAxis::Vertical)
                    .when(items.is_empty(), |this| {
                        this.child(
                            v_flex()
                                .w_full()
                                .items_center()
                                .justify_center()
                                .py_10()
                                .gap_2()
                                .child(
                                    Icon::new(IconName::Download)
                                        .size(px(20.))
                                        .text_color(theme.muted_foreground),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("No downloads"),
                                ),
                        )
                    })
                    .children(items.into_iter().map(|item| render_download_item(item, cx))),
            )
    }
}

fn render_download_item(item: DownloadItem, cx: &mut Context<DownloadManagerView>) -> impl IntoElement {
    let theme = cx.theme();
    let label = item.label();
    let is_downloading = matches!(item.status, DownloadStatus::Downloading { .. });
    let progress = item.progress_fraction();
    let downloaded_text = item.downloaded_display();
    let speed_text = item.speed_display();

    v_flex()
        .w_full()
        .p_3()
        .gap_1p5()
        .border_b_1()
        .border_color(theme.border.opacity(0.5))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Icon::new(match &item.kind {
                                DownloadKind::EngineVersion { .. } => IconName::Package,
                                DownloadKind::TemplateClone { .. } => IconName::Folder,
                            })
                            .size(px(14.))
                            .text_color(theme.muted_foreground),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.foreground)
                                .child(label),
                        ),
                )
                .child(match &item.status {
                    DownloadStatus::Downloading { speed_bps, .. } => {
                        if *speed_bps > 0 {
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(speed_text)
                                .into_any_element()
                        } else {
                            div()
                                .text_xs()
                                .text_color(theme.accent)
                                .child("Starting...")
                                .into_any_element()
                        }
                    }
                    DownloadStatus::Complete => {
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                Icon::new(IconName::Check)
                                    .size(px(12.))
                                    .text_color(theme.success),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.success)
                                    .child("Done"),
                            )
                            .into_any_element()
                    }
                    DownloadStatus::Failed(e) => {
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                Icon::new(IconName::Close)
                                    .size(px(12.))
                                    .text_color(theme.danger),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.danger)
                                    .child(e.clone()),
                            )
                            .into_any_element()
                    }
                }),
        )
        .when(is_downloading, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(downloaded_text),
            )
            .child(
                div()
                    .w_full()
                    .h(px(3.))
                    .rounded_full()
                    .bg(theme.border)
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .rounded_full()
                            .bg(theme.accent)
                            .w(relative(progress)),
                    ),
            )
        })
}
