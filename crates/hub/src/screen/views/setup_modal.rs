//! The setup that follows first-run onboarding: install an engine version,
//! then pick plugins for that install. Shown only in the session that ran
//! onboarding; "Not now" closes it and everything is reachable later from the
//! Versions screen.

use gpui::prelude::*;
use gpui::*;
use ui::{
    ActiveTheme as _, Disableable as _, Icon, IconName, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use wgpui_motion::prelude::{MotionExt as _, MotionToken, Transition};

use crate::EntryScreen;
use crate::core::types::SetupPage;

pub fn render_setup_modal(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let Some(flow) = screen.state.ui.setup_flow.clone() else {
        return div().into_any_element();
    };
    let theme = cx.theme().clone();
    let newest = screen.state.versions.installed.first().cloned();
    let installing = matches!(
        screen.state.versions.install_state,
        crate::service::installer_service::VersionInstallState::Downloading { .. }
    );

    let (step, title, subtitle) = match flow.page {
        SetupPage::Engine => (
            1,
            "Install an engine".to_string(),
            "An engine opens and creates Pulsar projects. Stable is the recommended release."
                .to_string(),
        ),
        SetupPage::Plugins => (
            2,
            match &newest {
                Some(version) => format!("Add plugins to v{}", version.metadata.version),
                None => "Add plugins".to_string(),
            },
            "Plugins belong to this engine version. They are optional and can be added or \
             removed later from the Versions screen."
                .to_string(),
        ),
    };

    let body = match flow.page {
        SetupPage::Engine => div()
            .flex_1()
            .min_h_0()
            .child(crate::screen::views::versions::render_install_panel(
                screen, cx, true,
            ))
            .into_any_element(),
        SetupPage::Plugins => div()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .child(crate::screen::views::onboarding::render_plugin_content(
                screen, cx,
            ))
            .into_any_element(),
    };

    let footer = match flow.page {
        SetupPage::Engine => h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(if installing {
                        "Installing: plugins come next when it finishes.".to_string()
                    } else if let Some(version) = &newest {
                        format!("v{} is already installed.", version.metadata.version)
                    } else {
                        "Pick a release above to install it.".to_string()
                    }),
            )
            .child(
                Button::new("setup-not-now")
                    .label("Not now")
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| this.close_setup_flow(cx))),
            )
            .child(
                Button::new("setup-to-plugins")
                    .label("Continue")
                    .primary()
                    .disabled(newest.is_none() || installing)
                    .on_click(cx.listener(|this, _, _, cx| this.setup_continue_to_plugins(cx))),
            )
            .into_any_element(),
        SetupPage::Plugins => h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!(
                        "{} installed",
                        screen.state.installed_plugins.len()
                    )),
            )
            .child(
                Button::new("setup-back")
                    .label("Back")
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| this.setup_back_to_engine(cx))),
            )
            .child(
                Button::new("setup-done")
                    .label("Done")
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| this.close_setup_flow(cx))),
            )
            .into_any_element(),
    };

    v_flex()
        .id("setup-modal-backdrop")
        .absolute()
        .inset_0()
        .items_center()
        .justify_center()
        .p_6()
        .bg(theme.background.opacity(0.45))
        // Click-through to the screen behind must not reach it.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            v_flex()
                .id("setup-modal")
                .w(px(780.))
                .max_w_full()
                .h(px(640.))
                .max_h_full()
                .overflow_hidden()
                .rounded_xl()
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover.opacity(0.97))
                .shadow_xl()
                .motion("setup-modal-motion")
                .transition(Transition::token(MotionToken::SpatialFast))
                .enter(|style| style.opacity(0.).mt(px(10.)))
                .base(|style| style.opacity(1.).mt(px(0.)))
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
                            Icon::new(match flow.page {
                                SetupPage::Engine => IconName::Download,
                                SetupPage::Plugins => IconName::Package,
                            })
                            .size_6()
                            .text_color(theme.foreground),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(subtitle),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("Step {step} of 2")),
                        )
                        .child(
                            Button::new("setup-close")
                                .icon(IconName::Close)
                                .compact()
                                .ghost()
                                .tooltip("Close")
                                .on_click(cx.listener(|this, _, _, cx| this.close_setup_flow(cx))),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_h_0()
                        .p_4()
                        .child(body),
                )
                .child(
                    div()
                        .w_full()
                        .px_5()
                        .py_3()
                        .border_t_1()
                        .border_color(theme.border)
                        .child(footer),
                ),
        )
        .into_any_element()
}
