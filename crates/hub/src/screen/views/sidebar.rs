use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use ui::{
    button::Button, button::ButtonVariants as _, h_flex, v_flex, ActiveTheme as _, Icon, IconName,
};
use wgpui_motion::prelude::*;

use crate::core::events::{FabSearchRequested, SettingsRequested};
use crate::core::types::{EntryScreenView, OnboardingStep};
use crate::screen::EntryScreen;

pub fn render_sidebar(screen: &mut EntryScreen, cx: &mut Context<EntryScreen>) -> impl IntoElement {
    let theme = cx.theme();
    let accent = theme.accent;
    let foreground = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let primary = theme.primary;
    let primary_fg = theme.primary_foreground;
    let profile = engine_state::EngineContext::global().and_then(|engine| engine.auth_profile());
    let (account_name, account_detail) = match profile {
        Some(profile) => (
            profile
                .display_name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or(profile.login),
            "Pulsar account",
        ),
        None => ("Account".to_string(), "Sign in to connect"),
    };
    let indicator_bounds = screen
        .state
        .ui
        .sidebar_nav_bounds
        .lock()
        .get(&screen.state.ui.view)
        .copied();

    let accent_bg = accent.opacity(0.12);
    let hover_bg = accent.opacity(0.07);

    let is_home = screen.state.ui.view == EntryScreenView::Home;
    let is_templates = screen.state.ui.view == EntryScreenView::Templates;
    let is_storage = screen.state.ui.view == EntryScreenView::Storage;
    let is_versions = screen.state.ui.view == EntryScreenView::Versions;
    let is_cloud = screen.state.ui.view == EntryScreenView::CloudProjects;
    let is_friends = screen.state.ui.view == EntryScreenView::Friends;

    v_flex()
        .relative()
        .w(px(220.))
        .h_full()
        .flex_shrink_0()
        .overflow_y_hidden()
        .bg(theme.sidebar.opacity(0.96))
        .border_r_1()
        .border_color(border)
        .when_some(indicator_bounds, |this, (x, y, width, height)| {
            this.child(
                div()
                    .id("sidebar-selection-indicator")
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(width))
                    .h(px(height))
                    .rounded_lg()
                    .border_1()
                    .border_color(border)
                    .bg(accent_bg)
                    .motion("sidebar-selection-indicator-motion")
                    .transition(Transition::token(MotionToken::SpatialNormal)),
            )
        })
        .child(
            h_flex()
                .w_full()
                .px_4()
                .pt_5()
                .pb_4()
                .gap_3()
                .items_center()
                .child(
                    div()
                        .flex_shrink_0()
                        .w(px(44.))
                        .h(px(44.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_lg()
                        .when(screen.state.logo.is_none(), |this| this.bg(primary))
                        .when_some(screen.state.logo.clone(), |this, logo| {
                            this.child(
                                img(ImageSource::Render(logo))
                                    .w(px(58.))
                                    .h(px(58.))
                                    .object_fit(gpui::ObjectFit::Contain),
                            )
                        })
                        .when(screen.state.logo.is_none(), |this| {
                            this.child(
                                Icon::new(IconName::Star)
                                    .size(px(18.))
                                    .text_color(primary_fg),
                            )
                        }),
                )
                .child(
                    v_flex()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(foreground)
                                .child("Pulsar Engine"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted_fg)
                                .child("Project Manager"),
                        ),
                ),
        )
        .child(div().w_full().h(px(1.0)).bg(border))
        .child(
            v_flex()
                .w_full()
                .px_3()
                .pt_4()
                .pb_2()
                .gap_0p5()
                .on_children_prepainted({
                    let positions = screen.state.ui.sidebar_nav_bounds.clone();
                    let weak = cx.entity().downgrade();
                    move |bounds, _, cx| {
                        capture_nav_bounds(
                            bounds,
                            1,
                            &[
                                EntryScreenView::Home,
                                EntryScreenView::CloudProjects,
                                EntryScreenView::Templates,
                                EntryScreenView::Storage,
                            ],
                            &positions,
                            &weak,
                            cx,
                        );
                    }
                })
                .child(
                    div()
                        .text_xs()
                        .h(px(22.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(muted_fg)
                        .px_2()
                        .pb_0()
                        .child("PROJECTS"),
                )
                .child(nav_item(
                    "nav-home",
                    IconName::LayoutDashboard,
                    "Home",
                    is_home,
                    accent,
                    foreground,
                    muted_fg,
                    accent_bg,
                    hover_bg,
                    cx.listener(|this, _, _, cx| {
                        this.state.ui.view = EntryScreenView::Home;
                        cx.notify();
                    }),
                ))
                .child(nav_item(
                    "nav-cloud",
                    IconName::Cloud,
                    "Cloud Projects",
                    is_cloud,
                    accent,
                    foreground,
                    muted_fg,
                    accent_bg,
                    hover_bg,
                    cx.listener(|this, _, _, cx| {
                        this.open_cloud_projects_view(cx);
                    }),
                ))
                .child(nav_item(
                    "nav-templates",
                    IconName::List,
                    "Templates",
                    is_templates,
                    accent,
                    foreground,
                    muted_fg,
                    accent_bg,
                    hover_bg,
                    cx.listener(|this, _, _, cx| {
                        this.state.ui.view = EntryScreenView::Templates;
                        cx.notify();
                    }),
                ))
                .child(nav_item(
                    "nav-storage",
                    IconName::Database,
                    "Storage",
                    is_storage,
                    accent,
                    foreground,
                    muted_fg,
                    accent_bg,
                    hover_bg,
                    cx.listener(|this, _, _, cx| {
                        this.open_storage_view(cx);
                    }),
                )),
        )
        .child(
            v_flex()
                .w_full()
                .px_3()
                .pt_2()
                .pb_2()
                .gap_0p5()
                .child(div().w_full().h(px(1.0)).bg(border).mb_2())
                .child(
                    div()
                        .text_xs()
                        .h(px(22.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(muted_fg)
                        .px_2()
                        .pb_0()
                        .child("ENGINE"),
                )
                .on_children_prepainted({
                    let positions = screen.state.ui.sidebar_nav_bounds.clone();
                    let weak = cx.entity().downgrade();
                    move |bounds, _, cx| {
                        capture_nav_bounds(
                            bounds,
                            2,
                            &[EntryScreenView::Versions],
                            &positions,
                            &weak,
                            cx,
                        );
                    }
                })
                .child(nav_item(
                    "nav-versions",
                    IconName::HardDrive,
                    "Versions",
                    is_versions,
                    accent,
                    foreground,
                    muted_fg,
                    accent_bg,
                    hover_bg,
                    cx.listener(|this, _, _, cx| {
                        this.state.ui.view = EntryScreenView::Versions;
                        cx.notify();
                    }),
                )),
        )
        .child(
            v_flex()
                .w_full()
                .px_3()
                .pt_2()
                .pb_2()
                .gap_0p5()
                .child(div().w_full().h(px(1.0)).bg(border).mb_2())
                .child(
                    div()
                        .text_xs()
                        .h(px(22.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(muted_fg)
                        .px_2()
                        .pb_0()
                        .child("SOCIAL"),
                )
                .on_children_prepainted({
                    let positions = screen.state.ui.sidebar_nav_bounds.clone();
                    let weak = cx.entity().downgrade();
                    move |bounds, _, cx| {
                        capture_nav_bounds(
                            bounds,
                            2,
                            &[EntryScreenView::Friends],
                            &positions,
                            &weak,
                            cx,
                        );
                    }
                })
                .child(
                    h_flex()
                        .id("nav-friends")
                        .w_full()
                        .gap_2p5()
                        .items_center()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .cursor_pointer()
                        .when(is_friends, |this| this.bg(gpui::hsla(0., 0., 0., 0.)))
                        .hover(|this| this.bg(hover_bg))
                        .child(
                            Icon::new(IconName::Group)
                                .size(px(15.))
                                .text_color(if is_friends { foreground } else { muted_fg }),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(if is_friends {
                                    gpui::FontWeight::SEMIBOLD
                                } else {
                                    gpui::FontWeight::NORMAL
                                })
                                .text_color(if is_friends { foreground } else { muted_fg })
                                .child("Friends"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_right()
                                .text_xs()
                                .text_color(muted_fg)
                                .child("online"),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.state.ui.view = EntryScreenView::Friends;
                            cx.notify();
                        })),
                ),
        )
        .child(
            v_flex()
                .w_full()
                .px_3()
                .pt_2()
                .pb_2()
                .gap_0p5()
                .child(div().w_full().h(px(1.0)).bg(border).mb_2())
                .child(
                    div()
                        .text_xs()
                        .h(px(22.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(muted_fg)
                        .px_2()
                        .pb_0()
                        .child("DISCOVER"),
                )
                .child(
                    h_flex()
                        .id("nav-fab")
                        .w_full()
                        .gap_2p5()
                        .items_center()
                        .px_3()
                        .h(px(36.))
                        .rounded_lg()
                        .cursor_pointer()
                        .hover(|this| this.bg(hover_bg))
                        .child(
                            Icon::new(IconName::ShoppingBag)
                                .size(px(15.))
                                .text_color(muted_fg),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(muted_fg)
                                .child("FAB Marketplace"),
                        )
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(FabSearchRequested);
                        })),
                ),
        )
        .child(div().flex_1())
        .child(
            v_flex()
                .w_full()
                .px_3()
                .pb_4()
                .gap_0p5()
                .child(div().w_full().h(px(1.0)).bg(border).mb_2())
                .when_some(screen.state.auth.message.clone(), |this, msg| {
                    this.child(
                        div()
                            .px_3()
                            .pb_2()
                            .text_xs()
                            .text_color(muted_fg)
                            .child(msg),
                    )
                })
                .child(
                    h_flex()
                        .id("nav-open")
                        .w_full()
                        .gap_2p5()
                        .items_center()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .cursor_pointer()
                        .hover(|this| this.bg(hover_bg))
                        .child(
                            Icon::new(IconName::FolderOpen)
                                .size(px(15.))
                                .text_color(muted_fg),
                        )
                        .child(div().text_sm().text_color(muted_fg).child("Open Folder"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_folder_dialog(cx);
                        })),
                )
                .child(
                    h_flex()
                        .id("nav-deps")
                        .w_full()
                        .gap_2p5()
                        .items_center()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .cursor_pointer()
                        .hover(|this| this.bg(hover_bg))
                        .child(
                            Icon::new(IconName::Settings)
                                .size(px(15.))
                                .text_color(muted_fg),
                        )
                        .child(div().text_sm().text_color(muted_fg).child("Dependencies"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_onboarding_step(OnboardingStep::Dependencies, cx);
                        })),
                )
                .child(
                    h_flex()
                        .id("nav-settings")
                        .w_full()
                        .gap_2p5()
                        .items_center()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .cursor_pointer()
                        .hover(|this| this.bg(hover_bg))
                        .child(
                            Icon::new(IconName::Settings)
                                .size(px(15.))
                                .text_color(muted_fg),
                        )
                        .child(div().text_sm().text_color(muted_fg).child("Settings"))
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(SettingsRequested);
                        })),
                )
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .pt_3()
                        .mt_2()
                        .border_t_1()
                        .border_color(border)
                        .child(screen.state.auth.profile_dropdown.clone())
                        .child(
                            v_flex()
                                .flex_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(foreground)
                                        .child(account_name),
                                )
                                .child(div().text_xs().text_color(muted_fg).child(account_detail)),
                        ),
                ),
        )
        .child(div().h(px(12.)))
}

fn nav_item(
    id: &'static str,
    icon: IconName,
    label: &'static str,
    is_active: bool,
    _accent: gpui::Hsla,
    foreground: gpui::Hsla,
    muted_fg: gpui::Hsla,
    _accent_bg: gpui::Hsla,
    hover_bg: gpui::Hsla,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    h_flex()
        .id(id)
        .w_full()
        .gap_2p5()
        .items_center()
        .px_3()
        .h(px(36.))
        .rounded_lg()
        .cursor_pointer()
        .when(is_active, |this| this.bg(gpui::hsla(0., 0., 0., 0.)))
        .hover(|this| this.bg(hover_bg))
        .child(Icon::new(icon).size(px(15.)).text_color(if is_active {
            foreground
        } else {
            muted_fg
        }))
        .child(
            div()
                .text_sm()
                .font_weight(if is_active {
                    gpui::FontWeight::SEMIBOLD
                } else {
                    gpui::FontWeight::NORMAL
                })
                .text_color(if is_active { foreground } else { muted_fg })
                .child(label),
        )
        .on_click(on_click)
}

fn capture_nav_bounds(
    bounds: Vec<Bounds<Pixels>>,
    first_row_index: usize,
    views: &[EntryScreenView],
    positions: &Arc<
        parking_lot::Mutex<std::collections::HashMap<EntryScreenView, (f32, f32, f32, f32)>>,
    >,
    weak: &WeakEntity<EntryScreen>,
    cx: &mut App,
) {
    let changed = {
        let mut positions = positions.lock();
        let mut changed = false;
        for (index, view) in views.iter().enumerate() {
            let Some(row) = bounds.get(first_row_index + index) else {
                continue;
            };
            let next: (f32, f32, f32, f32) = (
                row.origin.x.into(),
                row.origin.y.into(),
                row.size.width.into(),
                row.size.height.into(),
            );
            let differs = positions
                .get(view)
                .is_none_or(|current: &(f32, f32, f32, f32)| {
                    (current.0 - next.0).abs() > 0.5
                        || (current.1 - next.1).abs() > 0.5
                        || (current.2 - next.2).abs() > 0.5
                        || (current.3 - next.3).abs() > 0.5
                });
            if differs {
                positions.insert(*view, next);
                changed = true;
            }
        }
        changed
    };
    if changed {
        let _ = weak.update(cx, |_, cx| cx.notify());
    }
}
