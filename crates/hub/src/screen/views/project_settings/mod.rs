pub mod disk_info;
pub mod general;
pub mod git_ci;
pub mod git_info;
pub mod helpers;
pub mod integrations;
pub mod metadata;
pub mod performance;
pub mod types;

pub use types::*;

use gpui::prelude::*;
use gpui::*;
use ui::{
    button::Button, button::ButtonVariants as _, h_flex, scroll::ScrollbarAxis, v_flex,
    ActiveTheme as _, Icon, IconName,
    StyledExt as _,
};

use crate::screen::EntryScreen;

pub fn render_project_settings(
    screen: &mut EntryScreen,
    _window: &mut Window,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let Some(ref settings) = screen.state.ui.project_settings else {
        return div().into_any_element();
    };
    let active_tab = settings.active_tab.clone();
    let project_name = settings.project_name.clone();
    let project_path = settings.project_path.clone();

    div()
        .id("project-settings-modal")
        .absolute()
        .size_full()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.background.opacity(0.86))
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
        })
        .child(
            v_flex()
                .w_full()
                .max_w(px(820.))
                .h(relative(0.86))
                .p_0()
                .rounded_xl()
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .shadow_lg()
                .overflow_hidden()
                .child(
                    h_flex()
                        .w_full()
                        .px_5()
                        .py_4()
                        .gap_3()
                        .items_center()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            v_flex()
                                .flex_1()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child("Project Settings"),
                                )
                                .child(div().text_sm().text_color(theme.muted_foreground).child(
                                    format!(
                                        "{} \u{2014} {}",
                                        project_name,
                                        project_path.to_string_lossy()
                                    ),
                                )),
                        )
                        .child(
                            Button::new("close-project-settings")
                                .compact()
                                .ghost()
                                .icon(IconName::Close)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_project_settings(cx);
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        .items_stretch()
                        .overflow_hidden()
                        .child(
                            v_flex()
                                .id("project-settings-sidebar-scroll")
                                .w(px(208.))
                                .h_full()
                                .min_h_0()
                                .self_stretch()
                                .flex_shrink_0()
                                .border_r_1()
                                .border_color(theme.border)
                                .bg(theme.secondary.opacity(0.035))
                                .p_2()
                                .gap_1()
                                .overflow_hidden()
                                .scrollable(ScrollbarAxis::Vertical)
                                .child(tab_button(
                                    "ps-general",
                                    IconName::Settings,
                                    "General",
                                    active_tab == ProjectSettingsTab::General,
                                    ProjectSettingsTab::General,
                                    cx,
                                ))
                                .child(tab_button(
                                    "ps-git-info",
                                    IconName::GitBranch,
                                    "Git Info",
                                    active_tab == ProjectSettingsTab::GitInfo,
                                    ProjectSettingsTab::GitInfo,
                                    cx,
                                ))
                                .child(tab_button(
                                    "ps-git-ci",
                                    IconName::Github,
                                    "Git CI/CD",
                                    active_tab == ProjectSettingsTab::GitCI,
                                    ProjectSettingsTab::GitCI,
                                    cx,
                                ))
                                .child(tab_button(
                                    "ps-metadata",
                                    IconName::BookOpen,
                                    "Metadata",
                                    active_tab == ProjectSettingsTab::Metadata,
                                    ProjectSettingsTab::Metadata,
                                    cx,
                                ))
                                .child(tab_button(
                                    "ps-disk",
                                    IconName::HardDrive,
                                    "Disk Info",
                                    active_tab == ProjectSettingsTab::DiskInfo,
                                    ProjectSettingsTab::DiskInfo,
                                    cx,
                                ))
                                .child(tab_button(
                                    "ps-perf",
                                    IconName::Cpu,
                                    "Performance",
                                    active_tab == ProjectSettingsTab::Performance,
                                    ProjectSettingsTab::Performance,
                                    cx,
                                ))
                                .child(tab_button(
                                    "ps-integrations",
                                    IconName::Code,
                                    "Integrations",
                                    active_tab == ProjectSettingsTab::Integrations,
                                    ProjectSettingsTab::Integrations,
                                    cx,
                                )),
                        )
                        .child(
                            v_flex()
                                .id("project-settings-content-scroll")
                                .flex_1()
                                .h_full()
                                .min_w_0()
                                .min_h_0()
                                .self_stretch()
                                .overflow_hidden()
                                .scrollable(ScrollbarAxis::Vertical)
                                .p_4()
                                .child(
                                match active_tab {
                                    ProjectSettingsTab::General => {
                                        general::render_general_tab(screen, cx).into_any_element()
                                    }
                                    ProjectSettingsTab::GitInfo => {
                                        git_info::render_git_info_tab(screen, cx).into_any_element()
                                    }
                                    ProjectSettingsTab::GitCI => {
                                        git_ci::render_git_ci_tab(screen, cx).into_any_element()
                                    }
                                    ProjectSettingsTab::Metadata => {
                                        metadata::render_metadata_tab(screen, cx).into_any_element()
                                    }
                                    ProjectSettingsTab::DiskInfo => {
                                        disk_info::render_disk_info_tab(screen, cx)
                                            .into_any_element()
                                    }
                                    ProjectSettingsTab::Performance => {
                                        performance::render_performance_tab(screen, cx)
                                            .into_any_element()
                                    }
                                    ProjectSettingsTab::Integrations => {
                                        integrations::render_integrations_tab(screen, cx)
                                            .into_any_element()
                                    }
                                },
                            ),
                        ),
                ),
        )
        .into_any_element()
}

fn tab_button(
    id: &'static str,
    icon: IconName,
    label: &'static str,
    is_active: bool,
    tab: ProjectSettingsTab,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    h_flex()
        .id(id)
        .w_full()
        .px_3()
        .py_2()
        .rounded_md()
        .cursor_pointer()
        .when(is_active, |this| this.bg(theme.accent.opacity(0.12)))
        .hover(|this| this.bg(theme.accent.opacity(0.07)))
        .child(
            Icon::new(icon)
                .size(px(15.))
                .text_color(if is_active {
                    theme.accent
                } else {
                    theme.muted_foreground
                }),
        )
        .child(
            div()
                .text_sm()
                .font_weight(if is_active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_active {
                    theme.accent
                } else {
                    theme.muted_foreground
                })
                .child(label),
        )
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
            this.change_project_settings_tab(tab.clone(), cx);
        }))
}
