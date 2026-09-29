use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants},
    scroll::ScrollbarAxis,
    h_flex, v_flex, ActiveTheme, Disableable, Icon, IconName, StyledExt,
};

use crate::screen::EntryScreen;

pub fn render(screen: &mut EntryScreen, cx: &mut Context<EntryScreen>) -> AnyElement {
    let theme = cx.theme().clone();
    let (rust_installed, build_tools_installed) = screen
        .state
        .dependency_status
        .as_ref()
        .map(|status| (status.rust_installed, status.build_tools_installed))
        .unwrap_or((false, false));
    let can_install = !rust_installed || !build_tools_installed;
    let installation_running = screen
        .state
        .install_progress
        .as_ref()
        .is_some_and(|progress| {
            matches!(
                progress.status,
                crate::core::types::InstallStatus::Downloading
                    | crate::core::types::InstallStatus::Installing
            )
        });
    v_flex()
        .size_full()
        .gap_4()
        .child(
            h_flex()
                .items_center()
                .gap_3()
                .child(Icon::new(IconName::Package).size_6().text_color(theme.accent))
                .child(v_flex().gap_1()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).text_color(theme.foreground).child("Build tools"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("These tools are only needed to build the engine from source. You can use installed engine versions without them."))),
        )
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .gap_3()
                .id("onboarding-build-tools-content")
                .scrollable(ScrollbarAxis::Vertical)
                .p_5()
                .bg(theme.secondary.opacity(0.12))
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .child(super::super::render_dep_item("Rust toolchain", rust_installed, None, cx))
                .child(super::super::render_dep_item(
                    "C/C++ build tools",
                    build_tools_installed,
                    screen.state.dependency_status.as_ref().and_then(|status| status.compiler_info.clone()),
                    cx,
                ))
                .children(screen.state.install_progress.clone().map(|progress| super::super::render_install_progress(progress, cx)))
                .child(
                    Button::new("install-deps-onboarding")
                        .label(if can_install { "Install build tools" } else { "Build tools are ready" })
                        .primary()
                        .disabled(!can_install || installation_running)
                        .on_click(cx.listener(|this, _, _, cx| {
                            super::super::run_setup_script(this, cx);
                            cx.notify();
                        })),
                ),
        )
        .into_any_element()
}
