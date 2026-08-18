//! GitHub release fetching and asset selection.

use gpui::{
    Context, InteractiveElement as _, ParentElement as _, StatefulInteractiveElement as _,
    Window, px, Styled,
};
use crate::download::{GitHubReleases, GitHubAsset};
use gpui_component::{
    ActiveTheme,
    ContextModal as _,
    modal::Modal,
    scroll::ScrollbarAxis,
    text::TextView,
    v_flex,
    StyledExt as _,
};
use super::super::{InstallerView, ReleaseInfo};

const GITHUB_ORG:  &str = "Far-Beyond-Pulsar";
const GITHUB_REPO: &str = "Pulsar-Native";
const PAGE_SIZE:   u32  = 30;

impl InstallerView {
    // ─── Release list ─────────────────────────────────────────────────────────

    /// Fetch the first page of releases, clearing any existing list.
    pub fn fetch_releases(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.loading_releases = true;
        self.current_releases_page = 1;
        self.releases.clear();
        self.selected_release_idx = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let github = GitHubReleases::new(GITHUB_ORG, GITHUB_REPO);
            match github.get_releases_page(1, PAGE_SIZE).await {
                Ok(releases) => {
                    let has_more = releases.len() as u32 >= PAGE_SIZE;
                    let infos = map_releases(releases);
                    this.update(cx, |v, cx| {
                        v.releases = infos;
                        v.loading_releases = false;
                        v.has_more_releases = has_more;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("Failed to fetch releases: {e}");
                    this.update(cx, |v, cx| {
                        v.loading_releases = false;
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Append the next page of releases to the existing list.
    pub fn load_more_releases(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.loading_more || !self.has_more_releases {
            return;
        }
        self.loading_more = true;
        self.current_releases_page += 1;
        let page = self.current_releases_page;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let github = GitHubReleases::new(GITHUB_ORG, GITHUB_REPO);
            match github.get_releases_page(page, PAGE_SIZE).await {
                Ok(releases) => {
                    let has_more = releases.len() as u32 >= PAGE_SIZE;
                    let infos = map_releases(releases);
                    this.update(cx, |v, cx| {
                        v.releases.extend(infos);
                        v.loading_more = false;
                        v.has_more_releases = has_more;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("Failed to fetch more releases: {e}");
                    this.update(cx, |v, cx| {
                        v.loading_more = false;
                        v.current_releases_page -= 1; // roll back on error
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Mark a release as selected by its index in `self.releases`.
    pub fn select_release(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_release_idx = Some(index);
        cx.notify();
    }

    /// Release notes markdown for the currently selected release.
    pub fn selected_release_notes_markdown(&self) -> String {
        self.selected_release_idx
            .and_then(|idx| self.releases.get(idx))
            .map(|r| r.body.clone())
            .filter(|body| !body.trim().is_empty())
            .unwrap_or_else(|| "No release notes available for this version.".to_string())
    }

    /// Open a centered release-notes modal for the currently selected release.
    pub fn open_release_notes_modal_for_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(idx) = self.selected_release_idx else {
            tracing::warn!("release-notes: open requested from version selection with no selected_release_idx");
            return;
        };
        let Some(release) = self.releases.get(idx) else {
            tracing::warn!("release-notes: selected_release_idx={} is out of bounds (len={})", idx, self.releases.len());
            return;
        };

        tracing::info!(
            "release-notes: opening from version selection tag={} idx={} body_len={}",
            release.tag_name,
            idx,
            release.body.len()
        );

        Self::show_release_notes_modal(
            window,
            cx,
            format!("Release Notes · {}", release.tag_name),
            self.selected_release_notes_markdown(),
        );
    }

    /// Release notes markdown for an installed version.
    ///
    /// This reads from the cached GitHub releases list already loaded in memory.
    pub fn release_notes_markdown_for_version(&self, version: &str) -> String {
        self.find_release_index_by_version(version)
            .and_then(|idx| self.releases.get(idx))
            .map(|r| r.body.clone())
            .filter(|body| !body.trim().is_empty())
            .unwrap_or_else(|| {
                format!(
                    "No release notes are cached for `{version}` yet.\n\nSelect this version in the installer flow to load release metadata."
                )
            })
    }

    /// Open a centered release-notes modal for an installed version.
    ///
    /// Uses cached release metadata first, then fetches all releases on demand.
    pub fn open_release_notes_modal_for_version(
        &mut self,
        version: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            "release-notes: open requested for installed version='{}' cached_releases={}",
            version,
            self.releases.len()
        );

        let title = if let Some(idx) = self.find_release_index_by_version(&version) {
            tracing::info!(
                "release-notes: matched installed version='{}' to cached tag='{}' at idx={}",
                version,
                self.releases[idx].tag_name,
                idx
            );
            format!("Release Notes · {}", self.releases[idx].tag_name)
        } else {
            tracing::warn!(
                "release-notes: no cached match for installed version='{}'; showing fallback content",
                version
            );
            format!("Release Notes · {}", version)
        };

        Self::show_release_notes_modal(
            window,
            cx,
            title,
            self.release_notes_markdown_for_version(&version),
        );
    }

    pub(crate) fn show_release_notes_modal(
        window: &mut Window,
        cx: &mut gpui::App,
        title: String,
        markdown: String,
    ) {
        tracing::info!(
            "release-notes: invoking window.open_modal title='{}' markdown_len={} has_active_modal_before={}",
            title,
            markdown.len(),
            window.has_active_modal(cx)
        );

        window.open_modal(cx, move |modal: Modal, window, cx| {
            tracing::debug!("release-notes: modal builder running for title='{}'", title);
            modal
                .title(title.clone())
                .width(px(760.0))
                .max_w(px(960.0))
                .overlay(true)
                .overlay_closable(false)
                .on_close(|_, _, _| {
                    tracing::info!("release-notes: modal on_close fired");
                })
                .child(
                    v_flex()
                        .h(px(560.0))
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .child(
                            v_flex()
                                .id("release-notes-modal-scroll")
                                .scrollable(ScrollbarAxis::Vertical)
                                .p_4()
                                .child(TextView::markdown(
                                    "release-notes-modal-md",
                                    markdown.clone(),
                                    window,
                                    cx,
                                )),
                        ),
                )
        });

        tracing::info!(
            "release-notes: window.open_modal returned has_active_modal_after={}",
            window.has_active_modal(cx)
        );
    }

    fn find_release_index_by_version(&self, version: &str) -> Option<usize> {
        let wanted = normalize_release_key(version);
        self.releases
            .iter()
            .position(|r| normalize_release_key(&r.tag_name) == wanted)
    }

    // ─── Asset selection ──────────────────────────────────────────────────────

    /// Pick the best release asset for a given package `prefix` + current OS/arch.
    ///
    /// Resolution order:
    /// 1. Exact expected filename(s) (platform-specific extensions, bundle vs binary).
    /// 2. Conservative prefix+OS+arch substring fallback.
    pub fn select_asset_for(
        prefix: &str,
        assets: &[GitHubAsset],
        #[allow(unused_variables)] prefer_app_bundle: bool,
        #[allow(unused_variables)] allow_app_bundle: bool,
    ) -> Option<GitHubAsset> {
        let os_token = match std::env::consts::OS {
            "linux"   => "linux",
            "macos"   => "macos",
            "windows" => "windows",
            other => {
                tracing::warn!("Unrecognised OS '{other}', cannot select asset");
                return None;
            }
        };
        let arch_token = match std::env::consts::ARCH {
            "x86_64"  => "x86_64",
            "aarch64" => "arm64",
            other => {
                tracing::warn!("Unrecognised arch '{other}', cannot select asset");
                return None;
            }
        };

        let base = format!("{prefix}-{os_token}-{arch_token}");
        let expected_names = build_expected_names(&base, prefer_app_bundle, allow_app_bundle);

        // Exact-name match first.
        if let Some(found) = expected_names.iter().find_map(|name| {
            assets
                .iter()
                .find(|a| !a.name.ends_with(".sig") && a.name == *name)
        }) {
            return Some(found.clone());
        }

        // Fallback: any asset whose name starts with the prefix and contains OS+arch tokens.
        let candidates: Vec<&GitHubAsset> = assets
            .iter()
            .filter(|a| {
                let n = &a.name;
                !n.ends_with(".sig")
                    && n.starts_with(prefix)
                    && n.contains(os_token)
                    && n.contains(arch_token)
            })
            .collect();

        tracing::info!(
            "Asset selection: os={os_token} arch={arch_token} → {} candidate(s)",
            candidates.len()
        );

        if candidates.is_empty() {
            return None;
        }

        #[cfg(target_os = "macos")]
        {
            let app_zip = candidates
                .iter()
                .find(|a| a.name.ends_with(".app.zip"))
                .map(|a| (*a).clone());
            let binary = candidates
                .iter()
                .find(|a| !a.name.ends_with(".app.zip") && !a.name.ends_with(".zip"))
                .map(|a| (*a).clone());

            return if allow_app_bundle {
                if prefer_app_bundle { app_zip.or(binary) } else { binary.or(app_zip) }
            } else {
                binary
            };
        }

        #[cfg(not(target_os = "macos"))]
        candidates.into_iter().next().cloned()
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn normalize_release_key(version: &str) -> String {
    version.trim().trim_start_matches(['v', 'V']).to_ascii_lowercase()
}

fn map_releases(releases: Vec<crate::download::GitHubRelease>) -> Vec<ReleaseInfo> {
    releases
        .into_iter()
        .map(|r| ReleaseInfo {
            tag_name: r.tag_name,
            name: r.name,
            body: r.body,
            prerelease: r.prerelease,
        })
        .collect()
}

/// Build the ordered list of expected asset filenames to try for the current platform.
fn build_expected_names(
    base: &str,
    #[allow(unused_variables)] prefer_app_bundle: bool,
    #[allow(unused_variables)] allow_app_bundle: bool,
) -> Vec<String> {
    let mut names = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if allow_app_bundle {
            if prefer_app_bundle {
                names.push(format!("{base}.app.zip"));
                names.push(base.to_string());
            } else {
                names.push(base.to_string());
                names.push(format!("{base}.app.zip"));
            }
        } else {
            names.push(base.to_string());
        }
    }

    #[cfg(target_os = "windows")]
    {
        names.push(format!("{base}.exe"));
        names.push(base.to_string());
    }

    #[cfg(target_os = "linux")]
    {
        names.push(base.to_string());
        names.push(format!("{base}.tar.gz"));
        names.push(format!("{base}.tgz"));
        names.push(format!("{base}.zip"));
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    names.push(base.to_string());

    names
}
