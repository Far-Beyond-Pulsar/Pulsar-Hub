use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Icon, IconName,
};

use crate::service::installer_service;
use crate::EntryScreen;

pub fn render_versions(
    screen: &mut EntryScreen,
    window: &mut Window,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let installed = screen.state.versions.installed.clone();
    let releases = screen.state.versions.available_releases.clone();
    let is_fetching = screen.state.versions.fetching;

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
                            Button::new("btn-install-new")
                                .label("Install Latest")
                                .icon(IconName::Plus)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.install_latest_version(cx);
                                })),
                        ),
                ),
        )
        .child(div().w_full().h(px(1.0)).bg(theme.border))
        .child(
            v_flex()
                .w_full()
                .flex_1()
                .overflow_y_scroll()
                .gap_3()
                .when(installed.is_empty(), |this| {
                    this.child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .justify_center()
                            .py_12()
                            .gap_3()
                            .child(
                                Icon::new(IconName::Package)
                                    .size(px(48.))
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .text_color(theme.foreground)
                                    .child("No Pulsar installations found"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Click \"Install Latest\" to get started"),
                            ),
                    )
                })
                .children(installed.iter().enumerate().map(|(idx, ver)| {
                    let version = ver.metadata.version.clone();
                    let date = ver.metadata.install_date.clone();
                    let size = format_bytes(ver.disk_size_bytes);
                    let path = ver.metadata.install_path.clone();

                    let path_clone = path.clone();

                    h_flex()
                        .id(format!("version-{}", idx))
                        .w_full()
                        .items_center()
                        .justify_between()
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border)
                        .hover(|this| this.bg(theme.accent.opacity(0.05)))
                        .child(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .child(
                                    Icon::new(IconName::Package)
                                        .size(px(20.))
                                        .text_color(theme.accent),
                                )
                                .child(
                                    v_flex()
                                        .gap_0p5()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(theme.foreground)
                                                        .child(format!("v{}", version)),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .px_2()
                                                        .py_0p5()
                                                        .rounded_md()
                                                        .bg(theme.accent.opacity(0.12))
                                                        .text_color(theme.accent)
                                                         .child(size),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(if date.is_empty() {
                                                    "Unknown date".to_string()
                                                } else {
                                                    date
                                                })
                                                .child("·")
                                                .child(path.display().to_string()),
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child({
                                    let p = path_clone.clone();
                                    Button::new(format!("launch-{}", idx))
                                        .label("Launch")
                                        .icon(IconName::Play)
                                        .compact()
                                        .ghost()
                                        .on_click(move |_, _, cx| {
                                            let p = p.clone();
                                            cx.spawn(async move |_| {
                                                if let Err(e) =
                                                    installer_service::launch_engine(&p)
                                                {
                                                    tracing::error!("Launch failed: {}", e);
                                                }
                                            })
                                            .detach();
                                        })
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
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.remove_version(&v, cx);
                                        }))
                                }),
                        )
                        .into_any_element()
                }))
                .when(!releases.is_empty(), |this| {
                    this.child(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .pt_4()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.muted_foreground)
                                    .child("Available Releases"),
                            )
                            .children(releases.iter().filter(|r| !r.prerelease).take(10).enumerate().map(
                                |(idx, release)| {
                                    let tag = release.tag_name.clone();
                                    let name = release.name.clone();
                                    let has_asset =
                                        installer_service::find_platform_asset(release).is_some();
                                    let already_installed = installed
                                        .iter()
                                        .any(|v| v.metadata.version == tag.trim_start_matches('v'));

                                    h_flex()
                                        .id(format!("release-{}", idx))
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .p_3()
                                        .rounded_lg()
                                        .border_1()
                                        .border_color(theme.border)
                                        .hover(|this| this.bg(theme.accent.opacity(0.05)))
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    Icon::new(IconName::Label)
                                                        .size(px(16.))
                                                        .text_color(theme.muted_foreground),
                                                )
                                                .child(
                                                    v_flex()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                                .text_color(theme.foreground)
                                                                  .child(tag.clone()),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme.muted_foreground)
                                                                 .child(name),
                                                        ),
                                                ),
                                        )
                                        .child({
                                            if already_installed {
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground)
                                                    .child("Installed")
                                                    .into_any_element()
                                            } else if has_asset {
                                                let tag_clone = tag.clone();
                                                Button::new(format!("install-release-{}", idx))
                                                    .label("Install")
                                                    .icon(IconName::Download)
                                                    .compact()
                                                    .ghost()
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        let tag = tag_clone.clone();
                                                        if let Some(release) = this
                                                            .state
                                                            .versions
                                                            .available_releases
                                                            .iter()
                                                            .find(|r| r.tag_name == tag)
                                                            .cloned()
                                                        {
                                                            if let Some(asset) =
                                                                installer_service::find_platform_asset(
                                                                    &release,
                                                                )
                                                            {
                                                                let url =
                                                                    asset.browser_download_url.clone();
                                                                let dest =
                                                                    installer_service::default_install_path()
                                                                        .join(tag.trim_start_matches('v'));

                                                                this.state.versions.install_state =
                                                                    installer_service::VersionInstallState::Downloading {
                                                                        version: tag.clone(),
                                                                        progress: 0.0,
                                                                    };
                                                                cx.notify();

                                                                cx.spawn(async move |entity, cx| {
                                                                    let dl_tag = tag.clone();
                                                                    let result = cx
                                                                        .background_executor()
                                                                        .spawn(async move {
                                                                            installer_service::download_and_extract_blocking(
                                                                                &url,
                                                                                &dest,
                                                                                &dl_tag,
                                                                                |_| {},
                                                                            )
                                                                        })
                                                                        .await;
                                                                    let _ = cx.update(|cx| {
                                                                        entity.update(cx, |this, cx| {
                                                                            match result {
                                                                                Ok(()) => {
                                                                                    this.state.versions.install_state =
                                                                                        installer_service::VersionInstallState::Complete {
                                                                                            version: tag,
                                                                                        };
                                                                                    this.state.versions.installed =
                                                                                        installer_service::scan_installed_versions();
                                                                                }
                                                                                Err(e) => {
                                                                                    this.state.versions.install_state =
                                                                                        installer_service::VersionInstallState::Error {
                                                                                            version: tag,
                                                                                            message: e,
                                                                                        };
                                                                                }
                                                                            }
                                                                            cx.notify();
                                                                        });
                                                                    });
                                                                })
                                                                .detach();
                                                            }
                                                        }
                                                    }))
                                                    .into_any_element()
                                            } else {
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground)
                                                    .child("No binary")
                                                    .into_any_element()
                                            }
                                        })
                                        .into_any_element()
                                },
                            ),
                    )
                )
            }),
        )
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
