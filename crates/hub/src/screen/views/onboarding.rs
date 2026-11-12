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
pub(crate) fn render_plugin_phase(
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
