use std::path::PathBuf;

use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, v_flex, ActiveTheme as _, IconName, StyledExt as _,
};

use crate::service::launch_flags::{LaunchFlags, KNOWN_FLAGS};

/// A managed view shown inside a per-version `Popover` on the Versions screen
/// that edits the engine launch flags stored in `launch-flags.toml` beside the
/// installed binary. Stays open so several flags can be toggled before
/// dismissing with Escape or an outside click.
pub struct LaunchFlagsMenuView {
    focus_handle: FocusHandle,
    install_dir: PathBuf,
    flags: LaunchFlags,
}

impl LaunchFlagsMenuView {
    pub fn new(install_dir: PathBuf, cx: &mut Context<Self>) -> Self {
        let flags = LaunchFlags::load(&install_dir);
        Self {
            focus_handle: cx.focus_handle(),
            install_dir,
            flags,
        }
    }

    fn set_flag(&mut self, env: &'static str, on: bool, cx: &mut Context<Self>) {
        self.flags.set(env, on);
        self.persist(cx);
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.flags.reset_to_defaults();
        self.persist(cx);
    }

    fn persist(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.flags.save(&self.install_dir) {
            tracing::error!(
                "Failed to save launch flags for {}: {}",
                self.install_dir.display(),
                error
            );
        }
        cx.notify();
    }
}

impl EventEmitter<DismissEvent> for LaunchFlagsMenuView {}

impl Focusable for LaunchFlagsMenuView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for LaunchFlagsMenuView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .id("launch-flags-menu")
            .w(px(320.))
            .p_2()
            .gap_1()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .shadow_lg()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .child("Launch flags"),
                    )
                    .child(
                        Button::new("reset-launch-flags")
                            .label("Reset")
                            .icon(IconName::Refresh)
                            .compact()
                            .ghost()
                            .tooltip("Reset all flags to the engine defaults")
                            .on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
                    ),
            )
            .children(KNOWN_FLAGS.iter().enumerate().map(|(index, flag)| {
                let checked = self.flags.checked(flag);
                h_flex()
                    .id(format!("flag-row-{}", index))
                    .w_full()
                    .items_start()
                    .gap_2()
                    .px_1()
                    .py_0p5()
                    .rounded_md()
                    .hover(|this| this.bg(theme.accent.opacity(0.06)))
                    .child(
                        Checkbox::new(format!("flag-check-{}", index))
                            .checked(checked)
                            .on_click({
                                let view = cx.entity().downgrade();
                                move |checked, _, cx| {
                                    if let Some(view) = view.upgrade() {
                                        view.update(cx, |this, cx| {
                                            this.set_flag(flag.env, *checked, cx);
                                        });
                                    }
                                }
                            }),
                    )
                    .child(
                        v_flex().gap_0p5().child(
                            div()
                                .text_sm()
                                .text_color(theme.foreground)
                                .child(flag.label),
                        ).child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(flag.description),
                        ),
                    )
            }))
    }
}
