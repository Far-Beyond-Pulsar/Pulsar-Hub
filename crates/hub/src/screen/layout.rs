use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Icon, IconName, StyledExt, TitleBar,
};

use crate::core::types::EntryScreenView;
use crate::screen::views::download_manager::DownloadManagerView;
use crate::screen::EntryScreen;
use wgpui_motion::prelude::{MotionExt as _, MotionToken, Transition};

pub fn render_layout(
    screen: &mut EntryScreen,
    window: &mut Window,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    if screen.state.ui.show_onboarding {
        return crate::screen::views::render_onboarding(screen, window, cx).into_any_element();
    }

    if screen.state.ui.show_git_upstream_prompt.is_some() {
        return crate::screen::views::render_upstream_prompt(screen, window, cx).into_any_element();
    }

    if screen.state.ui.engine_prompt.is_some() {
        return crate::screen::views::engine_install_prompt::render_engine_install_prompt(
            screen, cx,
        )
        .into_any_element();
    }

    if screen.state.ui.release_notes_modal.is_some() {
        return crate::screen::views::release_notes_modal::render_release_notes_modal(
            screen, window, cx,
        )
        .into_any_element();
    }

    if screen.state.ui.building_src {
        return crate::screen::views::render_src_build_overlay(screen, cx).into_any_element();
    }

    let view = screen.state.ui.view;
    let active_downloads = screen.state.download_manager_view.read(cx).active_count();
    let accent_color = cx.theme().accent;
    let bg_color = cx.theme().background;

    v_flex()
        .size_full()
        .relative()
        .bg(bg_color)
        .child(
            h_flex()
                .flex_1()
                .w_full()
                .overflow_hidden()
                .child(crate::screen::views::render_sidebar(screen, cx))
                .child(
                    v_flex()
                        .flex_1()
                        .h_full()
                        .overflow_hidden()
                        .bg(bg_color)
                        .child(TitleBar::new().child(div().flex_1()).child({
                            let theme_picker = screen.state.theme_picker.clone();
                            let dm_view = screen.state.download_manager_view.clone();
                            h_flex()
                                .flex()
                                .items_center()
                                .px_2()
                                .gap_2()
                                .child({
                                    let has_active = active_downloads > 0;
                                    ui::popover::Popover::<DownloadManagerView>::new(
                                        "titlebar-downloads-popover",
                                    )
                                    .anchor(Corner::TopRight)
                                    .trigger(
                                        Button::new("titlebar-downloads")
                                            .icon(IconName::Download)
                                            .compact()
                                            .ghost()
                                            .tooltip("Downloads")
                                            .when(has_active, |this| {
                                                this.child(
                                                    div()
                                                        .ml_1()
                                                        .px_1()
                                                        .rounded_full()
                                                        .bg(accent_color)
                                                        .text_xs()
                                                        .text_color(bg_color)
                                                        .font_weight(gpui::FontWeight::BOLD)
                                                        .child(active_downloads.to_string()),
                                                )
                                            }),
                                    )
                                    .content(move |_, _| dm_view.clone())
                                })
                                .child(
                                    ui::popover::Popover::<ui_shell::ThemePicker>::new(
                                        "titlebar-theme-popover",
                                    )
                                    .anchor(Corner::TopRight)
                                    .trigger(
                                        Button::new("titlebar-theme-toggle")
                                            .icon(IconName::Palette)
                                            .compact()
                                            .ghost()
                                            .tooltip("Switch theme"),
                                    )
                                    .content(move |_, _| theme_picker.clone()),
                                )
                        }))
                        .child(match view {
                            EntryScreenView::Home => {
                                let bounds = window.viewport_size();
                                let width: f32 = f32::from(bounds.width);
                                let available_width: f32 = (width - 220.0 - 64.0).max(0.0);
                                v_flex()
                                    .flex_1()
                                    .min_h_0()
                                    .id("home-scroll")
                                    .scrollable(gpui::Axis::Vertical)
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .child(crate::screen::views::render_home_banner(
                                                screen, cx,
                                            ))
                                            .child(crate::screen::views::render_recent_projects(
                                                screen,
                                                available_width,
                                                cx,
                                            )),
                                    )
                                    .into_any_element()
                            }
                            EntryScreenView::Templates => {
                                let bounds = window.viewport_size();
                                let width: f32 = f32::from(bounds.width);
                                let available_width: f32 = (width - 220.0 - 64.0).max(0.0);
                                crate::screen::views::render_templates(screen, available_width, cx)
                                    .into_any_element()
                            }
                            EntryScreenView::Storage => {
                                crate::screen::views::render_storage(screen, cx).into_any_element()
                            }
                            EntryScreenView::Versions => {
                                if screen.state.version_detail.is_some() {
                                    crate::screen::views::version_detail::render_version_detail(
                                        screen, cx,
                                    )
                                    .into_any_element()
                                } else {
                                    crate::screen::views::render_versions(screen, window, cx)
                                        .into_any_element()
                                }
                            }
                            EntryScreenView::CloudProjects => {
                                crate::screen::views::render_cloud_projects(screen, window, cx)
                                    .into_any_element()
                            }
                            EntryScreenView::Friends => {
                                screen.state.friends_screen.clone().into_any_element()
                            }
                        }),
                ),
        )
        .when(screen.state.ui.show_cloud_intro_modal, |this| {
            this.child(crate::screen::views::render_cloud_intro_modal(screen, cx))
        })
        .when(screen.state.ui.show_new_project_modal, |this| {
            this.child(crate::screen::views::render_new_project_modal(screen, cx))
        })
        .when(screen.state.ui.show_clone_git_modal, |this| {
            this.child(crate::screen::views::render_clone_git_modal(screen, cx))
        })
        .when(
            view == EntryScreenView::Versions && screen.state.ui.show_install_modal,
            |this| {
                let theme = cx.theme();
                this.child(
                    v_flex()
                        .id("install-engine-modal-backdrop")
                        .absolute()
                        .inset_0()
                        .items_center()
                        .justify_center()
                        .p_6()
                        .bg(theme.background.opacity(0.28))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.state.ui.show_install_modal = false;
                                cx.notify();
                            }),
                        )
                        .child(
                            v_flex()
                                .id("install-engine-modal")
                                .w(px(760.))
                                .max_w_full()
                                .h(px(620.))
                                .max_h_full()
                                .overflow_hidden()
                                .rounded_xl()
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.popover.opacity(0.94))
                                .shadow_xl()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .motion("install-engine-modal-motion")
                                .transition(Transition::token(MotionToken::SpatialFast))
                                .enter(|style| style.opacity(0.).mt(px(10.)))
                                .base(|style| style.opacity(1.).mt(px(0.)))
                                .child(
                                    h_flex()
                                        .w_full()
                                        .px_5()
                                        .py_4()
                                        .items_center()
                                        .border_b_1()
                                        .border_color(theme.border)
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .child(
                                                    div()
                                                        .text_lg()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(theme.foreground)
                                                        .child("Install Engine"),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .child("Choose a release for this device"),
                                                ),
                                        )
                                        .child(
                                            Button::new("close-install-modal")
                                                .icon(IconName::Close)
                                                .compact()
                                                .ghost()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.state.ui.show_install_modal = false;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                .child(div().flex_1().min_h_0().p_4().child(
                                    crate::screen::views::versions::render_install_panel(
                                        screen, cx, true,
                                    ),
                                )),
                        ),
                )
            },
        )
        .when(screen.state.ui.show_dependency_setup, |this| {
            this.child(crate::screen::views::render_dependency_setup(
                screen, window, cx,
            ))
        })
        .into_any_element()
}
