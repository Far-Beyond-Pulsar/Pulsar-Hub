use gpui::prelude::*;
use gpui::*;
use ui::{
    button::Button,
    button::ButtonVariants as _,
    h_flex,
    v_flex,
    ActiveTheme as _,
    Disableable as _,
    Icon,
    IconName,
    StyledExt as _,
};

use crate::screen::EntryScreen;
use crate::service::storage_service::{repo_health, ProjectDiskStats};
use crate::util::formatters::format_size;

pub fn render_storage(screen: &mut EntryScreen, cx: &mut Context<EntryScreen>) -> impl IntoElement {
    let theme = cx.theme();
    let loading = screen.state.storage.loading;
    let mut rows: Vec<ProjectDiskStats> = screen.state.storage.rows.clone();
    rows.sort_by(|a, b| b.total_bytes().cmp(&a.total_bytes()));

    let total_bytes: u64 = rows.iter().map(|r| r.total_bytes()).sum();
    let total_git: u64 = rows.iter().map(|r| r.git_bytes).sum();

    v_flex()
        .flex_1()
        .h_full()
        .overflow_hidden()
        .child(
            h_flex()
                .w_full()
                .px_8()
                .pt_6()
                .pb_4()
                .gap_3()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child("Storage"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "{} across {} projects \u{b7} {:.0}% git history",
                            format_size(total_bytes),
                            rows.len(),
                            if total_bytes > 0 {
                                100.0 * total_git as f32 / total_bytes as f32
                            } else {
                                0.0
                            }
                        )),
                )
                .child(
                    Button::new("storage-refresh")
                        .icon(IconName::Refresh)
                        .label("Refresh")
                        .compact()
                        .disabled(loading)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_storage(cx);
                        })),
                ),
        )
        .when(loading && rows.is_empty(), |this| {
            this.child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::HardDrive)
                            .size(px(40.))
                            .text_color(theme.muted_foreground.opacity(0.4)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Measuring project sizes\u{2026}"),
                    ),
            )
        })
        .when(!loading && rows.is_empty(), |this| {
            this.child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::HardDrive)
                            .size(px(40.))
                            .text_color(theme.muted_foreground.opacity(0.4)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("No projects to measure yet"),
                    ),
            )
        })
        .when(!rows.is_empty(), |this| {
            this.child(
                v_flex()
                    .id("storage-scroll")
                    .flex_1()
                    .min_h_0()
                    .scrollable(Axis::Vertical)
                    .px_8()
                    .pb_6()
                    .gap_2()
                    .children(rows.iter().map(|row| render_storage_row(row, cx))),
            )
        })
}

fn render_storage_row(
    row: &ProjectDiskStats,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let total = row.total_bytes();
    let (health_label, health_kind) = repo_health(row);
    let path_open = row.path.clone();
    let name = std::path::Path::new(&row.path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| row.path.clone());

    let health_color = match health_kind {
        "success" => theme.success,
        "warning" => theme.warning,
        "danger" => gpui::red(),
        _ => theme.muted_foreground,
    };

    let working_pct = if total > 0 {
        100.0 * row.working_bytes as f32 / total as f32
    } else {
        100.0
    };

    h_flex()
        .w_full()
        .gap_4()
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(theme.secondary.opacity(0.06))
        .child(
            v_flex()
                .w(px(220.))
                .min_w(px(220.))
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .truncate()
                        .child(name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(row.path.clone()),
                ),
        )
        // Size bar: working vs .git split.
        .child(
            v_flex()
                .flex_1()
                .gap_1()
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.foreground)
                                .child(format!("{}", format_size(total))),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "{} files \u{b7} {} git",
                                    format_size(row.working_bytes),
                                    format_size(row.git_bytes),
                                )),
                        ),
                )
                .child(
                    h_flex()
                        .w_full()
                        .h(px(6.))
                        .rounded_full()
                        .overflow_hidden()
                        .bg(theme.secondary.opacity(0.35))
                        .child(
                            div()
                                .w(relative(working_pct / 100.0))
                                .h_full()
                                .bg(theme.accent),
                        )
                        .child(div().flex_1().h_full().bg(theme.warning.opacity(0.7))),
                ),
        )
        .child(
            div()
                .px_2()
                .py_0p5()
                .rounded_md()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(health_color)
                .bg(health_color.opacity(0.12))
                .child(health_label),
        )
        .child(
            Button::new(SharedString::from(format!("storage-open-{}", row.path)))
                .icon(IconName::FolderOpen)
                .compact()
                .ghost()
                .tooltip("Open folder")
                .on_click(move |_, _, _| {
                    let _ = open::that(&path_open);
                }),
        )
}
