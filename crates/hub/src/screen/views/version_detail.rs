//! One engine version's page: its plugins (per install) and the themes (shared
//! by every install), with the registries they come from.

use gpui::prelude::*;
use gpui::*;
use ui::{
    ActiveTheme as _, Disableable as _, Icon, IconName, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    tab::{Tab, TabBar},
    v_flex,
};

use crate::EntryScreen;
use crate::component::registry_list::{RegistryKind, render_registry_panel};
use crate::core::types::VersionTab;
use crate::service::installer_service;

pub fn render_version_detail(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let Some(detail) = screen.state.version_detail.clone() else {
        return div().into_any_element();
    };
    let theme = cx.theme().clone();
    let is_src = detail.version.eq_ignore_ascii_case("src");
    let plugin_count = screen.state.installed_plugins.len();
    let theme_count = screen.state.installed_themes.len();
    let selected = match detail.tab {
        VersionTab::Plugins => 0,
        VersionTab::Themes => 1,
    };
    let labels: [SharedString; 2] = [
        if is_src {
            "Plugins".into()
        } else {
            format!("Plugins ({plugin_count})").into()
        },
        format!("Themes ({theme_count})").into(),
    ];

    let launch_path = detail.path.clone();
    let launch_version = detail.version.clone();

    // `flex_1` + `min_h_0`, not `size_full`: this sits under the title bar, and
    // 100% of the parent's height would run a title bar's height past the
    // window, cutting the bottom of every scroll area below.
    v_flex()
        .flex_1()
        .min_h_0()
        .w_full()
        .px_8()
        .pt_6()
        .pb_4()
        .gap_4()
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap_3()
                .child(
                    Button::new("version-detail-back")
                        .icon(IconName::ArrowLeft)
                        .compact()
                        .ghost()
                        .tooltip("Back to engine versions")
                        .on_click(cx.listener(|this, _, _, cx| this.close_version_detail(cx))),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.foreground)
                                .child(format!("Engine v{}", detail.version)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .truncate()
                                .child(detail.path.display().to_string()),
                        ),
                )
                .child(
                    Button::new("version-detail-launch")
                        .label("Launch")
                        .icon(IconName::Play)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if launch_version.eq_ignore_ascii_case("src") {
                                this.launch_src_standalone(launch_path.clone(), cx);
                            } else {
                                let path = launch_path.clone();
                                cx.spawn(async move |_, _| {
                                    if let Err(error) = installer_service::launch_engine(&path) {
                                        tracing::error!("Launch failed: {}", error);
                                    }
                                })
                                .detach();
                            }
                        })),
                ),
        )
        .child(
            TabBar::new("version-detail-tabs")
                .w_full()
                .segmented()
                .selected_index(selected)
                .on_click(cx.listener(|this, ix: &usize, _, cx| {
                    this.set_version_tab(
                        if *ix == 0 { VersionTab::Plugins } else { VersionTab::Themes },
                        cx,
                    );
                }))
                .build_tabs(
                    labels.len(),
                    labels.iter().map(|label| (Some(label.clone()), false)).collect(),
                    {
                        let labels = labels.clone();
                        move |ix, _, _| Tab::new(labels[ix].clone())
                    },
                ),
        )
        .child(match detail.tab {
            VersionTab::Plugins => plugins_tab(screen, is_src, cx).into_any_element(),
            VersionTab::Themes => themes_tab(screen, cx).into_any_element(),
        })
        .into_any_element()
}

fn note(text: impl Into<SharedString>, cx: &mut Context<EntryScreen>) -> Div {
    let theme = cx.theme();
    h_flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(theme.accent.opacity(0.08))
        .child(
            Icon::new(IconName::Info)
                .size_4()
                .text_color(theme.muted_foreground),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(text.into()),
        )
}

fn plugins_tab(
    screen: &mut EntryScreen,
    is_src: bool,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    if is_src {
        return v_flex()
            .flex_1()
            .child(note(
                "A source build loads the plugins its workspace links. Plugins are installed \
                 into release versions only.",
                cx,
            ))
            .into_any_element();
    }
    v_flex()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(note(
            "Plugins belong to this version: a plugin only loads into the engine build it \
             was compiled against.",
            cx,
        ))
        .child(render_registry_panel(screen, RegistryKind::Plugins, cx))
        .into_any_element()
}

fn themes_tab(screen: &mut EntryScreen, cx: &mut Context<EntryScreen>) -> impl IntoElement {
    v_flex()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(note(
            "Themes are shared by every engine version. An installed theme appears in the \
             engine's theme picker right away.",
            cx,
        ))
        .child(render_registry_panel(screen, RegistryKind::Themes, cx))
}
