use gpui::prelude::*;
use gpui::*;
use smallvec::SmallVec;
use ui::list::{List, ListDelegate};
use ui::IndexPath;
use ui::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Icon, IconName, Selectable, StyledExt as _, text::TextView,
};

use crate::service::installer_service::{self, GitHubRelease};
use crate::EntryScreen;

/// Fixed height (px) for each release card, kept uniform so the virtualized
/// list can reuse a single measured row height.
const ROW_HEIGHT: f32 = 240.0;
/// Number of skeleton rows appended while the next page is being fetched.
const PLACEHOLDER_ROWS: usize = 3;
/// Number of remaining entities that trigger `load_more` while scrolling.
const LOAD_MORE_THRESHOLD: usize = 8;

/// Delegate that renders the engine release list. It reads the live
/// `EntryScreen` state so pagination/load-more flags stay in sync, and it
/// appends skeleton placeholder rows on screen while the next page is loading.
pub struct ReleaseListDelegate {
    screen: WeakEntity<EntryScreen>,
}

impl ReleaseListDelegate {
    pub fn new(screen: WeakEntity<EntryScreen>) -> Self {
        Self { screen }
    }

    fn loaded(&self, cx: &App) -> usize {
        self.screen
            .upgrade()
            .map(|s| {
                s.read(cx).state.versions.available_releases
                    .iter()
                    .filter(|r| !r.prerelease)
                    .count()
            })
            .unwrap_or(0)
    }

    fn loading_more(&self, cx: &App) -> bool {
        self.screen
            .upgrade()
            .map(|s| s.read(cx).state.versions.loading_more)
            .unwrap_or(false)
    }

    fn release_at(&self, cx: &App, row: usize) -> Option<GitHubRelease> {
        self.screen.upgrade()?.read(cx).state.versions.available_releases
            .iter()
            .filter(|r| !r.prerelease)
            .nth(row)
            .cloned()
    }

    fn installed_versions(&self, cx: &App) -> std::collections::HashSet<String> {
        self.screen
            .upgrade()
            .map(|s| {
                s.read(cx).state.versions.installed
                    .iter()
                    .map(|v| v.metadata.version.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn render_card(
        &self,
        row: usize,
        release: GitHubRelease,
        installed: std::collections::HashSet<String>,
        window: &mut Window,
        cx: &mut Context<List<Self>>,
    ) -> ReleaseItem {
        let theme = cx.theme();
        let tag = release.tag_name.clone();
        let name = release.name.clone();
        let body = release.body.clone();
        let has_asset = installer_service::find_platform_asset(&release).is_some();
        let already_installed = installed.contains(tag.trim_start_matches('v'));
        let weak = self.screen.clone();
        let details_view = self
            .screen
            .upgrade()
            .map(|s| s.read(cx).state.release_details_view.clone());

        let highlights = extract_summary(&body);
        let mut item = ReleaseItem::new(format!("release-{}", row));
        let tag_display = tag.clone();
        item = item.child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::new(IconName::Label).size(px(16.)).text_color(theme.accent))
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child(tag_display),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(name),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child({
                            if already_installed {
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("Installed")
                                    .into_any_element()
                            } else if has_asset {
                                let weak = weak.clone();
                                let install_tag = tag.clone();
                                Button::new(format!("install-{}", row))
                                    .label("Install")
                                    .icon(IconName::Download)
                                    .compact()
                                    .on_click(move |_, _, cx| {
                                        let weak = weak.clone();
                                        if let Some(e) = weak.upgrade() {
                                            let _ = e.update(cx, |this, cx| {
                                                this.install_release_by_tag(install_tag.clone(), cx);
                                            });
                                        }
                                    })
                                    .into_any_element()
                            } else {
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("No binary")
                                    .into_any_element()
                            }
                        })
                        .when_some(details_view, |this, dv| {
                            let tag = tag.clone();
                            let body = body.clone();
                            this.child(
                                ui::popover::Popover::<
                                    crate::screen::views::release_details::ReleaseDetailsView,
                                >::new(format!("release-details-popover-{}", row))
                                    .anchor(Corner::TopRight)
                                    .trigger(
                                        Button::new(format!("release-details-{}", row))
                                            .label("Details")
                                            .compact()
                                            .ghost()
                                            .tooltip("Full release notes"),
                                    )
                                    .content(move |_, cx| {
                                        let t = tag.clone();
                                        let b = body.clone();
                                        let _ = dv.update(cx, |v, cx| v.set_content(t, b, cx));
                                        dv.clone()
                                    }),
                            )
                        }),
                ),
        );
        if !highlights.is_empty() {
            let body_id = format!("release-body-{}-{}", row, tag);
            item = item.child(
                div()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .mt_2()
                    .overflow_hidden()
                    .child(TextView::markdown(body_id, highlights, window, cx)),
            );
        }
        item
    }
}

impl ListDelegate for ReleaseListDelegate {
    type Item = ReleaseItem;

    fn items_count(&self, _: usize, cx: &App) -> usize {
        self.loaded(cx) + if self.loading_more(cx) { PLACEHOLDER_ROWS } else { 0 }
    }

    fn render_item(
        &self,
        ix: IndexPath,
        window: &mut Window,
        cx: &mut Context<List<Self>>,
    ) -> Option<Self::Item> {
        if let Some(release) = self.release_at(cx, ix.row) {
            let installed = self.installed_versions(cx);
            Some(self.render_card(ix.row, release, installed, window, cx))
        } else if self.loading_more(cx) {
            Some(Self::skeleton_row(ix.row))
        } else {
            None
        }
    }

    fn set_selected_index(
        &mut self,
        _: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<List<Self>>,
    ) {
    }

    fn is_eof(&self, cx: &App) -> bool {
        self.screen
            .upgrade()
            .map(|s| {
                let v = &s.read(cx).state.versions;
                v.has_more && !v.loading_more
            })
            .unwrap_or(false)
    }

    fn load_more_threshold(&self) -> usize {
        LOAD_MORE_THRESHOLD
    }

    fn load_more(&mut self, _: &mut Window, cx: &mut Context<List<Self>>) {
        if let Some(s) = self.screen.upgrade() {
            let _ = s.update(cx, |this, cx| this.load_more_releases(cx));
        }
    }

    fn render_empty(
        &self,
        _: &mut Window,
        cx: &mut Context<List<Self>>,
    ) -> impl IntoElement {
        let fetching = self
            .screen
            .upgrade()
            .map(|s| s.read(cx).state.versions.fetching)
            .unwrap_or(false);
        v_flex()
            .w_full()
            .items_center()
            .justify_center()
            .py_12()
            .gap_3()
            .child(ui::spinner::Spinner::new().color(cx.theme().muted_foreground))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if fetching {
                        "Fetching releases..."
                    } else {
                        "Could not load releases. Click Refresh to try again."
                    }),
            )
    }
}

impl ReleaseListDelegate {
    /// A skeleton row that mirrors a release card, rendered while paging.
    fn skeleton_row(row: usize) -> ReleaseItem {
        let mut row_item = ReleaseItem::new(format!("release-skeleton-{}", row));
        row_item = row_item.child(
            h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .child(ui::skeleton::Skeleton::new().h_4().w_40())
                .child(
                    ui::skeleton::Skeleton::new()
                        .secondary(true)
                        .h_3()
                        .w_24(),
                ),
        );
        row_item = row_item.child(
            v_flex()
                .w_full()
                .mt_2()
                .gap_1()
                .child(ui::skeleton::Skeleton::new().secondary(true).h_3().w_full())
                .child(
                    ui::skeleton::Skeleton::new()
                        .secondary(true)
                        .h_3()
                        .w_full(),
                ),
        );
        row_item
    }
}

/// A single release row. Implements [`Selectable`] so it can be used by the
/// component `List`, and is laid out as a fixed-height column card with an
/// internally scrollable markdown body.
#[derive(IntoElement)]
pub struct ReleaseItem {
    base: Stateful<Div>,
    children: SmallVec<[AnyElement; 3]>,
    selected: bool,
}

impl ReleaseItem {
    fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: div().id(id),
            children: SmallVec::new(),
            selected: false,
        }
    }
}

impl Selectable for ReleaseItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Styled for ReleaseItem {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for ReleaseItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ReleaseItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        self.base
            .w_full()
            .h(px(ROW_HEIGHT))
            .my_2()
            .flex_col()
            .overflow_hidden()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .when(self.selected, |this| {
                this.bg(theme.accent.opacity(0.08)).border_color(theme.accent)
            })
            .children(self.children)
    }
}

/// The summary shown on a release card: the release notes "Highlights"
/// section if present, otherwise the first section that follows the last
/// "Container Images" section.
pub(crate) fn extract_summary(markdown: &str) -> String {
    let highlights = extract_highlights(markdown);
    if !highlights.is_empty() {
        return highlights;
    }
    extract_first_section_after_container_images(markdown)
}

/// Return the release notes "Highlights" section only.
///
/// Picks the first heading line (of any level) whose text contains the
/// (case-insensitive) word "Highlights", ignoring any other surrounding
/// characters. Its contents run from that heading until the next heading
/// (of any level) or the end of the notes. Returns an empty string when no
/// such heading exists.
pub(crate) fn extract_highlights(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();

    let mut start: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(text) = heading_text(line) {
            if text.to_lowercase().contains("highlights") {
                start = Some(i);
                break;
            }
        }
    }
    let Some(start) = start else {
        return String::new();
    };

    let mut end = lines.len();
    for j in (start + 1)..lines.len() {
        if is_heading(lines[j]) {
            end = j;
            break;
        }
    }

    lines[start..end].join("\n").trim().to_string()
}

/// If `line` is a Markdown heading, return its text with the leading `#`
/// markers and whitespace stripped.
fn heading_text(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if hashes == 0 {
        return None;
    }
    Some(t[hashes..].trim_start())
}

fn is_heading(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// Fallback when a release has no "Highlights" section: return the first
/// section that appears after the *last* heading containing "Container Images".
fn extract_first_section_after_container_images(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();

    let mut last_ci: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(text) = heading_text(line) {
            if text.to_lowercase().contains("container images") {
                last_ci = Some(i);
            }
        }
    }
    let Some(ci) = last_ci else { return String::new(); };

    let mut start: Option<usize> = None;
    for (i, line) in lines.iter().enumerate().skip(ci + 1) {
        if is_heading(line) {
            start = Some(i);
            break;
        }
    }
    let Some(start) = start else { return String::new(); };

    let mut end = lines.len();
    for j in (start + 1)..lines.len() {
        if is_heading(lines[j]) {
            end = j;
            break;
        }
    }

    lines[start..end].join("\n").trim().to_string()
}
