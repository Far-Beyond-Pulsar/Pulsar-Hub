use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use ui::{v_flex, ActiveTheme as _, TitleBar};

/// Hosts the upstream `ModernSettingsScreen` inside a hub-owned OS window.
///
/// Upstream's `SettingsWindow` reads the project path from `EngineContext`
/// globals, which the hub never populates — so we replicate its thin shell
/// (title bar + screen) and pass an explicit project path instead.
pub struct HubSettingsWindow {
    screen: Entity<ui_settings::ModernSettingsScreen>,
    project_path: Option<PathBuf>,
}

impl HubSettingsWindow {
    pub fn new(project_path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Populate the settings schema registry before rendering.
        engine_state::register_default_settings();
        let screen = cx.new(|cx| ui_settings::ModernSettingsScreen::new(project_path.clone(), window, cx));
        Self {
            screen,
            project_path,
        }
    }
}

impl Render for HubSettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self
            .project_path
            .as_ref()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .map(|name| format!("Project Settings · {}", name))
            .unwrap_or_else(|| "Settings".to_string());
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(TitleBar::new().child(
                div()
                    .flex()
                    .items_center()
                    .px_2()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(cx.theme().foreground)
                    .child(title),
            ))
            .child(self.screen.clone())
    }
}

/// Hosts the upstream git manager in a hub-owned OS window for a project path.
pub struct HubGitManagerWindow {
    manager: Entity<ui_git_manager::GitManager>,
}

impl HubGitManagerWindow {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let manager = cx.new(|cx| ui_git_manager::GitManager::new(path, window, cx));
        Self { manager }
    }
}

impl Render for HubGitManagerWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.manager.clone())
    }
}

fn secondary_window_options(width: f32, height: f32) -> WindowOptions {
    window_manager::default_window_options(width, height)
}

/// Open a settings window. With `Some(path)` it is scoped to that project's
/// `.pulsar` TOML settings; with `None` it edits global editor settings.
pub fn open_settings_window(project_path: Option<PathBuf>, cx: &mut App) {
    let options = secondary_window_options(1200.0, 700.0);
    cx.open_window(options, |window, cx| {
        let entry = cx.new(|cx| HubSettingsWindow::new(project_path, window, cx));
        cx.new(|cx| ui::Root::new(entry.into(), window, cx))
    })
    .expect("Failed to open settings window");
}

/// Open a git manager window rooted at the given project path.
pub fn open_git_manager_window(path: PathBuf, cx: &mut App) {
    let options = secondary_window_options(1280.0, 800.0);
    cx.open_window(options, |window, cx| {
        let entry = cx.new(|cx| HubGitManagerWindow::new(path, window, cx));
        cx.new(|cx| ui::Root::new(entry.into(), window, cx))
    })
    .expect("Failed to open git manager window");
}
