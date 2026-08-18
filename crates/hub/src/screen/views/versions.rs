use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants as _},
    h_flex, scroll::ScrollbarAxis, spinner::Spinner, v_flex, ActiveTheme as _, Icon, IconName,
    StyledExt as _,
};

use crate::core::types::format_bytes;
use crate::service::installer_service;
use crate::EntryScreen;

pub fn render_versions(
    screen: &mut EntryScreen,
    window: &mut Window,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    if screen.state.ui.show_install_modal {
        return render_install_modal(screen, window, cx).into_any_element();
    }

    render_installed_grid(screen, cx).into_any_element()
}

fn render_installed_grid(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let installed = screen.state.versions.installed.clone();

    v_flex()
        .size_full()
        .p_6()
        .gap_4()
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    v_flex()
                        .child(
                            div()
                                .text_xl()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.foreground)
                                .child("Engine Versions"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(format!("{} installed", installed.len())),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("btn-refresh")
                                .label("Refresh")
                                .icon(IconName::Refresh)
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.refresh_versions(cx);
                                })),
                        )
                        .child(
                            Button::new("btn-install")
                                .label("Install")
                                .icon(IconName::Download)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state.ui.show_install_modal = true;
                                    if this.state.versions.available_releases.is_empty() {
                                        this.refresh_versions(cx);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("btn-add-src")
                                .label("Add src")
                                .icon(IconName::Folder)
                                .ghost()
                                .tooltip("Add a local engine source checkout as the 'src' engine version")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.prompt_add_src(cx);
                                })),
                        ),
                ),
        )
        .child(div().w_full().h(px(1.0)).bg(theme.border))
        .child(
            v_flex()
                .id("installed-versions-list")
                .w_full()
                .flex_1()
                .scrollable(ScrollbarAxis::Vertical)
                .gap_3()
                .when(installed.is_empty(), |this| {
                    this.child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .justify_center()
                            .py_12()
                            .gap_3()
                            .child(Spinner::new().color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Click \"Install\" to get started"),
                            ),
                    )
                })
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_3()
                        .children(installed.iter().enumerate().map(|(idx, ver)| {
                    let version = ver.metadata.version.clone();
                    let date = ver.metadata.install_date.clone();
                    let size = format_bytes(ver.disk_size_bytes);
                    let path = ver.metadata.install_path.clone();
                    let path_clone = path.clone();

                    v_flex()
                        .id(format!("version-card-{}", idx))
                        .w(px(280.))
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border)
                        .hover(|this| this.bg(theme.accent.opacity(0.05)))
                        .gap_3()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Icon::new(IconName::Package)
                                        .size(px(18.))
                                        .text_color(theme.accent),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(theme.foreground)
                                        .child(format!("v{}", version)),
                                ),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_md()
                                        .bg(theme.accent.opacity(0.12))
                                        .text_color(theme.accent)
                                        .child(size),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(if date.is_empty() {
                                            "Unknown date".to_string()
                                        } else {
                                            date
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(path.display().to_string()),
                        )
                        .child(div().w_full().h(px(1.0)).bg(theme.border))
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .child({
                                            let p = path_clone.clone();
                                            let src = version.eq_ignore_ascii_case("src");
                                            Button::new(format!("launch-{}", idx))
                                                .label("Launch")
                                                .icon(IconName::Play)
                                                .compact()
                                                .ghost()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    if src {
                                                        this.launch_src_standalone(p.clone(), cx);
                                                    } else {
                                                        let p = p.clone();
                                                        cx.spawn(async move |_, _| {
                                                            if let Err(e) =
                                                                installer_service::launch_engine(
                                                                    &p,
                                                                )
                                                            {
                                                                tracing::error!(
                                                                    "Launch failed: {}",
                                                                    e
                                                                );
                                                            }
                                                        })
                                                        .detach();
                                                    }
                                                }))
                                        })
                                        .child({
                                            let p = path_clone.clone();
                                            Button::new(format!("folder-{}", idx))
                                                .icon(IconName::FolderOpen)
                                                .compact()
                                                .ghost()
                                                .tooltip("Open folder")
                                                .on_click(move |_, _, _| {
                                                    installer_service::open_install_dir(&p);
                                                })
                                        })
                                        .child({
                                            let v = version.clone();
                                            Button::new(format!("remove-{}", idx))
                                                .icon(IconName::Trash)
                                                .compact()
                                                .ghost()
                                                .tooltip("Remove engine")
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.remove_version(&v, cx);
                                                }))
                                        }),
                                )
                                .child({
                                    let v = version.clone();
                                    Button::new(format!("info-{}", idx))
                                        .icon(IconName::Settings)
                                        .compact()
                                        .ghost()
                                        .tooltip("View release notes")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.open_release_notes(v.clone(), cx);
                                        }))
                                }),
                        )
                                .into_any_element()
                    })),
                ),
        )
}

fn render_install_modal(
    screen: &mut EntryScreen,
    window: &mut Window,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();

    v_flex()
        .size_full()
        .p_6()
        .gap_4()
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            Button::new("btn-back")
                                .icon(IconName::ArrowLeft)
                                .compact()
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state.ui.show_install_modal = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.foreground)
                                .child("Install Engine"),
                        ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            screen
                                .channel_menu
                                .as_ref()
                                .cloned()
                                .map(|menu| {
                                    ui::popover::Popover::<
                                        crate::screen::views::channel_menu::ChannelMenuView,
                                    >::new("install-channels-popover")
                                        .anchor(Corner::TopRight)
                                        .trigger(
                                            Button::new("btn-channels")
                                                .label("Channels")
                                                .icon(IconName::Settings)
                                                .compact()
                                                .ghost()
                                                .tooltip("Select release channels"),
                                        )
                                        .content(move |_, _| menu.clone())
                                        .into_any_element()
                                })
                                .unwrap_or_else(|| div().into_any_element()),
                        )
                        .child(
                            Button::new("btn-debug-install-all")
                                .label("Debug: install all")
                                .icon(IconName::Package)
                                .compact()
                                .ghost()
                                .tooltip("Install every available engine version (debug)")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.install_all_versions(cx);
                                })),
                        )
                        .child(
                            Button::new("btn-refresh-releases")
                                .label("Refresh")
                                .icon(IconName::Refresh)
                                .compact()
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.refresh_versions(cx);
                                })),
                        ),
                ),
        )
        .child(div().w_full().h(px(1.0)).bg(theme.border))
        .child(
            v_flex()
                .id("install-releases-list")
                .w_full()
                .flex_1()
                .px_1()
                .child(
                    screen
                        .release_list
                        .as_ref()
                        .map(|list| list.clone().into_any_element())
                        .unwrap_or_else(|| div().into_any_element()),
                ),
        )
}

