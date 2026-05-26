//! GitHub release fetching and asset selection.

use gpui::{Context, Window};
use crate::download::{GitHubReleases, GitHubAsset};
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

fn map_releases(releases: Vec<crate::download::GitHubRelease>) -> Vec<ReleaseInfo> {
    releases
        .into_iter()
        .map(|r| ReleaseInfo {
            tag_name: r.tag_name,
            name: r.name,
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
