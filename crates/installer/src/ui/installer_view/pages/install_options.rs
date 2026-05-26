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
use gpui::prelude::FluentBuilder;
use super::super::{InstallerView, Page, SIDECAR_PACKAGES};

impl InstallerView {
    pub(crate) fn render_install_options(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let path_str         = self.install_config.install_path.display().to_string();
        let desktop_shortcut = self.install_config.create_desktop_shortcut;
        let start_menu       = self.install_config.create_start_menu_shortcut;
        let add_to_path      = self.install_config.add_to_path;

        #[cfg(target_os = "macos")]
        let use_app_bundle = self.macos_use_app_bundle;

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
                    // ── macOS: download-format picker ─────────────────────────
                    .when(cfg!(target_os = "macos"), |el: gpui::Stateful<gpui::Div>| {
                        #[cfg(target_os = "macos")]
                        let use_bundle = use_app_bundle;
                        #[cfg(not(target_os = "macos"))]
                        let use_bundle = false;

                        el.child(
                            v_flex()
                                .gap_3()
                                .child(section_label("Download Format", cx))
                                .child(
                                    v_flex()
                                        .gap_2()
                                        // Binary (recommended)
                                        .child(format_option_row(
                                            "macos-format-binary",
                                            "Binary",
                                            Some(("Recommended", true)),
                                            "Bare executable — smaller download, no Gatekeeper bundle issues.",
                                            !use_bundle,
                                            cx.listener(|this, _, _, cx| {
                                                this.macos_use_app_bundle = false;
                                                #[cfg(target_os = "macos")]
                                                if this.install_config.install_path.extension()
                                                    == Some(std::ffi::OsStr::new("app"))
                                                {
                                                    this.install_config.install_path =
                                                        this.install_config.install_path.with_extension("");
                                                }
                                                cx.notify();
                                            }),
                                            cx,
                                        ))
                                        // .app Bundle (not recommended)
                                        .child(format_option_row(
                                            "macos-format-appbundle",
                                            ".app Bundle",
                                            Some(("Not Recommended", false)),
                                            "Pre-built .app bundle — larger download, may require Gatekeeper approval.",
                                            use_bundle,
                                            cx.listener(|this, _, _, cx| {
                                                this.macos_use_app_bundle = true;
                                                #[cfg(target_os = "macos")]
                                                if this.install_config.install_path.extension()
                                                    != Some(std::ffi::OsStr::new("app"))
                                                {
                                                    let p = this.install_config.install_path.clone();
                                                    this.install_config.install_path =
                                                        p.with_file_name(format!(
                                                            "{}.app",
                                                            p.file_name()
                                                                .and_then(|n| n.to_str())
                                                                .unwrap_or("Pulsar")
                                                        ));
                                                }
                                                cx.notify();
                                            }),
                                            cx,
                                        )),
                                ),
                        )
                    })
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
                                this.navigate_to(Page::VersionSelection, window, cx);
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

/// A selectable radio-style row for the macOS download-format picker.
/// `on_click` should be the result of `cx.listener(...)`.
fn format_option_row(
    id: &str,
    label: &str,
    badge: Option<(&str, bool)>, // (text, is_positive)
    desc: &str,
    selected: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    cx: &mut gpui::Context<InstallerView>,
) -> impl IntoElement {
    h_flex()
        .id(id.to_string())
        .gap_3()
        .p_3()
        .rounded(px(8.0))
        .border_1()
        .border_color(if selected {
            cx.theme().secondary.opacity(0.5)
        } else {
            cx.theme().border
        })
        .bg(if selected {
            cx.theme().secondary.opacity(0.05)
        } else {
            cx.theme().sidebar.opacity(0.3)
        })
        .cursor_pointer()
        .on_click(on_click)
        // Radio indicator
        .child(
            gpui::div()
                .w(px(16.0))
                .h(px(16.0))
                .rounded_full()
                .border_2()
                .border_color(cx.theme().secondary)
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |e| {
                    e.child(
                        gpui::div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(cx.theme().secondary),
                    )
                }),
        )
        // Label + badge + description
        .child(
            v_flex()
                .flex_1()
                .gap(px(1.0))
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            gpui::div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(cx.theme().foreground)
                                .child(label.to_string()),
                        )
                        .when_some(badge, |el, (badge_text, positive)| {
                            el.child(
                                gpui::div()
                                    .px_2()
                                    .py(px(1.0))
                                    .rounded(px(4.0))
                                    .bg(if positive {
                                        cx.theme().success.opacity(0.15)
                                    } else {
                                        cx.theme().warning.opacity(0.15)
                                    })
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if positive {
                                        cx.theme().success
                                    } else {
                                        cx.theme().warning
                                    })
                                    .child(badge_text.to_string()),
                            )
                        }),
                )
                .child(
                    gpui::div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(desc.to_string()),
                ),
        )
}
