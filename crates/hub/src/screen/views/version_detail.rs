//! One engine version's page: its plugins (per install) and the themes (shared
//! by every install), with the registries they come from.

use gpui::prelude::*;
use gpui::*;
use ui::{
    ActiveTheme as _, Disableable as _, Icon, IconName, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    scroll::ScrollbarAxis,
    tab::{Tab, TabBar},
    v_flex,
};

use crate::EntryScreen;
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

    v_flex()
        .size_full()
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
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_lg()
                .child(crate::screen::views::onboarding::render_plugin_content(
                    screen, cx,
                )),
        )
        .into_any_element()
}

fn themes_tab(screen: &mut EntryScreen, cx: &mut Context<EntryScreen>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let refreshing = screen.state.theme_refresh_in_progress;
    let installed: std::collections::HashSet<String> = screen
        .state
        .installed_themes
        .iter()
        .map(|record| record.file.clone())
        .collect();
    let registry = screen.state.registry_themes.clone();
    // Themes the Hub installed whose registry no longer lists them.
    let orphaned: Vec<_> = screen
        .state
        .installed_themes
        .iter()
        .filter(|record| !registry.iter().any(|theme| theme.file == record.file))
        .cloned()
        .collect();
    let error = screen.state.theme_error.clone();

    v_flex()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(note(
            "Themes are shared by every engine version. An installed theme appears in the \
             engine's theme picker right away.",
            cx,
        ))
        .when_some(error, |this, error| {
            this.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(theme.danger.opacity(0.12))
                    .text_sm()
                    .text_color(theme.danger)
                    .child(error),
            )
        })
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child("Theme registry"),
                )
                .child(
                    Button::new("refresh-theme-registry")
                        .icon(IconName::Refresh)
                        .compact()
                        .ghost()
                        .disabled(refreshing)
                        .tooltip(if refreshing { "Refreshing..." } else { "Refresh theme registry" })
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_theme_registry(cx))),
                ),
        )
        .child(
            v_flex()
                .id("registry-theme-scroll")
                .flex_1()
                .min_h_0()
                .gap_2()
                .scrollable(ScrollbarAxis::Vertical)
                .when(refreshing && registry.is_empty(), |this| {
                    this.child(
                        div()
                            .py_8()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .text_center()
                            .child("Fetching theme registry..."),
                    )
                })
                .when(!refreshing && registry.is_empty() && orphaned.is_empty(), |this| {
                    this.child(
                        div()
                            .py_8()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .text_center()
                            .child("No themes found. Refresh to try the registry again."),
                    )
                })
                .children(registry.iter().map(|entry| {
                    let is_installed = installed.contains(&entry.file);
                    theme_row(
                        &entry.file,
                        &entry.name,
                        &entry.author,
                        entry.variants.len(),
                        is_installed,
                        cx,
                    )
                }))
                .when(!orphaned.is_empty(), |this| {
                    this.child(
                        div()
                            .mt_2()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Installed, no longer in a registry"),
                    )
                    .children(orphaned.iter().map(|record| {
                        theme_row(&record.file, &record.name, "", 0, true, cx)
                    }))
                }),
        )
}

fn theme_row(
    file: &str,
    name: &str,
    author: &str,
    variants: usize,
    installed: bool,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let (install_file, remove_file) = (file.to_string(), file.to_string());
    let detail = match (author.is_empty(), variants) {
        (true, 0) => String::new(),
        (true, n) => format!("{n} variants"),
        (false, 0) => format!("by {author}"),
        (false, n) => format!("by {author} · {n} variants"),
    };
    h_flex()
        .id(SharedString::from(format!("theme-row-{file}")))
        .w_full()
        .p_3()
        .gap_3()
        .items_center()
        .rounded_lg()
        .bg(theme.secondary.opacity(0.12))
        .border_1()
        .border_color(if installed {
            theme.success_foreground.opacity(0.3)
        } else {
            theme.border
        })
        .child(
            Icon::new(IconName::Palette)
                .size_5()
                .text_color(theme.muted_foreground),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .truncate()
                        .child(name.to_string()),
                )
                .when(!detail.is_empty(), |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(detail),
                    )
                }),
        )
        .child(if installed {
            h_flex()
                .gap_1()
                .items_center()
                .child(
                    div()
                        .px_2()
                        .py(px(2.))
                        .rounded_full()
                        .bg(theme.success_foreground.opacity(0.15))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.success_foreground)
                        .child("Installed"),
                )
                .child(
                    Button::new(SharedString::from(format!("remove-theme-{file}")))
                        .icon(IconName::Trash)
                        .compact()
                        .ghost()
                        .tooltip("Remove theme")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_installed_theme(&remove_file, cx)
                        })),
                )
                .into_any_element()
        } else {
            Button::new(SharedString::from(format!("install-theme-{file}")))
                .primary()
                .label("Install")
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.install_registry_theme(&install_file, cx)
                }))
                .into_any_element()
        })
}
