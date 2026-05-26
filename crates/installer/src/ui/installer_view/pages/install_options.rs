//! Install-options page — path picker, macOS format, sidecars, and misc flags.

use gpui::{
    Context, FontWeight, IntoElement, InteractiveElement as _,
    ParentElement, StatefulInteractiveElement as _, Styled, px,
};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, v_flex,
};
use super::super::{InstallerView, Page, SIDECAR_PACKAGES};

impl InstallerView {
    pub(crate) fn render_install_options(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let path_str         = self.install_config.install_path.display().to_string();
        let desktop_shortcut = self.install_config.create_desktop_shortcut;
        let start_menu       = self.install_config.create_start_menu_shortcut;
        let add_to_path      = self.install_config.add_to_path;

        v_flex()
            .size_full()
            .child(Self::render_panel_header("Install Options", None, cx))
            // Scrollable content
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .id("install-opts-scroll")
                    .overflow_y_scroll()
                    .p_6()
                    .gap_6()
                    // ── Install path ──────────────────────────────────────────
                    .child(
                        v_flex()
                            .gap_3()
                            .child(section_label("Installation Path", cx))
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(
                                        gpui::div()
                                            .flex_1()
                                            .px_3()
                                            .py_2()
                                            .rounded(px(6.0))
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().sidebar)
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child(path_str),
                                    )
                                    .child(
                                        Button::new("browse-btn")
                                            .outline()
                                            .label("Browse…")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_native_picker(cx);
                                            })),
                                    ),
                            ),
                    )
                    // ── Optional sidecar packages ─────────────────────────────
                    .child({
                        let selected = self.selected_sidecars.clone();
                        v_flex()
                            .gap_3()
                            .child(section_label("Optional Components", cx))
                            .children(
                                SIDECAR_PACKAGES
                                    .iter()
                                    .enumerate()
                                    .map(|(i, (id, name, desc))| {
                                        let id_str   = id.to_string();
                                        let is_checked = selected.contains(&id_str);
                                        let id_cb    = id_str.clone();
                                        h_flex()
                                            .gap_3()
                                            .items_center()
                                            .child(
                                                Checkbox::new(format!("sidecar-cb-{i}"))
                                                    .checked(is_checked)
                                                    .on_click(cx.listener(
                                                        move |this, checked: &bool, _, cx| {
                                                            if *checked {
                                                                if !this.selected_sidecars.contains(&id_cb) {
                                                                    this.selected_sidecars.push(id_cb.clone());
                                                                }
                                                            } else {
                                                                this.selected_sidecars
                                                                    .retain(|s| s != &id_cb);
                                                            }
                                                            cx.notify();
                                                        },
                                                    )),
                                            )
                                            .child(
                                                v_flex()
                                                    .child(
                                                        gpui::div()
                                                            .text_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .child(*name),
                                                    )
                                                    .child(
                                                        gpui::div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(*desc),
                                                    ),
                                            )
                                    }),
                            )
                    })
                    // ── Install options ───────────────────────────────────────
                    .child(
                        v_flex()
                            .gap_3()
                            .child(section_label("Options", cx))
                            .child(
                                v_flex()
                                    .gap_3()
                                    .child(option_row(
                                        "desktop-shortcut-cb",
                                        desktop_shortcut,
                                        "Create Desktop Shortcut",
                                        "Add a shortcut to your desktop.",
                                        cx.listener(|this, checked: &bool, _, cx| {
                                            this.install_config.create_desktop_shortcut = *checked;
                                            cx.notify();
                                        }),
                                        cx,
                                    ))
                                    .child(option_row(
                                        "start-menu-cb",
                                        start_menu,
                                        {
                                            #[cfg(target_os = "macos")]
                                            { "Add to Dock" }
                                            #[cfg(windows)]
                                            { "Create Start Menu Shortcut" }
                                            #[cfg(target_os = "linux")]
                                            { "Create Application Launcher Entry" }
                                            #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
                                            { "Create Shortcut" }
                                        },
                                        "Integrate with the system launcher.",
                                        cx.listener(|this, checked: &bool, _, cx| {
                                            this.install_config.create_start_menu_shortcut =
                                                *checked;
                                            cx.notify();
                                        }),
                                        cx,
                                    ))
                                    .child(option_row(
                                        "path-cb",
                                        add_to_path,
                                        "Add to PATH",
                                        "Run `pulsar` from any terminal.",
                                        cx.listener(|this, checked: &bool, _, cx| {
                                            this.install_config.add_to_path = *checked;
                                            cx.notify();
                                        }),
                                        cx,
                                    )),
                            ),
                    ),
            )
            // ── Action bar ────────────────────────────────────────────────────
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .items_center()
                    .child(
                        Button::new("opts-back-btn")
                            .outline()
                            .label("← Back")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::ReleaseNotes, window, cx);
                            })),
                    )
                    .child(
                        Button::new("opts-install-btn")
                            .primary()
                            .label("Install →")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate_to(Page::Installing, window, cx);
                                this.start_installation(window, cx);
                            })),
                    ),
            )
    }
}

// ─── Local helpers ────────────────────────────────────────────────────────────

/// Bold section-heading label.
fn section_label(text: &str, cx: &mut gpui::Context<InstallerView>) -> impl IntoElement {
    gpui::div()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
        .child(text.to_string())
}

/// A checkbox row with a title and description.
/// `on_click` should be the result of `cx.listener(...)`.
fn option_row(
    id: &str,
    checked: bool,
    title: &str,
    desc: &str,
    on_click: impl Fn(&bool, &mut gpui::Window, &mut gpui::App) + 'static,
    cx: &mut gpui::Context<InstallerView>,
) -> impl IntoElement {
    h_flex()
        .gap_3()
        .items_center()
        .child(
            Checkbox::new(id.to_string())
                .checked(checked)
                .on_click(on_click),
        )
        .child(
            v_flex()
                .child(
                    gpui::div()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child(title.to_string()),
                )
                .child(
                    gpui::div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(desc.to_string()),
                ),
        )
}

