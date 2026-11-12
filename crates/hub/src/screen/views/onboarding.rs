use crate::core::types::*;
use crate::screen::EntryScreen;
use crate::service::auth_service::AuthService;
use crate::service::dependency_service::DependencyService;
use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants},
    h_flex,
    scroll::ScrollbarAxis,
    v_flex, ActiveTheme, Disableable, Icon, IconName, Sizable, StyledExt,
};
mod steps;

/// First-run setup, one page: a word of welcome, an optional account, a look.
/// Engines and plugins are not part of it: when it ends, the setup modal
/// (`setup_modal`) offers an engine and then its plugins.
pub fn render_onboarding(
    screen: &mut EntryScreen,
    _window: &mut Window,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let account = steps::account::render(screen, cx);
    let look = steps::theme::render(screen, cx);
    div()
        .absolute()
        .size_full()
        .inset_0()
        .flex()
        .flex_col()
        .bg(theme.background)
        .child(
            h_flex()
                .w_full()
                .px_8()
                .py_3()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    Icon::new(IconName::Package)
                        .size(px(18.))
                        .text_color(theme.foreground),
                )
                .child(
                    div()
                        .ml_2()
                        .flex_1()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child("Pulsar Hub Setup"),
                )
                .child(
                    Button::new("close-onboarding")
                        .compact()
                        .ghost()
                        .icon(IconName::Close)
                        .tooltip("Skip setup")
                        .on_click(cx.listener(|this, _, _, cx| this.finish_onboarding(cx))),
                ),
        )
        .child(
            h_flex()
                .w_full()
                .flex_1()
                .min_h_0()
                .justify_center()
                .overflow_hidden()
                .px_8()
                .py_6()
                .child(
                    v_flex()
                        .w_full()
                        .max_w(px(1000.))
                        .h_full()
                        .gap_5()
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_2xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.foreground)
                                        .child("Welcome to Pulsar Hub"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child(
                                            "Both choices below are optional and can be changed \
                                             later. Next you can install an engine version and \
                                             its plugins.",
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .flex_1()
                                .min_h_0()
                                .w_full()
                                .gap_6()
                                .items_start()
                                .child(div().w(px(380.)).flex_shrink_0().h_full().child(account))
                                .child(div().flex_1().min_w_0().h_full().child(look)),
                        ),
                ),
        )
        .child(
            h_flex()
                .w_full()
                .justify_center()
                .px_8()
                .py_4()
                .border_t_1()
                .border_color(theme.border)
                .child(
                    h_flex()
                        .w_full()
                        .max_w(px(1000.))
                        .justify_end()
                        .child(
                            Button::new("onboarding-finish")
                                .label("Get started")
                                .primary()
                                .on_click(cx.listener(|this, _, _, cx| this.finish_onboarding(cx))),
                        ),
                ),
        )
}
fn render_theme_content(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let current_name = cx.theme().theme_name().clone();
    let themes: Vec<std::rc::Rc<ui::ThemeConfig>> = ui::ThemeRegistry::global(cx)
        .sorted_themes()
        .into_iter()
        .cloned()
        .collect();
    v_flex().flex_1().min_h_0().child(
        v_flex()
            .id("theme-scroll")
            .flex_1()
            .min_h_0()
            .scrollable(ScrollbarAxis::Vertical)
            .gap_3()
            .p_4()
            .child(
                h_flex().flex_wrap().gap_3().children(
                    themes
                        .into_iter()
                        .map(|config| render_theme_card(config, &current_name, screen, cx)),
                ),
            ),
    )
}
fn render_theme_card(
    config: std::rc::Rc<ui::ThemeConfig>,
    current_name: &str,
    _screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let name = config.name.clone();
    let is_active = name == current_name;
    let name_for_click = name.clone();
    let is_dark = config.mode.is_dark();
    let swatch_bg = config
        .colors
        .background
        .as_ref()
        .and_then(|h| gpui::Rgba::try_from(h.as_ref()).ok().map(gpui::Hsla::from))
        .unwrap_or_else(|| {
            if is_dark {
                gpui::hsla(0., 0., 0.1, 1.)
            } else {
                gpui::hsla(0., 0., 0.97, 1.)
            }
        });
    let swatch_fg = config
        .colors
        .foreground
        .as_ref()
        .and_then(|h| gpui::Rgba::try_from(h.as_ref()).ok().map(gpui::Hsla::from))
        .unwrap_or_else(|| {
            if is_dark {
                gpui::hsla(0., 0., 0.95, 1.)
            } else {
                gpui::hsla(0., 0., 0.05, 1.)
            }
        });
    let accent_col = config
        .colors
        .accent
        .as_ref()
        .and_then(|h| gpui::Rgba::try_from(h.as_ref()).ok().map(gpui::Hsla::from))
        .unwrap_or_else(|| {
            if is_dark {
                gpui::hsla(0.6, 0.7, 0.5, 1.)
            } else {
                gpui::hsla(0.6, 0.7, 0.4, 1.)
            }
        });
    v_flex()
        .id(SharedString::from(format!("theme-card-{}", name)))
        .w(px(160.))
        .p(px(12.))
        .rounded_lg()
        .cursor_pointer()
        .bg(if is_active {
            theme.secondary.opacity(0.4)
        } else {
            theme.secondary.opacity(0.15)
        })
        .hover(|this| this.bg(theme.secondary.opacity(0.3)))
        .border_1()
        .border_color(if is_active {
            theme.accent
        } else {
            theme.border
        })
        .gap_2()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_, _, _, cx| {
                if let Some(cfg) = ui::ThemeRegistry::global(cx)
                    .themes()
                    .get(&name_for_click)
                    .cloned()
                {
                    ui::Theme::global_mut(cx).apply_config(&cfg);
                    cx.refresh_windows();
                }
            }),
        )
        .child(
            div()
                .w_full()
                .h(px(40.))
                .rounded_md()
                .bg(swatch_bg)
                .border_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .justify_center()
                .gap(px(4.))
                .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(accent_col))
                .child(div().w(px(24.)).h(px(4.)).rounded_full().bg(swatch_fg)),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child(name.to_string()),
        )
        .child(
            h_flex()
                .gap_1()
                .items_center()
                .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(if is_dark {
                    theme.primary
                } else {
                    theme.warning
                }))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(if is_dark { "Dark" } else { "Light" }),
                ),
        )
}
pub(crate) fn render_plugin_content(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let search_input = screen.inputs().plugin_search.clone();
    let query = screen.state.input.plugin_search_query.to_lowercase();
    let is_refreshing = screen.state.registry_refresh_in_progress;
    let phase = screen.state.plugin_install_phase.clone();
    let has_phase = phase.is_some();
    let available: Vec<RegistryPlugin> = screen
        .state
        .registry_plugins
        .iter()
        .filter(|p| {
            query.is_empty()
                || p.name.to_lowercase().contains(&query)
                || p.description.to_lowercase().contains(&query)
                || p.author.to_lowercase().contains(&query)
                || p.tags.iter().any(|t| t.to_lowercase().contains(&query))
        })
        .cloned()
        .collect();
    let installed_urls: std::collections::HashSet<String> = screen
        .state
        .installed_plugins
        .iter()
        .map(|p| p.repo_url.clone())
        .collect();
    let custom_installed: Vec<InstalledPlugin> = screen
        .state
        .installed_plugins
        .iter()
        .filter(|p| {
            !screen
                .state
                .registry_plugins
                .iter()
                .any(|rp| rp.repo_url == p.repo_url)
        })
        .cloned()
        .collect();
    let registries_empty = screen.state.registry_plugins.is_empty();
    let available_empty = available.is_empty();
    let phase_element = phase.map(|p| render_plugin_phase(p, cx));
    let plugin_cards: Vec<_> = available
        .into_iter()
        .map(|plugin| {
            let is_installed = installed_urls.contains(&plugin.repo_url);
            render_registry_plugin_card(plugin, is_installed, cx)
        })
        .collect();
    let custom_plugin_rows: Vec<_> = custom_installed
        .into_iter()
        .enumerate()
        .map(|(idx, plugin)| render_custom_plugin_row(idx, plugin, cx))
        .collect();
    v_flex()
        .flex_1()
        .min_h_0()
        .child(
            h_flex()
                .px_4()
                .pt_3()
                .pb_2()
                .gap_2()
                .items_center()
                .child(div().flex_1().child(ui::input::Input::new(&search_input)))
                .child(
                    Button::new("refresh-registries-btn")
                        .icon(IconName::Refresh)
                        .ghost()
                        .compact()
                        .disabled(is_refreshing)
                        .tooltip(if is_refreshing {
                            "Refreshing..."
                        } else {
                            "Refresh plugin registries"
                        })
                        .on_click(cx.listener(|screen, _, _, cx| {
                            screen.refresh_plugin_registry(cx);
                        })),
                ),
        )
        .when_some(phase_element, |this, el| this.child(el))
        .child(
            v_flex()
                .id("registry-plugin-scroll")
                .flex_1()
                .min_h_0()
                .scrollable(ScrollbarAxis::Vertical)
                .px_4()
                .pb_2()
                .gap_2()
                .when(is_refreshing && registries_empty, |this| {
                    this.child(
                        div()
                            .py_10()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .text_center()
                            .child("Fetching plugin registry..."),
                    )
                })
                .when(!is_refreshing && registries_empty, |this| {
                    this.child(
                        v_flex()
                            .py_10()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::Package)
                                    .size_8()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .text_center()
                                    .child(
                                        "No plugins found. Click \u{21BA} to refresh registries.",
                                    ),
                            ),
                    )
                })
                .when(!registries_empty && available_empty, |this| {
                    this.child(
                        div()
                            .py_8()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .text_center()
                            .child("No plugins match your search"),
                    )
                })
                .children(plugin_cards)
                .when(!custom_plugin_rows.is_empty(), |this| {
                    this.child(
                        h_flex()
                            .mt_2()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(1.))
                                    .bg(theme.muted_foreground.opacity(0.25)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("Custom installed"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(1.))
                                    .bg(theme.muted_foreground.opacity(0.25)),
                            ),
                    )
                    .children(custom_plugin_rows)
                }),
        )
}
fn render_registry_plugin_card(
    plugin: RegistryPlugin,
    is_installed: bool,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let repo_url = plugin.repo_url.clone();
    let repo_url_for_remove = repo_url.clone();
    let name = plugin.name.clone();
    let desc = plugin.description.clone();
    let author = plugin.author.clone();
    let tags = plugin.tags.clone();
    v_flex()
        .id(SharedString::from(format!("rp-{}", repo_url)))
        .w_full()
        .p_3()
        .gap_2()
        .rounded_lg()
        .bg(theme.secondary.opacity(0.12))
        .border_1()
        .border_color(if is_installed {
            theme.success_foreground.opacity(0.3)
        } else {
            theme.border
        })
        .child(
            h_flex()
                .gap_2()
                .items_start()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .truncate()
                                        .child(name),
                                )
                                .when(!author.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(format!("by {author}")),
                                    )
                                }),
                        )
                        .when(!desc.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(desc),
                            )
                        }),
                )
                .child(if is_installed {
                    h_flex()
                        .gap_1()
                        .items_center()
                        .flex_shrink_0()
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
                            Button::new(SharedString::from(format!(
                                "remove-reg-{}",
                                repo_url_for_remove
                            )))
                            .ghost()
                            .compact()
                            .icon(IconName::Trash)
                            .on_click(cx.listener(
                                move |screen, _, _, cx| {
                                    if let Some(idx) = screen
                                        .state
                                        .installed_plugins
                                        .iter()
                                        .position(|p| p.repo_url == repo_url_for_remove)
                                    {
                                        screen.remove_plugin(idx, cx);
                                    }
                                },
                            )),
                        )
                        .into_any_element()
                } else {
                    Button::new(SharedString::from(format!("install-reg-{}", repo_url)))
                        .label("Install")
                        .primary()
                        .compact()
                        .on_click({
                            let url = repo_url.clone();
                            cx.listener(move |screen, _, _, cx| {
                                if let Some(plugin) = screen
                                    .state
                                    .registry_plugins
                                    .iter()
                                    .find(|p| p.repo_url == url)
                                    .cloned()
                                {
                                    screen.install_registry_plugin(plugin, cx);
                                }
                            })
                        })
                        .into_any_element()
                }),
        )
        .when(!tags.is_empty(), |this| {
            this.child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .children(tags.into_iter().map(|tag| {
                        div()
                            .px_2()
                            .py(px(1.))
                            .rounded_full()
                            .bg(theme.accent)
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.accent_foreground)
                            .child(tag)
                    })),
            )
        })
}
fn render_plugin_phase(
    phase: PluginInstallPhase,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let (icon, color, headline, sub, progress_val, show_bar) = match &phase {
        PluginInstallPhase::FetchingMetadata => (
            IconName::Download,
            theme.foreground,
            "Fetching release info...",
            String::new(),
            None,
            false,
        ),
        PluginInstallPhase::Downloading { progress } => (
            IconName::Download,
            theme.foreground,
            "Downloading binary...",
            format!("{:.0}%", progress * 100.0),
            Some(*progress),
            true,
        ),
        PluginInstallPhase::Building { logs } => (
            IconName::Settings,
            theme.foreground,
            "Building from source...",
            logs.last().cloned().unwrap_or_default(),
            None,
            false,
        ),
        PluginInstallPhase::Complete(p) => (
            IconName::Check,
            theme.success_foreground,
            "Installed successfully!",
            format!("{} \u{2014} {}", p.name, p.version),
            Some(1.0),
            true,
        ),
        PluginInstallPhase::Error(e) => (
            IconName::WarningTriangle,
            gpui::red(),
            "Installation failed",
            e.clone(),
            None,
            false,
        ),
    };
    v_flex()
        .mx_4()
        .mb_3()
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(color.opacity(0.35))
        .bg(color.opacity(0.08))
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(icon).size_4().text_color(color))
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(headline),
                )
                .when(
                    matches!(
                        phase,
                        PluginInstallPhase::Complete(_) | PluginInstallPhase::Error(_)
                    ),
                    |this| {
                        this.child(
                            Button::new("dismiss-phase")
                                .ghost()
                                .compact()
                                .icon(IconName::Close)
                                .on_click(cx.listener(|screen, _, _, cx| {
                                    screen.state.plugin_install_phase = None;
                                    cx.notify();
                                })),
                        )
                    },
                ),
        )
        .when(!sub.is_empty(), |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(sub),
            )
        })
        .when(show_bar, |this| {
            let pv = progress_val.unwrap_or(0.0);
            this.child(
                div()
                    .w_full()
                    .h(px(4.))
                    .rounded_full()
                    .bg(theme.secondary.opacity(0.3))
                    .child(
                        div()
                            .h_full()
                            .rounded_full()
                            .bg(color)
                            .w(relative(pv.max(0.0).min(1.0))),
                    ),
            )
        })
}
fn render_custom_plugin_row(
    idx: usize,
    plugin: InstalledPlugin,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let method_label = match plugin.install_method {
        PluginInstallMethod::BinaryDownload => "Binary",
        PluginInstallMethod::BuiltFromSource => "Source",
    };
    let method_color = match plugin.install_method {
        PluginInstallMethod::BinaryDownload => theme.success_foreground,
        PluginInstallMethod::BuiltFromSource => theme.foreground,
    };
    let name = plugin.name.clone();
    let version = plugin.version.clone();
    let repo = plugin.repo_url.clone();
    h_flex()
        .id(SharedString::from(format!("plugin-row-{idx}")))
        .w_full()
        .px_3()
        .py_2()
        .gap_3()
        .items_center()
        .rounded_lg()
        .bg(theme.secondary.opacity(0.15))
        .border_1()
        .border_color(theme.border)
        .child(
            Icon::new(IconName::Package)
                .size_4()
                .text_color(theme.foreground),
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
                        .child(name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .truncate()
                        .child(repo),
                ),
        )
        .child(
            div()
                .flex_shrink_0()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.muted_foreground)
                .child(version),
        )
        .child(
            div()
                .flex_shrink_0()
                .px_2()
                .py(px(2.))
                .rounded_full()
                .bg(method_color.opacity(0.15))
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(method_color)
                .child(method_label),
        )
        .child(
            Button::new(SharedString::from(format!("remove-plugin-{idx}")))
                .ghost()
                .compact()
                .icon(IconName::Trash)
                .on_click(cx.listener(move |screen, _, _, cx| {
                    screen.remove_plugin(idx, cx);
                })),
        )
}







