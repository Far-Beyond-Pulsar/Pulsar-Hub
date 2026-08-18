use gpui::prelude::*;
use gpui::*;
use ui::{
    button::{Button, ButtonVariants as _},
    h_flex, scroll::ScrollbarAxis, spinner::Spinner, v_flex, ActiveTheme as _, Icon, IconName,
    StyledExt as _,
};

use crate::core::types::{DownloadItem, DownloadKind, format_bytes};
use crate::service::installer_service;
use crate::EntryScreen;

pub fn render_versions(
    screen: &mut EntryScreen,
    window: &mut Window,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    if screen.state.ui.show_install_modal {
        return render_install_modal(screen, cx).into_any_element();
    }

    render_installed_grid(screen, cx).into_any_element()
}

fn render_installed_grid(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let installed = screen.state.versions.installed.clone();

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
                            Button::new("btn-install")
                                .label("Install")
                                .icon(IconName::Download)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state.ui.show_install_modal = true;
                                    if this.state.versions.available_releases.is_empty() {
                                        this.refresh_versions(cx);
                                    }
                                    cx.notify();
                                })),
                        ),
                ),
        )
        .child(div().w_full().h(px(1.0)).bg(theme.border))
        .child(
            v_flex()
                .id("installed-versions-list")
                .w_full()
                .flex_1()
                .scrollable(ScrollbarAxis::Vertical)
                .gap_3()
                .when(installed.is_empty(), |this| {
                    this.child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .justify_center()
                            .py_12()
                            .gap_3()
                            .child(Spinner::new().color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Click \"Install\" to get started"),
                            ),
                    )
                })
                .children(installed.iter().enumerate().map(|(idx, ver)| {
                    let version = ver.metadata.version.clone();
                    let date = ver.metadata.install_date.clone();
                    let size = format_bytes(ver.disk_size_bytes);
                    let path = ver.metadata.install_path.clone();
                    let path_clone = path.clone();

                    v_flex()
                        .id(format!("version-card-{}", idx))
                        .w(px(280.))
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border)
                        .hover(|this| this.bg(theme.accent.opacity(0.05)))
                        .gap_3()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Icon::new(IconName::Package)
                                        .size(px(18.))
                                        .text_color(theme.accent),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(theme.foreground)
                                        .child(format!("v{}", version)),
                                ),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_md()
                                        .bg(theme.accent.opacity(0.12))
                                        .text_color(theme.accent)
                                        .child(size),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(if date.is_empty() {
                                            "Unknown date".to_string()
                                        } else {
                                            date
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(path.display().to_string()),
                        )
                        .child(div().w_full().h(px(1.0)).bg(theme.border))
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
                })),
        )
}

fn render_install_modal(
    screen: &mut EntryScreen,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme();
    let releases = screen.state.versions.available_releases.clone();
    let installed = screen.state.versions.installed.clone();
    let fetching = screen.state.versions.fetching;

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
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            Button::new("btn-back")
                                .icon(IconName::ArrowLeft)
                                .compact()
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state.ui.show_install_modal = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.foreground)
                                .child("Install Engine"),
                        ),
                )
                .child(
                    Button::new("btn-refresh-releases")
                        .label("Refresh")
                        .icon(IconName::Refresh)
                        .compact()
                        .ghost()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_versions(cx);
                        })),
                ),
        )
        .child(div().w_full().h(px(1.0)).bg(theme.border))
        .child(
            v_flex()
                .id("install-releases-list")
                .w_full()
                .flex_1()
                .scrollable(ScrollbarAxis::Vertical)
                .gap_3()
                .when(fetching && releases.is_empty(), |this| {
                    this.child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .justify_center()
                            .py_12()
                            .gap_3()
                            .child(Spinner::new().color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Fetching releases..."),
                            ),
                    )
                })
                .when(!fetching && releases.is_empty(), |this| {
                    this.child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .justify_center()
                            .py_12()
                            .gap_3()
                            .child(Spinner::new().color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("Could not load releases. Click Refresh to try again."),
                            ),
                    )
                })
                .children(
                    releases
                        .iter()
                        .filter(|r| !r.prerelease)
                        .enumerate()
                        .map(|(idx, release)| {
                            let tag = release.tag_name.clone();
                            let name = release.name.clone();
                            let body = release.body.clone();
                            let has_asset =
                                installer_service::find_platform_asset(release).is_some();
                            let already_installed = installed
                                .iter()
                                .any(|v| v.metadata.version == tag.trim_start_matches('v'));

                            v_flex()
                                .id(format!("release-{}", idx))
                                .w_full()
                                .p_4()
                                .rounded_lg()
                                .border_1()
                                .border_color(theme.border)
                                .hover(|this| this.bg(theme.accent.opacity(0.05)))
                                .gap_2()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    Icon::new(IconName::Label)
                                                        .size(px(16.))
                                                        .text_color(theme.accent),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(theme.foreground)
                                                        .child(tag.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .child(name),
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
                                                Button::new(format!("install-{}", idx))
                                                    .label("Install")
                                                    .icon(IconName::Download)
                                                    .compact()
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
                                                                    asset.browser_download_url
                                                                        .clone();
                                                                let dest =
                                                                    installer_service::default_install_path(
                                                                    )
                                                                    .join(
                                                                        tag.trim_start_matches('v'),
                                                                    );

                                                                let dl_id = format!(
                                                                    "engine-{}",
                                                                    tag
                                                                );
                                                                let dm_view = this.state.download_manager_view.clone();
                                                                dm_view.update(cx, |view, cx| {
                                                                    view.add_item(DownloadItem {
                                                                        id: dl_id.clone(),
                                                                        kind: DownloadKind::EngineVersion {
                                                                            version: tag.clone(),
                                                                        },
                                                                        status: crate::core::types::DownloadStatus::Downloading {
                                                                            bytes_downloaded: 0,
                                                                            total_bytes: 0,
                                                                            speed_bps: 0,
                                                                        },
                                                                        started_at: std::time::Instant::now(),
                                                                    });
                                                                    cx.notify();
                                                                });
                                                                this.state.versions.install_state =
                                                                    installer_service::VersionInstallState::Downloading {
                                                                        version: tag.clone(),
                                                                        progress: 0.0,
                                                                    };
                                                                cx.notify();

                                                                cx.spawn(async move |entity, cx| {
                                                                    let dl_tag = tag.clone();
                                                                    let dl_id = dl_id.clone();
                                                                    let progress = std::sync::Arc::new(parking_lot::Mutex::new(
                                                                        installer_service::DownloadProgress::default(),
                                                                    ));
                                                                    let progress_clone = progress.clone();
                                                                    let dm_view = dm_view;

                                                                    let download_task = cx
                                                                        .background_executor()
                                                                        .spawn(async move {
                                                                            installer_service::download_and_extract_with_progress(
                                                                                &url,
                                                                                &dest,
                                                                                &dl_tag,
                                                                                progress_clone,
                                                                            )
                                                                        });

                                                                    loop {
                                                                        cx.background_executor()
                                                                            .timer(std::time::Duration::from_millis(150))
                                                                            .await;

                                                                        let snapshot = {
                                                                            let p = progress.lock();
                                                                            (p.bytes_downloaded, p.total_bytes, p.speed_bps, p.done, p.error.clone())
                                                                        };

                                                                        let (bytes, total, speed, done, error) = snapshot;
                                                                        let _ = cx.update(|cx| {
                                                                            let _ = entity.update(cx, |this, cx| {
                                                                                if !done {
                                                                                    dm_view.update(cx, |view, cx| {
                                                                                        view.update_progress(&dl_id, bytes, total, speed);
                                                                                        cx.notify();
                                                                                    });
                                                                                }
                                                                                cx.notify();
                                                                            });
                                                                        });

                                                                        if done {
                                                                            break;
                                                                        }
                                                                    }

                                                                    let _ = cx.update(|cx| {
                                                                        let _ = entity.update(cx, |this, cx| {
                                                                            let p = progress.lock();
                                                                            if let Some(ref e) = p.error {
                                                                                dm_view.update(cx, |view, cx| {
                                                                                    view.fail(&dl_id, e.clone());
                                                                                    cx.notify();
                                                                                });
                                                                                this.state.versions.install_state =
                                                                                    installer_service::VersionInstallState::Error {
                                                                                        version: tag,
                                                                                        message: e.clone(),
                                                                                    };
                                                                            } else {
                                                                                dm_view.update(cx, |view, cx| {
                                                                                    view.complete(&dl_id);
                                                                                    cx.notify();
                                                                                });
                                                                                this.state.versions.install_state =
                                                                                    installer_service::VersionInstallState::Complete {
                                                                                        version: tag,
                                                                                    };
                                                                                this.state.versions.installed =
                                                                                    installer_service::scan_installed_versions();
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
                                        }),
                                )
                                .when(!body.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .max_h_48()
                                            .overflow_y_scroll()
                                            .child(body),
                                    )
                                })
                                .into_any_element()
                        }),
                ),
        )
}

