use crate::core::actions::*;
use crate::screen::EntryScreen;
use crate::{FabSearchRequested, GitManagerRequested, ProjectSelected, SettingsRequested};
use gpui::*;
use ui::Root;

pub struct EntryWindow {
    screen: Entity<EntryScreen>,
    /// Kept focused so the "Hub" key context is always in the dispatch chain,
    /// even when no inner widget holds focus.
    focus_handle: FocusHandle,
}

impl EntryWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let entry_screen = cx.new(|cx| EntryScreen::new(window, cx));
        let s = entry_screen.clone();
        cx.subscribe_in(
            &s,
            window,
            |this: &mut Self, _screen, event: &ProjectSelected, _window, cx| {
                cx.emit(event.clone());
            },
        )
        .detach();
        cx.subscribe_in(
            &s,
            window,
            |this: &mut Self, _screen, event: &GitManagerRequested, _window, cx| {
                crate::windows::open_git_manager_window(event.path.clone(), cx);
                cx.emit(event.clone());
            },
        )
        .detach();
        cx.subscribe_in(
            &s,
            window,
            |this: &mut Self, _screen, _event: &SettingsRequested, _window, cx| {
                crate::windows::open_settings_window(None, cx);
                cx.emit(SettingsRequested);
            },
        )
        .detach();
        cx.subscribe_in(
            &s,
            window,
            |this: &mut Self, _screen, _event: &FabSearchRequested, _window, cx| {
                cx.emit(FabSearchRequested);
            },
        )
        .detach();
        Self {
            screen: entry_screen,
            focus_handle,
        }
    }

    pub fn entry_screen(&self) -> &Entity<EntryScreen> {
        &self.screen
    }
}

impl EventEmitter<ProjectSelected> for EntryWindow {}
impl EventEmitter<GitManagerRequested> for EntryWindow {}
impl EventEmitter<SettingsRequested> for EntryWindow {}
impl EventEmitter<FabSearchRequested> for EntryWindow {}

impl Render for EntryWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Hub")
            .id("hub-root")
            .track_focus(&self.focus_handle)
            .size_full()
            .on_action(cx.listener(|this, _: &NewProjectModal, _, cx| {
                this.screen.update(cx, |screen, cx| screen.open_new_project_modal(cx));
            }))
            .on_action(cx.listener(|this, _: &OpenFolderDialog, _, cx| {
                this.screen.update(cx, |screen, cx| screen.open_folder_dialog(cx));
            }))
            .on_action(cx.listener(|this, _: &CloneGitModal, _, cx| {
                this.screen.update(cx, |screen, cx| screen.open_clone_git_modal(cx));
            }))
            .on_action(cx.listener(|this, _: &CheckGitUpdates, _, cx| {
                this.screen.update(cx, |screen, cx| screen.start_git_fetch_all(cx));
            }))
            .on_action(|_: &OpenAppSettings, _, cx| {
                crate::windows::open_settings_window(None, cx);
            })
            .child(self.screen.clone())
            .children(Root::render_modal_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
