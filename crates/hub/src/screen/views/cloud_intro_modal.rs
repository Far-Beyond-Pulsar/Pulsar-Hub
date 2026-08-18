use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Icon, IconName,
};

use crate::screen::EntryScreen;

pub fn render_cloud_intro_modal(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let current_page = screen.state.ui.cloud_intro_page;

    div()
        .id("cloud-intro-modal-overlay")
        .absolute()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.background.opacity(0.60))
        .p_6()
        .child(
            v_flex()
                .id("cloud-intro-modal")
                .w_full()
                .max_w(px(540.))
                .rounded_2xl()
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .shadow_xl()
                .overflow_hidden()
                // Top header
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .px_6()
                        .pt_6()
                        .pb_4()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            h_flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .w(px(38.))
                                        .h(px(38.))
                                        .rounded_xl()
                                        .bg(theme.accent.opacity(0.15))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            Icon::new(IconName::Cloud)
                                                .size(px(20.))
                                                .text_color(theme.accent),
                                        ),
                                )
                                .child(
                                    v_flex()
                                        .child(
                                            div()
                                                .text_base()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme.foreground)
                                                .child("Pulsar Studio"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.secondary_foreground)
                                                .child("Cloud Collaboration & Remote Infrastructure"),
                                        ),
                                ),
                        )
                        .child(
                            Button::new("cloud-intro-close-btn")
                                .icon(IconName::Close)
                                .ghost()
                                .compact()
                                .tooltip("Close")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_cloud_intro_modal(cx);
                                })),
                        ),
                )
                // Modal body
                .child(
                    v_flex()
                        .w_full()
                        .px_6()
                        .py_5()
                        .gap_4()
                        .min_h(px(260.))
                        .child(match current_page {
                            0 => render_page_overview(cx).into_any_element(),
                            1 => render_page_realtime(cx).into_any_element(),
                            _ => render_page_git_ci(cx).into_any_element(),
                        }),
                )
                // Footer
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .px_6()
                        .py_4()
                        .bg(theme.secondary_foreground.opacity(0.04))
                        .border_t_1()
                        .border_color(theme.border)
                        .child(
                            // Page Indicators
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .children((0..3).map(|i| {
                                    let is_active = i == current_page;
                                    div()
                                        .id(SharedString::from(format!("cloud-intro-dot-{i}")))
                                        .w(if is_active { px(20.) } else { px(8.) })
                                        .h(px(8.))
                                        .rounded_full()
                                        .bg(if is_active {
                                            theme.accent
                                        } else {
                                            theme.secondary_foreground.opacity(0.30)
                                        })
                                        .cursor_pointer()
                                        .hover(|this| {
                                            this.bg(if is_active {
                                                theme.accent
                                            } else {
                                                theme.secondary_foreground.opacity(0.60)
                                            })
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.set_cloud_intro_page(i, cx);
                                        }))
                                })),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .when(current_page > 0, |this| {
                                    this.child(
                                        Button::new("cloud-intro-prev-btn")
                                            .label("Back")
                                            .ghost()
                                            .compact()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.prev_cloud_intro_page(cx);
                                            })),
                                    )
                                })
                                .when(current_page < 2, |this| {
                                    this.child(
                                        Button::new("cloud-intro-next-btn")
                                            .label("Next")
                                            .primary()
                                            .compact()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.next_cloud_intro_page(cx);
                                            })),
                                    )
                                })
                                .when(current_page == 2, |this| {
                                    this.child(
                                        Button::new("cloud-intro-finish-btn")
                                            .label("Get Started")
                                            .primary()
                                            .compact()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.close_cloud_intro_modal(cx);
                                            })),
                                    )
                                }),
                        ),
                ),
        )
}

fn render_page_overview(cx: &Context<EntryScreen>) -> impl IntoElement {
    let theme = cx.theme();

    v_flex()
        .gap_3()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.accent)
                .child("WELCOME TO PULSAR STUDIO"),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.foreground)
                .child("Pulsar Studio powers remote collaboration, dedicated server hosting, and distributed workflows for Pulsar Engine."),
        )
        .child(
            v_flex()
                .p_4()
                .gap_2()
                .rounded_xl()
                .bg(theme.accent.opacity(0.08))
                .border_1()
                .border_color(theme.accent.opacity(0.30))
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::Heart)
                                .size(px(16.))
                                .text_color(theme.accent),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground)
                                .child("How We Fund Pulsar Engine"),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.secondary_foreground)
                        .child("Pulsar Studio is the only paid part of the engine and the primary way we sustainably fund ongoing development. The core Pulsar Engine remains 100% free and open-source for everyone."),
                ),
        )
}

fn render_page_realtime(cx: &Context<EntryScreen>) -> impl IntoElement {
    let theme = cx.theme();

    v_flex()
        .gap_3()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.accent)
                .child("REAL-TIME COLLABORATION"),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.foreground)
                .child("Store your project state on a remote server where anyone on your team can connect and edit simultaneously in real time."),
        )
        .child(
            v_flex()
                .gap_2p5()
                .child(
                    h_flex()
                        .p_3()
                        .gap_3()
                        .items_center()
                        .rounded_lg()
                        .bg(theme.secondary_foreground.opacity(0.05))
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .w(px(32.))
                                .h(px(32.))
                                .rounded_lg()
                                .bg(theme.accent.opacity(0.12))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(IconName::Group)
                                        .size(px(16.))
                                        .text_color(theme.accent),
                                ),
                        )
                        .child(
                            v_flex()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child("Multi-User Live Editing"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.secondary_foreground)
                                        .child("Team members see each other’s edits and changes instantly."),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .p_3()
                        .gap_3()
                        .items_center()
                        .rounded_lg()
                        .bg(theme.secondary_foreground.opacity(0.05))
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .w(px(32.))
                                .h(px(32.))
                                .rounded_lg()
                                .bg(theme.accent.opacity(0.12))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(IconName::Globe)
                                        .size(px(16.))
                                        .text_color(theme.accent),
                                ),
                        )
                        .child(
                            v_flex()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child("Dedicated Cloud State"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.secondary_foreground)
                                        .child("Centralized remote servers keep assets and scene states perfectly in sync."),
                                ),
                        ),
                ),
        )
}

fn render_page_git_ci(cx: &Context<EntryScreen>) -> impl IntoElement {
    let theme = cx.theme();

    v_flex()
        .gap_3()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.accent)
                .child("GIT INTEGRATION & CI AUTOMATION"),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.foreground)
                .child("Designed for professional teams with full version control integration and automated pipelines."),
        )
        .child(
            v_flex()
                .gap_2p5()
                .child(
                    h_flex()
                        .p_3()
                        .gap_3()
                        .items_center()
                        .rounded_lg()
                        .bg(theme.secondary_foreground.opacity(0.05))
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .w(px(32.))
                                .h(px(32.))
                                .rounded_lg()
                                .bg(theme.accent.opacity(0.12))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(IconName::GitBranch)
                                        .size(px(16.))
                                        .text_color(theme.accent),
                                ),
                        )
                        .child(
                            v_flex()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child("Version Control via Git"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.secondary_foreground)
                                        .child("Native Git support for branching, tracking changes, and upstream syncing."),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .p_3()
                        .gap_3()
                        .items_center()
                        .rounded_lg()
                        .bg(theme.secondary_foreground.opacity(0.05))
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .w(px(32.))
                                .h(px(32.))
                                .rounded_lg()
                                .bg(theme.accent.opacity(0.12))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(IconName::Cpu)
                                        .size(px(16.))
                                        .text_color(theme.accent),
                                ),
                        )
                        .child(
                            v_flex()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child("Automated CI & Build Pipelines"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.secondary_foreground)
                                        .child("Run continuous integration, automated builds, tests, and multi-platform packaging."),
                                ),
                        ),
                ),
        )
}
