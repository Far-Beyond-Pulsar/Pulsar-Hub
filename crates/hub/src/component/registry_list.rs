//! One list for everything a registry offers (plugins, themes): a bordered
//! panel with a search box, a virtualized list of fixed-height rows, an
//! Installed badge or an Install button on each, and the registry's refresh
//! button and status above it.
//!
//! The rows are a pure function of the screen's data and the search text
//! ([`plugin_rows`], [`theme_rows`]); the delegate asks for them when the list
//! draws, so there is no copy to keep in sync. Only the rows in view are drawn.

use gpui::prelude::*;
use gpui::*;
use pulsar_install_layout::ThemeRecord;
use smallvec::SmallVec;
use ui::list::{List, ListDelegate};
use ui::{
    ActiveTheme as _, Disableable as _, Icon, IconName, IndexPath, Selectable, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};

use crate::EntryScreen;
use crate::core::types::{InstalledPlugin, PluginInstallPhase, RegistryPlugin};
use crate::service::theme_service::RegistryTheme;

/// Every row has this height, so the list measures one and virtualizes the rest.
const ROW_HEIGHT: f32 = 84.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryKind {
    Plugins,
    Themes,
}

impl RegistryKind {
    fn icon(self) -> IconName {
        match self {
            Self::Plugins => IconName::Package,
            Self::Themes => IconName::Palette,
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Self::Plugins => "plugins",
            Self::Themes => "themes",
        }
    }
}

/// One row of a registry list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowData {
    /// What the actions act on: a plugin's repository URL, a theme's file name.
    pub key: String,
    pub title: String,
    /// Author, version, variants: one muted line.
    pub meta: String,
    pub description: String,
    pub installed: bool,
}

fn matches_query(query: &str, fields: &[&str]) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty() || fields.iter().any(|field| field.to_lowercase().contains(&query))
}

/// The plugin rows: the registry's plugins matching `query`, then the
/// installed ones no registry lists ("custom").
pub fn plugin_rows(
    registry: &[RegistryPlugin],
    installed: &[InstalledPlugin],
    query: &str,
) -> Vec<RowData> {
    let version_of = |url: &str| {
        installed
            .iter()
            .find(|plugin| plugin.repo_url == url)
            .map(|plugin| plugin.version.clone())
    };
    let mut rows: Vec<RowData> = registry
        .iter()
        .filter(|plugin| {
            matches_query(
                query,
                &[
                    &plugin.name,
                    &plugin.description,
                    &plugin.author,
                    &plugin.tags.join(" "),
                ],
            )
        })
        .map(|plugin| {
            let version = version_of(&plugin.repo_url);
            let mut meta = Vec::new();
            if !plugin.author.is_empty() {
                meta.push(format!("by {}", plugin.author));
            }
            if let Some(version) = &version {
                meta.push(format!("installed {version}"));
            }
            if !plugin.tags.is_empty() {
                meta.push(plugin.tags.join(", "));
            }
            RowData {
                key: plugin.repo_url.clone(),
                title: plugin.name.clone(),
                meta: meta.join(" · "),
                description: plugin.description.clone(),
                installed: version.is_some(),
            }
        })
        .collect();
    rows.extend(
        installed
            .iter()
            .filter(|plugin| !registry.iter().any(|entry| entry.repo_url == plugin.repo_url))
            .filter(|plugin| matches_query(query, &[&plugin.name, &plugin.repo_url]))
            .map(|plugin| RowData {
                key: plugin.repo_url.clone(),
                title: plugin.name.clone(),
                meta: format!("custom · installed {}", plugin.version),
                description: plugin.repo_url.clone(),
                installed: true,
            }),
    );
    rows
}

/// The theme rows, laid out like the plugin rows.
pub fn theme_rows(
    registry: &[RegistryTheme],
    installed: &[ThemeRecord],
    query: &str,
) -> Vec<RowData> {
    let is_installed = |file: &str| installed.iter().any(|record| record.file == file);
    let mut rows: Vec<RowData> = registry
        .iter()
        .filter(|theme| matches_query(query, &[&theme.name, &theme.author, &theme.variants.join(" ")]))
        .map(|theme| {
            let mut meta = Vec::new();
            if !theme.author.is_empty() {
                meta.push(format!("by {}", theme.author));
            }
            meta.push(format!(
                "{} variant{}",
                theme.variants.len(),
                if theme.variants.len() == 1 { "" } else { "s" }
            ));
            RowData {
                key: theme.file.clone(),
                title: theme.name.clone(),
                meta: meta.join(" · "),
                description: theme.variants.join(", "),
                installed: is_installed(&theme.file),
            }
        })
        .collect();
    rows.extend(
        installed
            .iter()
            .filter(|record| !registry.iter().any(|theme| theme.file == record.file))
            .filter(|record| matches_query(query, &[&record.name, &record.file]))
            .map(|record| RowData {
                key: record.file.clone(),
                title: record.name.clone(),
                meta: "no longer in a registry".to_string(),
                description: record.file.clone(),
                installed: true,
            }),
    );
    rows
}

/// Reads the live screen state, so the list is always current, and turns the
/// row buttons into calls on the screen.
pub struct RegistryListDelegate {
    screen: WeakEntity<EntryScreen>,
    kind: RegistryKind,
    query: String,
    selected: Option<IndexPath>,
}

impl RegistryListDelegate {
    pub fn new(screen: WeakEntity<EntryScreen>, kind: RegistryKind) -> Self {
        Self { screen, kind, query: String::new(), selected: None }
    }

    fn rows(&self, cx: &App) -> Vec<RowData> {
        let Some(screen) = self.screen.upgrade() else {
            return Vec::new();
        };
        let state = &screen.read(cx).state;
        match self.kind {
            RegistryKind::Plugins => {
                plugin_rows(&state.registry_plugins, &state.installed_plugins, &self.query)
            }
            RegistryKind::Themes => {
                theme_rows(&state.registry_themes, &state.installed_themes, &self.query)
            }
        }
    }

    fn refreshing(&self, cx: &App) -> bool {
        self.screen
            .upgrade()
            .map(|screen| {
                let state = &screen.read(cx).state;
                match self.kind {
                    RegistryKind::Plugins => state.registry_refresh_in_progress,
                    RegistryKind::Themes => state.theme_refresh_in_progress,
                }
            })
            .unwrap_or(false)
    }

    /// A plugin install is running: its Install buttons wait.
    fn install_busy(&self, cx: &App) -> bool {
        self.kind == RegistryKind::Plugins
            && self
                .screen
                .upgrade()
                .map(|screen| {
                    matches!(
                        screen.read(cx).state.plugin_install_phase,
                        Some(
                            PluginInstallPhase::FetchingMetadata
                                | PluginInstallPhase::Downloading { .. }
                                | PluginInstallPhase::Building { .. }
                        )
                    )
                })
                .unwrap_or(false)
    }
}

impl ListDelegate for RegistryListDelegate {
    type Item = RegistryItem;

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<List<Self>>,
    ) -> Task<()> {
        self.query = query.to_string();
        Task::ready(())
    }

    fn items_count(&self, _section: usize, cx: &App) -> usize {
        self.rows(cx).len()
    }

    fn render_item(
        &self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<List<Self>>,
    ) -> Option<Self::Item> {
        let row = self.rows(cx).into_iter().nth(ix.row)?;
        let theme = cx.theme().clone();
        let busy = self.install_busy(cx);
        let kind = self.kind;
        let screen = self.screen.clone();
        let key = row.key.clone();

        let action: AnyElement = if row.installed {
            let (screen, key) = (screen.clone(), key.clone());
            h_flex()
                .gap_1()
                .items_center()
                .child(
                    div()
                        .px_2()
                        .py(px(2.))
                        .rounded_full()
                        .bg(theme.success_foreground.opacity(0.15))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.success_foreground)
                        .child("Installed"),
                )
                .child(
                    Button::new(SharedString::from(format!("remove-{key}")))
                        .icon(IconName::Trash)
                        .compact()
                        .ghost()
                        .tooltip(match kind {
                            RegistryKind::Plugins => "Remove plugin",
                            RegistryKind::Themes => "Remove theme",
                        })
                        .on_click(move |_, _, cx| {
                            let _ = screen.update(cx, |this, cx| match kind {
                                RegistryKind::Plugins => {
                                    if let Some(index) = this
                                        .state
                                        .installed_plugins
                                        .iter()
                                        .position(|plugin| plugin.repo_url == key)
                                    {
                                        this.remove_plugin(index, cx);
                                    }
                                }
                                RegistryKind::Themes => this.remove_installed_theme(&key, cx),
                            });
                        }),
                )
                .into_any_element()
        } else {
            let (screen, key) = (screen.clone(), key.clone());
            Button::new(SharedString::from(format!("install-{key}")))
                .label("Install")
                .primary()
                .compact()
                .disabled(busy)
                .on_click(move |_, _, cx| {
                    let _ = screen.update(cx, |this, cx| match kind {
                        RegistryKind::Plugins => {
                            if let Some(plugin) = this
                                .state
                                .registry_plugins
                                .iter()
                                .find(|plugin| plugin.repo_url == key)
                                .cloned()
                            {
                                this.install_registry_plugin(plugin, cx);
                            }
                        }
                        RegistryKind::Themes => this.install_registry_theme(&key, cx),
                    });
                })
                .into_any_element()
        };

        Some(
            RegistryItem::new(format!("registry-row-{}", row.key))
                .installed(row.installed)
                .child(
                    h_flex()
                        .size_full()
                        .gap_3()
                        .items_center()
                        .child(
                            Icon::new(kind.icon())
                                .size_5()
                                .text_color(theme.muted_foreground),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .truncate()
                                        .child(row.title),
                                )
                                .when(!row.meta.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .truncate()
                                            .child(row.meta),
                                    )
                                })
                                .when(!row.description.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .max_h(px(32.))
                                            .overflow_hidden()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(row.description),
                                    )
                                }),
                        )
                        .child(div().flex_shrink_0().child(action)),
                ),
        )
    }

    fn loading(&self, cx: &App) -> bool {
        self.refreshing(cx) && self.rows(cx).is_empty()
    }

    fn render_empty(&self, _window: &mut Window, cx: &mut Context<List<Self>>) -> impl IntoElement {
        let theme = cx.theme();
        let text = if self.query.trim().is_empty() {
            format!("No {} found. Refresh to read the registry again.", self.kind.noun())
        } else {
            format!("No {} match \"{}\".", self.kind.noun(), self.query.trim())
        };
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .child(Icon::new(IconName::Inbox).size_8().text_color(theme.muted_foreground.opacity(0.6)))
            .child(div().text_sm().text_color(theme.muted_foreground).child(text))
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<List<Self>>,
    ) {
        self.selected = ix;
    }
}

/// One row: a fixed-height card the list can virtualize.
#[derive(IntoElement)]
pub struct RegistryItem {
    base: Stateful<Div>,
    children: SmallVec<[AnyElement; 1]>,
    selected: bool,
    installed: bool,
}

impl RegistryItem {
    fn new(id: impl Into<ElementId>) -> Self {
        Self { base: div().id(id), children: SmallVec::new(), selected: false, installed: false }
    }

    fn installed(mut self, installed: bool) -> Self {
        self.installed = installed;
        self
    }
}

impl Selectable for RegistryItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Styled for RegistryItem {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for RegistryItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for RegistryItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        self.base
            .w_full()
            .h(px(ROW_HEIGHT))
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border.opacity(0.6))
            .overflow_hidden()
            .when(self.installed, |this| this.bg(theme.success_foreground.opacity(0.04)))
            .when(self.selected, |this| this.bg(theme.accent.opacity(0.08)))
            .children(self.children)
    }
}

/// The list's panel: the registry's status line and refresh button, the
/// install status or error, and the bordered, searchable, virtualized list.
pub fn render_registry_panel(
    screen: &mut EntryScreen,
    kind: RegistryKind,
    cx: &mut Context<EntryScreen>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let (list, refreshing, available, installed, banner) = match kind {
        RegistryKind::Plugins => (
            screen.plugin_list.clone(),
            screen.state.registry_refresh_in_progress,
            screen.state.registry_plugins.len(),
            screen.state.installed_plugins.len(),
            screen
                .state
                .plugin_install_phase
                .clone()
                .map(|phase| {
                    crate::screen::views::onboarding::render_plugin_phase(phase, cx)
                        .into_any_element()
                }),
        ),
        RegistryKind::Themes => (
            screen.theme_list.clone(),
            screen.state.theme_refresh_in_progress,
            screen.state.registry_themes.len(),
            screen.state.installed_themes.len(),
            screen.state.theme_error.clone().map(|error| {
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(theme.danger.opacity(0.12))
                    .text_sm()
                    .text_color(theme.danger)
                    .child(error)
                    .into_any_element()
            }),
        ),
    };
    let refresh_id = match kind {
        RegistryKind::Plugins => "refresh-plugin-registry",
        RegistryKind::Themes => "refresh-theme-registry",
    };

    v_flex()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "{available} {} in the registry · {installed} installed",
                            kind.noun()
                        )),
                )
                .child(
                    Button::new(refresh_id)
                        .icon(IconName::Refresh)
                        .compact()
                        .ghost()
                        .disabled(refreshing)
                        .tooltip(if refreshing { "Refreshing..." } else { "Refresh the registry" })
                        .on_click(cx.listener(move |this, _, _, cx| match kind {
                            RegistryKind::Plugins => this.refresh_plugin_registry(cx),
                            RegistryKind::Themes => this.refresh_theme_registry(cx),
                        })),
                ),
        )
        .when_some(banner, |this, banner| this.child(banner))
        // The border and the clipping belong to this container, so the list's
        // own scrolling never reaches past it.
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .bg(theme.secondary.opacity(0.08))
                .child(list),
        )
}

#[cfg(test)]
mod tests {
    use super::{RowData, matches_query, plugin_rows, theme_rows};
    use crate::core::types::{InstalledPlugin, PluginInstallMethod, RegistryPlugin};
    use crate::service::theme_service::RegistryTheme;
    use pulsar_install_layout::ThemeRecord;

    fn plugin(name: &str, repo: &str, tags: &[&str]) -> RegistryPlugin {
        RegistryPlugin {
            name: name.into(),
            description: format!("{name} for Pulsar"),
            repo_url: repo.into(),
            author: "Far-Beyond-Pulsar".into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            registry_url: String::new(),
        }
    }

    fn installed(name: &str, repo: &str) -> InstalledPlugin {
        InstalledPlugin {
            name: name.into(),
            repo_url: repo.into(),
            version: "v1".into(),
            installed_at: String::new(),
            install_method: PluginInstallMethod::BinaryDownload,
            library_path: String::new(),
        }
    }

    #[test]
    fn the_query_matches_name_description_author_and_tags_in_any_case() {
        assert!(matches_query("", &["x"]));
        assert!(matches_query("  WATER ", &["Water volumes"]));
        assert!(!matches_query("fog", &["Water volumes"]));
    }

    #[test]
    fn registry_plugins_come_first_then_custom_installed_ones() {
        let registry = [plugin("Water", "r/water", &["render"]), plugin("Fog", "r/fog", &["render"])];
        let installed = [installed("Water", "r/water"), installed("Mine", "x/mine")];
        let rows = plugin_rows(&registry, &installed, "");
        let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, ["r/water", "r/fog", "x/mine"]);
        assert!(rows[0].installed && !rows[1].installed && rows[2].installed);
        assert!(rows[2].meta.starts_with("custom"));
    }

    #[test]
    fn searching_filters_both_the_registry_and_the_custom_plugins() {
        let registry = [plugin("Water", "r/water", &["render"]), plugin("Fog", "r/fog", &["render"])];
        let installed = [installed("Mine", "x/mine")];
        assert_eq!(plugin_rows(&registry, &installed, "fog").len(), 1);
        assert_eq!(plugin_rows(&registry, &installed, "mine")[0].key, "x/mine");
        assert!(plugin_rows(&registry, &installed, "zzz").is_empty());
    }

    #[test]
    fn theme_rows_mark_installed_and_keep_orphans() {
        let registry = [RegistryTheme {
            name: "Nord".into(),
            author: "A".into(),
            variants: vec!["Nord Dark".into()],
            file: "nord.json".into(),
            registry_url: String::new(),
            json: String::new(),
        }];
        let records = [
            ThemeRecord {
                name: "Nord".into(),
                file: "nord.json".into(),
                sha256: String::new(),
                registry_url: String::new(),
                installed_at: String::new(),
            },
            ThemeRecord {
                name: "Old".into(),
                file: "old.json".into(),
                sha256: String::new(),
                registry_url: String::new(),
                installed_at: String::new(),
            },
        ];
        let rows: Vec<RowData> = theme_rows(&registry, &records, "");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].installed);
        assert_eq!(rows[0].meta, "by A · 1 variant");
        assert_eq!(rows[1].meta, "no longer in a registry");
    }
}
