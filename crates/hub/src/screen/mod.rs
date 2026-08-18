pub mod views;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use gpui::*;
use parking_lot::Mutex;
use ui::ContextModal;

use crate::core::events::*;
use crate::core::state::*;
use crate::core::types::*;
use crate::screen::views::project_settings::ProjectSettingsTab;
use crate::service::auth_service::AuthService;
use crate::service::cloud_service::CloudService;
use crate::service::dependency_service::DependencyService;
use crate::service::git_service::GitService;
use crate::service::plugin_service::PluginService;
use crate::service::project_service::ProjectService;
use crate::service::thumbnail_service::ThumbnailService;
use ui_common::ProfileDropdownEvent;

fn git_fetch_status(
    result: Result<ui_git_manager::AutoFetchOutcome, git2::Error>,
) -> Option<GitFetchStatus> {
    match result {
        Ok(ui_git_manager::AutoFetchOutcome::Busy) => None,
        Ok(ui_git_manager::AutoFetchOutcome::Fetched(snapshot))
            if snapshot.upstream_oid.is_none() =>
        {
            Some(GitFetchStatus::NotStarted)
        }
        Ok(ui_git_manager::AutoFetchOutcome::Fetched(snapshot)) if snapshot.behind == 0 => {
            Some(GitFetchStatus::UpToDate)
        }
        Ok(ui_git_manager::AutoFetchOutcome::Fetched(snapshot)) => {
            Some(GitFetchStatus::UpdatesAvailable(snapshot.behind))
        }
        Err(error) => Some(GitFetchStatus::Error(error.to_string())),
    }
}

fn current_git_status_generation(
    generations: &HashMap<PathBuf, u64>,
    repository_key: &Path,
) -> u64 {
    generations.get(repository_key).copied().unwrap_or_default()
}

fn next_git_status_generation(
    generations: &mut HashMap<PathBuf, u64>,
    repository_key: &Path,
) -> u64 {
    let generation = current_git_status_generation(generations, repository_key).wrapping_add(1);
    generations.insert(repository_key.to_path_buf(), generation);
    generation
}

fn apply_git_status_if_current(
    statuses: &mut HashMap<String, GitFetchStatus>,
    generations: &HashMap<PathBuf, u64>,
    status_key: String,
    repository_key: &Path,
    expected_generation: u64,
    status: Option<GitFetchStatus>,
) -> bool {
    let Some(status) = status else {
        return false;
    };
    if current_git_status_generation(generations, repository_key) != expected_generation {
        return false;
    }

    statuses.insert(status_key, status);
    true
}

fn git_fetch_paths(
    projects: &[crate::service::project_service::RecentProject],
) -> Vec<(String, PathBuf)> {
    projects
        .iter()
        .map(|project| (project.path.clone(), PathBuf::from(&project.path)))
        .collect()
}

pub struct EntryScreen {
    pub state: AppState,
    pub inputs: InputEntities,
    /// Component list view for the engine release list (infinite scroll).
    pub(crate) release_list: Option<
        gpui::Entity<
            ui::list::List<crate::screen::views::release_list::ReleaseListDelegate>,
        >,
    >,
    /// Channel-selection dropdown content (multi-select checkboxes).
    pub(crate) channel_menu:
        Option<gpui::Entity<crate::screen::views::channel_menu::ChannelMenuView>>,
}

impl EntryScreen {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut state = AppState::new(window, cx);
        let inputs = InputEntities::new(window, cx);
        let self_entity = cx.entity().clone();
        let release_delegate_weak = self_entity.downgrade();
        let release_list = cx.new(|cx| {
            let delegate =
                crate::screen::views::release_list::ReleaseListDelegate::new(
                    release_delegate_weak,
                );
            ui::list::List::new(delegate, window, cx).no_query()
        });
        let channel_menu = Some(cx.new(|cx| {
            crate::screen::views::channel_menu::ChannelMenuView::new(
                self_entity.downgrade(),
                cx,
            )
        }));

        cx.subscribe_in(
            &state.auth.profile_dropdown,
            window,
            |_this, _, event: &ui_common::ProfileDropdownEvent, window, cx| {
                if matches!(event, ui_common::ProfileDropdownEvent::GitSettingsRequested) {
                    ui_git_manager::open_git_settings_modal(window, cx);
                }
            },
        )
        .detach();

        let status = DependencyService::check();
        state.dependency_status = Some(status);

        for proj in &state.recent_projects.projects {
            state.project_thumbnail_queue.push_back(proj.path.clone());
        }
        for tmpl in &state.templates {
            state.template_thumbnail_queue.push_back(tmpl.clone());
        }

        let oobe_marker = directories::ProjectDirs::from("com", "Pulsar", "Pulsar_Engine")
            .map(|d| d.data_dir().join("oobe_complete"));
        let is_fresh = oobe_marker.as_ref().map(|p| !p.exists()).unwrap_or(true);
        let force_oobe = crate::FORCE_OOBE.swap(false, Ordering::Relaxed);
        if is_fresh || force_oobe {
            state.ui.show_onboarding = true;
            if let Some(ref path) = oobe_marker {
                let _ = std::fs::create_dir_all(path.parent().unwrap());
                let _ = std::fs::write(path, "1");
            }
        }

        inputs.subscribe_all(self_entity.downgrade(), cx);

        let profile_dropdown = state.auth.profile_dropdown.clone();
        cx.subscribe(
            &profile_dropdown,
            |this, _, event: &ProfileDropdownEvent, cx| {
                if matches!(event, ProfileDropdownEvent::SignInRequested) {
                    this.begin_github_sign_in(cx);
                }
                cx.notify();
            },
        )
        .detach();

        let mut this = Self {
            state,
            inputs,
            release_list: Some(release_list),
            channel_menu,
        };
        this.state.git_auto_fetch_task = Some(Self::start_git_auto_fetch_task(cx));
        this.load_thumbnails(cx);
        if this.state.ui.show_onboarding {
            this.refresh_plugin_registry(cx);
        }
        this
    }

    pub fn inputs(&self) -> &InputEntities {
        &self.inputs
    }

    pub(crate) fn check_dependencies_async(&mut self, cx: &mut Context<Self>) {
        // TODO: async spawn
        self.state.dependency_status = Some(DependencyService::check());
        cx.notify();
    }

    fn start_git_auto_fetch_task(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            let settings_watcher = ui_git_manager::AutoFetchSettingsWatcher::new();

            loop {
                let scheduled_settings = ui_git_manager::read_auto_fetch_settings();
                if settings_watcher
                    .wait(
                        cx.background_executor()
                            .timer(std::time::Duration::from_secs(
                                scheduled_settings.interval_minutes * 60,
                            )),
                    )
                    .await
                    == ui_git_manager::AutoFetchWaitOutcome::SettingsChanged
                {
                    continue;
                }

                if this.upgrade().is_none() {
                    break;
                }

                let current_settings = ui_git_manager::read_auto_fetch_settings();
                if current_settings != scheduled_settings || !current_settings.enabled {
                    continue;
                }

                if this
                    .update(cx, |screen, cx| screen.start_git_fetch_all(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    pub(crate) fn start_git_fetch_all(&mut self, cx: &mut Context<Self>) {
        if self.state.is_fetching_updates {
            return;
        }

        let paths = git_fetch_paths(&self.state.recent_projects.projects);

        if paths.is_empty() {
            return;
        }

        self.state.is_fetching_updates = true;
        cx.notify();

        let task = cx.spawn(async move |this, cx| {
            for (status_key, path) in paths {
                if this.upgrade().is_none() {
                    return;
                }

                let discovery_path = path.clone();
                let repository_key = match cx
                    .background_executor()
                    .spawn(
                        async move { ui_git_manager::canonical_repository_path(&discovery_path) },
                    )
                    .await
                {
                    Ok(repository_key) => repository_key,
                    Err(error) if error.code() == git2::ErrorCode::NotFound => {
                        if this
                            .update(cx, |screen, cx| {
                                if screen
                                    .state
                                    .git_fetch_statuses
                                    .lock()
                                    .remove(&status_key)
                                    .is_some()
                                {
                                    cx.notify();
                                }
                            })
                            .is_err()
                        {
                            return;
                        }
                        continue;
                    }
                    Err(error) => {
                        if this
                            .update(cx, |screen, cx| {
                                screen
                                    .state
                                    .git_fetch_statuses
                                    .lock()
                                    .insert(status_key, GitFetchStatus::Error(error.to_string()));
                                cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        }
                        continue;
                    }
                };
                let expected_generation = match this.update(cx, |screen, _| {
                    current_git_status_generation(
                        &screen.state.git_repository_generations,
                        &repository_key,
                    )
                }) {
                    Ok(generation) => generation,
                    Err(_) => return,
                };
                let result = cx
                    .background_executor()
                    .spawn(async move { ui_git_manager::fetch_tracking_snapshot(&path) })
                    .await;
                let status = git_fetch_status(result);

                if this
                    .update(cx, |screen, cx| {
                        let updated = apply_git_status_if_current(
                            &mut screen.state.git_fetch_statuses.lock(),
                            &screen.state.git_repository_generations,
                            status_key,
                            &repository_key,
                            expected_generation,
                            status,
                        );
                        if updated {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    return;
                }
            }

            let _ = this.update(cx, |screen, cx| {
                screen.state.is_fetching_updates = false;
                cx.notify();
            });
        });
        self.state.git_fetch_task = Some(task);
    }

    pub(crate) fn pull_project_updates(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let status_key = path.to_string_lossy().to_string();
        let repository_key = match ui_git_manager::canonical_repository_path(&path) {
            Ok(repository_key) => repository_key,
            Err(error) if error.code() == git2::ErrorCode::NotFound => {
                self.state.git_fetch_statuses.lock().remove(&status_key);
                cx.notify();
                return;
            }
            Err(error) => {
                self.state
                    .git_fetch_statuses
                    .lock()
                    .insert(status_key, GitFetchStatus::Error(error.to_string()));
                cx.notify();
                return;
            }
        };
        let generation =
            next_git_status_generation(&mut self.state.git_repository_generations, &repository_key);
        self.state
            .git_fetch_statuses
            .lock()
            .insert(status_key.clone(), GitFetchStatus::Fetching);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let status = cx
                .background_executor()
                .spawn(async move {
                    match ui_git_manager::pull_from_remote(&path, None) {
                        Ok(()) => GitFetchStatus::UpToDate,
                        Err(error) => GitFetchStatus::Error(error.to_string()),
                    }
                })
                .await;

            let _ = this.update(cx, |screen, cx| {
                let updated = apply_git_status_if_current(
                    &mut screen.state.git_fetch_statuses.lock(),
                    &screen.state.git_repository_generations,
                    status_key,
                    &repository_key,
                    generation,
                    Some(status),
                );
                if updated {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn open_folder_dialog(&self, cx: &mut Context<Self>) {
        let recent_projects_path = self.state.recent_projects_path.clone();
        cx.spawn(async move |entity, cx| {
            if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                let path = folder.path().to_path_buf();
                if !ProjectService::validate_project(&path) {
                    return;
                }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let path_str = path.to_string_lossy().to_string();
                let is_git = ProjectService::is_git_repo(&path);
                let project = crate::service::project_service::RecentProject {
                    name,
                    path: path_str,
                    last_opened: Some(chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()),
                    is_git,
                };
                let _ = cx.update(|cx| {
                    let _ = entity.update(cx, |this, cx| {
                        this.state.recent_projects.add_or_update(project);
                        this.state.recent_projects.save(&recent_projects_path);
                        cx.emit(ProjectSelected { path });
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    pub(crate) fn clone_git_repo(&mut self, url: Option<String>, cx: &mut Context<Self>) {
        let repo_url = url.unwrap_or_else(|| self.state.input.git_repo_url_text.clone());
        if repo_url.is_empty() {
            return;
        }
        let recent_projects_path = self.state.recent_projects_path.clone();
        self.state.clone_error = None;
        cx.spawn(async move |entity, cx| {
            if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                let parent = folder.path().to_path_buf();
                let target = parent.join(
                    repo_url
                        .trim_end_matches(".git")
                        .split('/')
                        .last()
                        .unwrap_or("repo"),
                );
                if target.exists() {
                    let err = format!("Directory already exists: {}", target.display());
                    let _ = cx.update(|cx| {
                        let _ = entity.update(cx, |this, cx| {
                            this.state.clone_error = Some(err);
                            cx.notify();
                        });
                    });
                    return;
                }
                let progress = Arc::new(Mutex::new(CloneProgress {
                    current: 0,
                    total: 0,
                    message: "Starting clone...".to_string(),
                    completed: false,
                    error: None,
                    cancelled: false,
                }));
                let p = progress.clone();
                let url = repo_url.clone();
                let t = target.clone();
                let _ = cx
                    .background_executor()
                    .spawn(async move { GitService::clone_repository(url, t, p) })
                    .await;
                let has_error = progress.lock().error.is_some();
                if has_error {
                    let err = progress.lock().error.clone().unwrap_or_default();
                    let _ = cx.update(|cx| {
                        let _ = entity.update(cx, |this, cx| {
                            this.state.clone_progress = None;
                            this.state.clone_error = Some(err.clone());
                            this.state.download_manager_view.update(cx, |view, cx| {
                                for item in &mut view.items {
                                    if matches!(
                                        item.kind,
                                        DownloadKind::TemplateClone { .. }
                                    ) && matches!(
                                        item.status,
                                        DownloadStatus::Downloading { .. }
                                    ) {
                                        item.status =
                                            DownloadStatus::Failed(err.clone());
                                    }
                                }
                                cx.notify();
                            });
                            cx.notify();
                        });
                    });
                    return;
                }
                let show_upstream = ProjectService::is_git_repo(&target)
                    && !GitService::has_origin_remote(&target);
                let _ = cx.update(|cx| {
                    let _ = entity.update(cx, |this, cx| {
                        this.state.clone_progress = None;
                        this.state.clone_error = None;
                        this.state.input.new_project_path = Some(target.clone());
                        this.state.download_manager_view.update(cx, |view, cx| {
                            for item in &mut view.items {
                                if matches!(
                                    item.kind,
                                    DownloadKind::TemplateClone { .. }
                                ) && matches!(
                                    item.status,
                                    DownloadStatus::Downloading { .. }
                                ) {
                                    item.status = DownloadStatus::Complete;
                                }
                            }
                            cx.notify();
                        });
                        if show_upstream {
                            let n = target
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            this.state.ui.show_git_upstream_prompt = Some((target.clone(), n));
                        } else {
                            let ps = target.to_string_lossy().to_string();
                            let n = target
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            this.state.recent_projects.add_or_update(
                                crate::service::project_service::RecentProject {
                                    name: n,
                                    path: ps,
                                    last_opened: Some(
                                        chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
                                    ),
                                    is_git: true,
                                },
                            );
                            this.state.recent_projects.save(&recent_projects_path);
                            cx.emit(ProjectSelected { path: target });
                        }
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    pub(crate) fn clone_template(&mut self, template: Template, cx: &mut Context<Self>) {
        let dl_id = format!("template-{}", template.name);
        self.state.download_manager_view.update(cx, |view, cx| {
            view.add_item(DownloadItem {
                id: dl_id,
                kind: DownloadKind::TemplateClone {
                    name: template.name.clone(),
                },
                status: DownloadStatus::Downloading {
                    bytes_downloaded: 0,
                    total_bytes: 0,
                    speed_bps: 0,
                },
                started_at: std::time::Instant::now(),
            });
            cx.notify();
        });
        cx.notify();
        self.clone_git_repo(Some(template.repo_url), cx);
    }

    pub(crate) fn setup_git_upstream(&mut self, cx: &mut Context<Self>) {
        let (path, _) = match &self.state.ui.show_git_upstream_prompt.take() {
            Some(pair) => pair.clone(),
            None => return,
        };
        let url = self.state.input.git_upstream_url_text.clone();
        let recent_projects_path = self.state.recent_projects_path.clone();
        cx.spawn(async move |entity, cx| {
            if !url.is_empty() {
                let p = path.clone();
                let u = url.clone();
                let _ = cx
                    .background_executor()
                    .spawn(async move { GitService::add_user_upstream(&p, &u) })
                    .await;
            }
            let _ = cx.update(|cx| {
                let _ = entity.update(cx, |this, cx| {
                    let ps = path.to_string_lossy().to_string();
                    let n = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    this.state.recent_projects.add_or_update(
                        crate::service::project_service::RecentProject {
                            name: n,
                            path: ps,
                            last_opened: Some(
                                chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
                            ),
                            is_git: true,
                        },
                    );
                    this.state.recent_projects.save(&recent_projects_path);
                    cx.emit(ProjectSelected { path });
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// Launch a project with the engine declared in its `Pulsar.toml`.
    ///
    /// If the declared engine is installed, it is launched with the project
    /// path as a CLI argument (instant open). If it's required but missing, an
    /// auto-install prompt is shown instead. With no declared requirement it
    /// falls back to the newest installed engine, or emits `ProjectSelected`
    /// for the embedder to handle when none is installed.
    pub(crate) fn launch_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let is_git = ProjectService::is_git_repo(&path);
        let project = crate::service::project_service::RecentProject {
            name,
            path: path.to_string_lossy().to_string(),
            last_opened: Some(chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()),
            is_git,
        };
        self.state.recent_projects.add_or_update(project);
        self.state
            .recent_projects
            .save(&self.state.recent_projects_path);

        let required = self.required_engine_for_project(&path);
        match required {
            Some(req) if req.eq_ignore_ascii_case("src") => {
                // Projects pinned to `src` use a local engine source checkout.
                if let Some(src) = self.state.src_engine_path.clone() {
                    self.launch_src_project(src, &path, cx);
                } else {
                    tracing::warn!(
                        "Project '{}' targets the 'src' engine but no source checkout is configured",
                        path.display()
                    );
                }
                return;
            }
            Some(req) if self.engine_requirement_satisfied(&req) => {
                if let Some(dir) = self.installed_engine_dir_satisfying(&req) {
                    self.launch_project_with_engine(dir, &path);
                    cx.emit(ProjectSelected { path });
                    return;
                }
            }
            Some(req) => {
                // Declared but not installed → ask to auto-install.
                self.request_engine_install(path, req, cx);
                return;
            }
            None => {}
        }

        // No usable engine requirement → open with the newest installed engine if any.
        if let Some(dir) = self.newest_installed_engine_dir() {
            self.launch_project_with_engine(dir, &path);
        }
        cx.emit(ProjectSelected { path });
    }

    /// Compile the engine from the local `src` checkout in the background, then
    /// launch the target project in it. Keeps the hub window open (showing the
    /// build overlay) until the build finishes so the task isn't cancelled.
    fn launch_src_project(
        &mut self,
        src: PathBuf,
        project: &Path,
        cx: &mut Context<Self>,
    ) {
        self.start_src_build(src, Some(project.to_path_buf()), cx);
    }

    /// Compile the `src` engine standalone (no project) and launch it.
    pub(crate) fn launch_src_standalone(&mut self, src: PathBuf, cx: &mut Context<Self>) {
        self.start_src_build(src, None, cx);
    }

    /// Kick off a `cargo build --release` of the local `src` checkout, streaming
    /// progress into `ui.build_progress` (re-rendering the overlay via a poll),
    /// then launch the built engine — with `project` (and close the window) when
    /// provided, otherwise standalone.
    fn start_src_build(
        &mut self,
        src: PathBuf,
        project: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        use parking_lot::Mutex as PM;
        let progress = std::sync::Arc::new(PM::new(
            crate::service::installer_service::BuildProgress::default(),
        ));
        self.state.ui.build_progress = Some(progress.clone());
        self.state.ui.building_src = true;
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let progress_for_task = progress.clone();
            let _build = cx.background_executor().spawn(async move {
                crate::service::installer_service::compile_engine_src_with_progress(
                    &src,
                    progress_for_task,
                )
            });

            // Poll so the overlay re-renders as compiler output arrives.
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                let finished = progress.lock().finished;
                let _ = cx.update(|cx| {
                    let _ = entity.update(cx, |_, cx| cx.notify());
                });
                if finished {
                    break;
                }
            }

            let result = _build.await;
            let project = project.clone();
            let _ = cx.update(|cx| {
                entity.update(cx, |this, cx| {
                    this.state.ui.building_src = false;
                    this.state.ui.build_progress = None;
                    match result {
                        Ok(binary) => {
                            let current_dir = binary
                                .parent()
                                .map(|p| p.to_path_buf())
                                .unwrap_or_default();
                            if let Some(project) = &project {
                                let _ =
                                    crate::service::installer_service::launch_engine_binary_for_project(
                                        &binary, &current_dir, project,
                                    );
                                let _ = cx
                                    .emit(crate::core::events::ProjectSelected {
                                        path: project.clone(),
                                    });
                            } else {
                                let _ = crate::service::installer_service::launch_engine_binary(
                                    &binary, &current_dir,
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to build the src engine: {}", e);
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// The installed-version list, including the special local "src" engine.
    pub(crate) fn installed_versions(&self) -> Vec<crate::service::installer_service::InstalledVersion> {
        crate::service::installer_service::installed_versions_with_src(
            self.state.src_engine_path.as_deref(),
        )
    }

    /// Prompt the user for a local engine source checkout and register it as
    /// the special "src" engine version.
    pub(crate) fn prompt_add_src(&mut self, cx: &mut Context<Self>) {
        let config_path = self.state.src_engine_config_path.clone();
        cx.spawn(async move |entity, cx| {
            if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                let path = folder.path().to_path_buf();
                let path_string = path.to_string_lossy().to_string();
                let _ = cx.update(|cx| {
                    entity.update(cx, |this, cx| {
                        this.state.src_engine_path = Some(PathBuf::from(&path_string));
                        let _ = std::fs::write(&config_path, path_string.as_bytes());
                        this.state.versions.installed = this.installed_versions();
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    fn launch_project_with_engine(&self, install_dir: PathBuf, project: &Path) {
        // Spawn synchronously: the caller closes the hub window right after
        // (`ProjectSelected`), so a spawned task would be cancelled before
        // the engine process starts.
        if let Err(e) =
            crate::service::installer_service::launch_engine_for_project(&install_dir, project)
        {
            tracing::error!("Failed to launch engine for project: {}", e);
        }
    }

    /// The newest installed engine dir that satisfies `required`.
    fn installed_engine_dir_satisfying(&self, required: &str) -> Option<PathBuf> {
        use crate::service::installer_service as svc;
        self.state
            .versions
            .installed
            .iter()
            .filter(|v| svc::installed_satisfies(&v.metadata.version, required))
            .max_by(|a, b| {
                svc::parse_version(&a.metadata.version).cmp(&svc::parse_version(&b.metadata.version))
            })
            .map(|v| v.metadata.install_path.clone())
    }

    /// The newest installed engine dir, if any.
    fn newest_installed_engine_dir(&self) -> Option<PathBuf> {
        self.state
            .versions
            .installed
            .first()
            .map(|v| v.metadata.install_path.clone())
    }

    pub(crate) fn remove_recent_project(&mut self, path: &str, cx: &mut Context<Self>) {
        self.state.recent_projects.remove(path);
        self.state
            .recent_projects
            .save(&self.state.recent_projects_path);
        cx.notify();
    }

    pub(crate) fn open_git_manager(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        cx.emit(GitManagerRequested { path });
    }

    pub(crate) fn open_project_settings(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let (editor, git_tool) = ProjectService::load_tool_preferences(&path);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut ps =
            crate::screen::views::project_settings::ProjectSettings::new(path.clone(), name);
        ps.preferred_editor = editor;
        ps.preferred_git_tool = git_tool;
        self.state.ui.project_settings = Some(ps);
        cx.notify();
    }

    pub(crate) fn close_project_settings(&mut self, cx: &mut Context<Self>) {
        self.state.ui.project_settings = None;
        cx.notify();
    }

    pub fn calculate_columns(&self, available_width: gpui::Pixels) -> usize {
        let card_width = 320.0;
        let gap = 24.0;
        let f_width: f32 = f32::from(available_width);
        let cols = ((f_width + gap) / (card_width + gap)).floor() as usize;
        cols.max(1)
    }

    pub(crate) fn change_project_settings_tab(
        &mut self,
        _tab: ProjectSettingsTab,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
    }

    pub(crate) fn refresh_project_settings(&mut self, cx: &mut Context<Self>) {
        if let Some(ref settings) = self.state.ui.project_settings.clone() {
            let (editor, git_tool) = ProjectService::load_tool_preferences(&settings.project_path);
            let name = settings
                .project_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut ps = crate::screen::views::project_settings::ProjectSettings::new(
                settings.project_path.clone(),
                name,
            );
            ps.preferred_editor = editor;
            ps.preferred_git_tool = git_tool;
            self.state.ui.project_settings = Some(ps);
        }
        cx.notify();
    }

    pub(crate) fn browse_project_location(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |entity, cx| {
            if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                let _ = cx.update(|cx| {
                    let _ = entity.update(cx, |this, cx| {
                        this.state.input.new_project_path = Some(folder.path().to_path_buf());
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    pub(crate) fn create_new_project(&self, cx: &mut Context<Self>) {
        let name = self.state.input.new_project_name_text.clone();
        if name.is_empty() {
            return;
        }
        let base_path = self
            .state
            .input
            .new_project_path
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let project_path = base_path.join(&name);
        let recent_projects_path = self.state.recent_projects_path.clone();
        let n = name.clone();
        let pp = project_path.clone();
        cx.spawn(async move |entity, cx| {
            let _ = cx
                .background_executor()
                .spawn(async move {
                    let _ = std::fs::create_dir_all(&pp);
                    let _ = ProjectService::create_project_dirs(&pp);
                    let _ = ProjectService::write_pulsar_toml(&pp, &n);
                    let _ = ProjectService::init_repository(&pp);
                })
                .await;
            let pstr = project_path.to_string_lossy().to_string();
            let project = crate::service::project_service::RecentProject {
                name,
                path: pstr,
                last_opened: Some(chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()),
                is_git: true,
            };
            let _ = cx.update(|cx| {
                let _ = entity.update(cx, |this, cx| {
                    this.state.recent_projects.add_or_update(project);
                    this.state.recent_projects.save(&recent_projects_path);
                    cx.emit(ProjectSelected { path: project_path });
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub(crate) fn begin_github_sign_in(&mut self, cx: &mut Context<Self>) {
        let Some(client_id) = pulsar_auth::github_client_id_from_env() else {
            self.state.auth.message =
                Some("Set PULSAR_GITHUB_CLIENT_ID to enable GitHub sign-in.".to_string());
            cx.notify();
            return;
        };
        self.state.auth.loading = true;
        self.state.auth.message = Some("Starting GitHub sign-in\u{2026}".to_string());
        self.state.auth.device_code = None;
        self.state.auth.device_verification_url = None;
        cx.notify();
        cx.spawn(async move |entity, cx| {
            let c_id = client_id.clone();
            let flow = cx
                .background_executor()
                .spawn(async move { pulsar_auth::start_device_flow(&c_id) })
                .await;
            let Ok(flow) = flow else {
                let _ = cx.update(|cx| {
                    entity.update(cx, |this, cx| {
                        this.state.auth.loading = false;
                        this.state.auth.message =
                            Some("Failed to start GitHub device flow.".to_string());
                        cx.notify();
                    })
                });
                return;
            };
            let uri = flow.verification_uri.clone();
            let _ = open::that(&uri);
            let _ = cx.update(|cx| {
                entity.update(cx, |this, cx| {
                    this.state.auth.device_code = Some(flow.user_code.clone());
                    this.state.auth.device_verification_url = Some(flow.verification_uri.clone());
                    this.state.auth.loading = false;
                    cx.notify();
                })
            });
            let c_id2 = client_id.to_string();
            let flow_clone = flow.clone();
            let token = cx
                .background_executor()
                .spawn(async move { pulsar_auth::wait_for_device_flow_token(&c_id2, &flow_clone) })
                .await;
            let Ok(token) = token else {
                let _ = cx.update(|cx| {
                    entity.update(cx, |this, cx| {
                        this.state.auth.loading = false;
                        this.state.auth.device_code = None;
                        this.state.auth.device_verification_url = None;
                        this.state.auth.message =
                            Some("GitHub sign-in timed out or failed.".to_string());
                        cx.notify();
                    })
                });
                return;
            };
            let token_fetch = token.clone();
            let profile = cx
                .background_executor()
                .spawn(async move { pulsar_auth::fetch_profile(&token_fetch) })
                .await;
            let profile = match profile {
                Ok(p) => p,
                Err(e) => {
                    let _ = cx.update(|cx| {
                        entity.update(cx, |this, cx| {
                            this.state.auth.loading = false;
                            this.state.auth.message = Some(format!("Failed to fetch profile: {e}"));
                            cx.notify();
                        })
                    });
                    return;
                }
            };
            let _ = pulsar_auth::store_access_token(&token);
            let _ = pulsar_auth::save_cached_profile(&profile);
            if let Some(ec) = engine_state::EngineContext::global() {
                ec.set_auth_profile(profile.clone());
            }
            let _ = cx.update(|cx| {
                entity.update(cx, |this, cx| {
                    this.state.auth.loading = false;
                    this.state.auth.device_code = None;
                    this.state.auth.device_verification_url = None;
                    this.state.auth.message = None;
                    this.state.auth.profile_dropdown.update(cx, |d, cx| {
                        d.ensure_avatar_loaded(cx);
                        cx.notify();
                    });
                    cx.notify();
                })
            });
        })
        .detach();
    }

    pub(crate) fn handle_auth_device_code(&self, _cx: &mut Context<Self>) {
        if let Some(ref url) = self.state.auth.device_verification_url {
            let _ = open::that(url);
        }
    }

    pub(crate) fn cancel_auth(&mut self, cx: &mut Context<Self>) {
        self.state.auth.loading = false;
        self.state.auth.message = None;
        self.state.auth.device_code = None;
        self.state.auth.device_verification_url = None;
        cx.notify();
    }

    pub(crate) fn sign_out(&mut self, cx: &mut Context<Self>) {
        let _ = pulsar_auth::clear_cached_profile();
        let _ = pulsar_auth::clear_access_token();
        if let Some(ec) = engine_state::EngineContext::global() {
            ec.clear_auth_profile();
        }
        self.state.auth.profile_dropdown.update(cx, |d, cx| {
            d.avatar_image = None;
            d.avatar_url_loaded = None;
            d.is_open = false;
            cx.notify();
        });
        self.state.auth.onboarding_avatar = None;
        self.state.auth.onboarding_avatar_url = None;
        cx.notify();
    }

    pub(crate) fn handle_invite_response(&mut self, _accept: bool, cx: &mut Context<Self>) {
        self.state.pending_invite = None;
        cx.notify();
    }

    pub(crate) fn add_cloud_server(&mut self, cx: &mut Context<Self>) {
        let alias = self.state.input.add_server_alias_text.clone();
        let url_text = self.state.input.add_server_url_text.clone();
        let email = self.state.input.add_server_email_text.clone();
        let password = self.state.input.add_server_password_text.clone();
        self.state.add_server_logging_in = true;
        self.state.add_server_error = None;
        cx.notify();
        let normalized_url = normalize_url(&url_text);
        cx.spawn(async move |entity, cx| {
            let nu = normalized_url.clone();
            let em = email.clone();
            let pw = password.clone();
            let result = cx
                .background_executor()
                .spawn(async move { CloudService::login(&nu, &em, &pw) })
                .await;
            let _ = cx.update(|cx| {
                entity.update(cx, |this, cx| {
                    this.state.add_server_logging_in = false;
                    if let Some((token, username)) = result {
                        this.state.cloud_servers.push(CloudServer {
                            id: uuid::Uuid::new_v4().to_string(),
                            alias,
                            url: normalized_url,
                            auth_token: token,
                            username,
                            status: CloudServerStatus::Unknown,
                            projects: Vec::new(),
                        });
                        this.save_cloud_servers();
                        this.state.add_server_error = None;
                        this.state.input.add_server_alias_text.clear();
                        this.state.input.add_server_url_text.clear();
                        this.state.input.add_server_email_text.clear();
                        this.state.input.add_server_password_text.clear();
                        this.state.ui.show_add_server = false;
                    } else {
                        this.state.add_server_error =
                            Some("Login failed. Check your credentials.".to_string());
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }

    pub(crate) fn test_cloud_server_connection(&self, index: usize, cx: &mut Context<Self>) {
        if index >= self.state.cloud_servers.len() {
            return;
        }
        let server = self.state.cloud_servers[index].clone();
        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(
                    async move { CloudService::fetch_server_info(&server.url, &server.auth_token) },
                )
                .await;
            let _ = cx.update(|cx| {
                entity.update(cx, |this, cx| {
                    if index < this.state.cloud_servers.len() {
                        this.state.cloud_servers[index].status = result
                            .as_ref()
                            .map(|r| r.0.clone())
                            .unwrap_or(CloudServerStatus::Offline);
                        if let Some((_, projects)) = result {
                            this.state.cloud_servers[index].projects = projects;
                        }
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }

    pub(crate) fn open_cloud_projects_view(&mut self, cx: &mut Context<Self>) {
        self.state.ui.view = EntryScreenView::CloudProjects;
        if !crate::util::path_helpers::is_cloud_intro_seen() {
            self.state.ui.show_cloud_intro_modal = true;
            self.state.ui.cloud_intro_page = 0;
        }
        cx.notify();
    }

    pub(crate) fn open_cloud_intro_modal(&mut self, cx: &mut Context<Self>) {
        self.state.ui.show_cloud_intro_modal = true;
        self.state.ui.cloud_intro_page = 0;
        cx.notify();
    }

    pub(crate) fn close_cloud_intro_modal(&mut self, cx: &mut Context<Self>) {
        self.state.ui.show_cloud_intro_modal = false;
        crate::util::path_helpers::mark_cloud_intro_seen();
        cx.notify();
    }

    pub(crate) fn next_cloud_intro_page(&mut self, cx: &mut Context<Self>) {
        if self.state.ui.cloud_intro_page < 2 {
            self.state.ui.cloud_intro_page += 1;
        } else {
            self.close_cloud_intro_modal(cx);
        }
        cx.notify();
    }

    pub(crate) fn prev_cloud_intro_page(&mut self, cx: &mut Context<Self>) {
        if self.state.ui.cloud_intro_page > 0 {
            self.state.ui.cloud_intro_page -= 1;
        }
        cx.notify();
    }

    pub(crate) fn set_cloud_intro_page(&mut self, page: usize, cx: &mut Context<Self>) {
        self.state.ui.cloud_intro_page = page.min(2);
        cx.notify();
    }

    pub(crate) fn select_cloud_server(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.selected_cloud_server = Some(index);
        self.test_cloud_server_connection(index, cx);
        cx.notify();
    }

    pub(crate) fn refresh_cloud_server(&self, index: usize, cx: &mut Context<Self>) {
        self.test_cloud_server_connection(index, cx);
    }

    pub(crate) fn prepare_cloud_project(
        &self,
        server_idx: usize,
        project_idx: usize,
        _cx: &mut Context<Self>,
    ) {
        if server_idx >= self.state.cloud_servers.len() {
            return;
        }
        let server = self.state.cloud_servers[server_idx].clone();
        if project_idx >= server.projects.len() {
            return;
        }
        let project = server.projects[project_idx].clone();
        std::thread::spawn(move || {
            CloudService::prepare_workspace(&server.url, &project.id, &server.auth_token)
        });
    }

    pub(crate) fn open_cloud_project(
        &mut self,
        server_idx: usize,
        project_idx: usize,
        cx: &mut Context<Self>,
    ) {
        if server_idx >= self.state.cloud_servers.len() {
            return;
        }
        let server = self.state.cloud_servers[server_idx].clone();
        if project_idx >= server.projects.len() {
            return;
        }
        let project = server.projects[project_idx].clone();
        let eid = project.environment_id.as_deref().unwrap_or("");
        let path = CloudService::open_workspace(
            &server.url,
            &project.id,
            &server.auth_token,
            &server.username,
            eid,
        );
        cx.emit(ProjectSelected { path });
    }

    pub(crate) fn stop_cloud_project(
        &self,
        server_idx: usize,
        project_idx: usize,
        _cx: &mut Context<Self>,
    ) {
        if server_idx >= self.state.cloud_servers.len() {
            return;
        }
        let server = self.state.cloud_servers[server_idx].clone();
        if project_idx >= server.projects.len() {
            return;
        }
        let project = server.projects[project_idx].clone();
        std::thread::spawn(move || {
            CloudService::stop_workspace(&server.url, &project.id, &server.auth_token)
        });
    }

    pub(crate) fn delete_cloud_project(
        &self,
        server_idx: usize,
        project_idx: usize,
        _cx: &mut Context<Self>,
    ) {
        if server_idx >= self.state.cloud_servers.len() {
            return;
        }
        let server = self.state.cloud_servers[server_idx].clone();
        if project_idx >= server.projects.len() {
            return;
        }
        let project = server.projects[project_idx].clone();
        std::thread::spawn(move || {
            CloudService::delete_workspace(&server.url, &project.id, &server.auth_token)
        });
    }

    pub(crate) fn create_cloud_project(&mut self, cx: &mut Context<Self>) {
        let name = self.state.input.create_project_name_text.clone();
        if name.is_empty() {
            return;
        }
        let server_idx = self.state.selected_cloud_server.unwrap_or(0);
        if server_idx >= self.state.cloud_servers.len() {
            return;
        }
        let server = self.state.cloud_servers[server_idx].clone();
        let desc = self.state.input.create_project_description_text.clone();
        std::thread::spawn(move || {
            CloudService::create_workspace(&server.url, &name, &desc, &server.auth_token)
        });
        self.state.ui.show_create_project = false;
        self.state.input.create_project_name_text.clear();
        self.state.input.create_project_description_text.clear();
        cx.notify();
    }

    pub(crate) fn remove_cloud_server(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.state.cloud_servers.len() {
            self.state.cloud_servers.remove(index);
            self.save_cloud_servers();
            if self.state.selected_cloud_server == Some(index) {
                self.state.selected_cloud_server = None;
            }
            cx.notify();
        }
    }

    pub(crate) fn refresh_plugin_registry(&mut self, cx: &mut Context<Self>) {
        if self.state.registry_refresh_in_progress {
            return;
        }
        self.state.registry_refresh_in_progress = true;
        cx.notify();
        let registries = self.state.plugin_registries.clone();
        let registries_path = self.state.registries_path.clone();
        cx.spawn(async move |entity, cx| {
            let regs = registries.clone();
            let rp = registries_path.clone();
            let _ = cx
                .background_executor()
                .spawn(async move {
                    match PluginService::clone_or_pull_registries(&regs, &rp) {
                        Ok(()) => tracing::debug!("Plugin registries cloned/pulled successfully"),
                        Err(e) => tracing::error!("Failed to clone/pull plugin registries: {e}"),
                    }
                })
                .await;
            let regs2 = registries.clone();
            let rp2 = registries_path.clone();
            let plugins = cx
                .background_executor()
                .spawn(async move {
                    let list = PluginService::load_plugins_from_registries(&regs2, &rp2);
                    tracing::debug!("Loaded {} plugins from registries", list.len());
                    list
                })
                .await;
            let _ = cx.update(|cx| {
                entity.update(cx, |this, cx| {
                    this.state.registry_plugins = plugins;
                    this.state.registry_refresh_in_progress = false;
                    cx.notify();
                })
            });
        })
        .detach();
    }

    pub(crate) fn install_registry_plugin(
        &mut self,
        plugin: RegistryPlugin,
        cx: &mut Context<Self>,
    ) {
        if self.state.plugin_install_phase.is_some() {
            return;
        }
        self.state.plugin_install_phase = Some(PluginInstallPhase::FetchingMetadata);
        cx.notify();
        let plugins_path = self.state.plugins_path.clone();
        let pname = plugin.name.clone();
        let purl = plugin.repo_url.clone();
        cx.spawn(async move |entity, cx| {
            let (owner, repo) = match PluginService::parse_github_owner_repo(&purl) {
                Some(pair) => pair,
                None => {
                    let _ = cx.update(|cx| {
                        entity.update(cx, |this, cx| {
                            this.state.plugin_install_phase =
                                Some(PluginInstallPhase::Error("Invalid repo URL".to_string()));
                            cx.notify();
                        })
                    });
                    return;
                }
            };
            let repo_tag = repo.clone();
            let release: Option<(String, Option<String>)> = cx
                .background_executor()
                .spawn(async move { PluginService::fetch_latest_release(&owner, &repo) })
                .await
                .ok()
                .flatten();
            let Some((tag, binary_url_opt)) = release else {
                let _ = cx.update(|cx| {
                    entity.update(cx, |this, cx| {
                        this.state.plugin_install_phase =
                            Some(PluginInstallPhase::Error("No releases found".to_string()));
                        cx.notify();
                    })
                });
                return;
            };
            let ext = native_plugin_ext();
            if let Some(binary_url) = binary_url_opt {
                let lib_name = format!("{}_{}.{}", repo_tag, tag, ext);
                let pp = plugins_path.clone();
                let bu = binary_url.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move { PluginService::download_binary(&bu, &pp, &lib_name) })
                    .await;
                match result {
                    Ok(lib_path) => {
                        let installed = InstalledPlugin {
                            name: pname,
                            repo_url: purl,
                            version: tag,
                            installed_at: chrono::Local::now()
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string(),
                            install_method: PluginInstallMethod::BinaryDownload,
                            library_path: lib_path,
                        };
                        let _ = cx.update(|cx| {
                            entity.update(cx, |this, cx| {
                                this.state.plugin_install_phase =
                                    Some(PluginInstallPhase::Complete(installed.clone()));
                                this.state.installed_plugins.push(installed);
                                this.save_installed_plugins();
                                cx.notify();
                            })
                        });
                    }
                    _ => {
                        let _ = cx.update(|cx| {
                            entity.update(cx, |this, cx| {
                                this.state.plugin_install_phase =
                                    Some(PluginInstallPhase::Error("Download failed".to_string()));
                                cx.notify();
                            })
                        });
                    }
                }
            } else {
                let tag_for_installed = tag.clone();
                let pp = plugins_path.clone();
                let purl2 = purl.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        PluginService::build_from_source(&purl2, Some(&tag), &pp, &tag)
                    })
                    .await;
                match result {
                    Ok((lib_path, _logs)) => {
                        let installed = InstalledPlugin {
                            name: pname,
                            repo_url: purl,
                            version: tag_for_installed,
                            installed_at: chrono::Local::now()
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string(),
                            install_method: PluginInstallMethod::BuiltFromSource,
                            library_path: lib_path,
                        };
                        let _ = cx.update(|cx| {
                            entity.update(cx, |this, cx| {
                                this.state.plugin_install_phase =
                                    Some(PluginInstallPhase::Complete(installed.clone()));
                                this.state.installed_plugins.push(installed);
                                this.save_installed_plugins();
                                cx.notify();
                            })
                        });
                    }
                    _ => {
                        let _ = cx.update(|cx| {
                            entity.update(cx, |this, cx| {
                                this.state.plugin_install_phase =
                                    Some(PluginInstallPhase::Error("Build failed".to_string()));
                                cx.notify();
                            })
                        });
                    }
                }
            }
        })
        .detach();
    }

    pub(crate) fn uninstall_plugin(&mut self, _index: usize, cx: &mut Context<Self>) {
        cx.notify();
    }

    pub(crate) fn remove_plugin(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.state.installed_plugins.len() {
            let lib_path = std::path::Path::new(&self.state.installed_plugins[index].library_path)
                .to_path_buf();
            let _ = std::fs::remove_file(&lib_path);
            self.state.installed_plugins.remove(index);
            self.save_installed_plugins();
            cx.notify();
        }
    }

    pub(crate) fn start_dependency_setup(&mut self, cx: &mut Context<Self>) {
        self.state.ui.show_dependency_setup = true;
        let progress = Arc::new(std::sync::Mutex::new(InstallProgress {
            logs: Vec::new(),
            progress: 0.0,
            status: InstallStatus::Idle,
        }));
        self.state.install_progress = Some(InstallProgress {
            logs: Vec::new(),
            progress: 0.0,
            status: InstallStatus::Downloading,
        });
        cx.notify();
        let p = progress.clone();
        cx.spawn(async move |entity, cx| {
            let pp = p.clone();
            let result = cx
                .background_executor()
                .spawn(async move { DependencyService::install_rust(pp) })
                .await;
            match result {
                Ok(()) => {
                    let mut prog = p.lock().unwrap();
                    prog.status = InstallStatus::Complete;
                    prog.progress = 1.0;
                    let prog2 = prog.clone();
                    let status = DependencyService::check();
                    let _ = cx.update(|cx| {
                        entity.update(cx, |this, cx| {
                            this.state.install_progress = Some(prog2);
                            this.state.dependency_status = Some(status);
                            cx.notify();
                        })
                    });
                }
                _ => {
                    let prog = p.lock().unwrap();
                    let prog2 = prog.clone();
                    let _ = cx.update(|cx| {
                        entity.update(cx, |this, cx| {
                            this.state.install_progress = Some(prog2);
                            cx.notify();
                        })
                    });
                }
            }
        })
        .detach();
    }

    pub(crate) fn show_onboarding_flow(&mut self, cx: &mut Context<Self>) {
        self.state.ui.show_onboarding = true;
        self.state.ui.onboarding_tab = OnboardingTab::Theme;
        cx.notify();
    }

    pub(crate) fn dismiss_onboarding(&mut self, cx: &mut Context<Self>) {
        self.state.ui.show_onboarding = false;
        cx.notify();
    }

    pub(crate) fn switch_onboarding_tab(&mut self, tab: OnboardingTab, cx: &mut Context<Self>) {
        self.state.ui.onboarding_tab = tab;
        cx.notify();
    }

    pub(crate) fn inject_notification(&mut self, invite: PendingInvite, cx: &mut Context<Self>) {
        self.state.pending_invite = Some(invite);
        cx.notify();
    }

    pub(crate) fn load_thumbnails(&mut self, cx: &mut Context<Self>) {
        if self.state.project_thumbnail_inflight == 0
            && !self.state.project_thumbnail_queue.is_empty()
        {
            let path = self.state.project_thumbnail_queue.pop_front().unwrap();
            let path_store = path.clone();
            self.state.project_thumbnail_inflight += 1;
            cx.notify();
            cx.spawn(async move |entity_proj, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { ThumbnailService::load_project_thumbnail(&path) })
                    .await;
                let _ = cx.update(|cx| {
                    entity_proj.update(cx, |this, cx| {
                        this.state.project_thumbnails.insert(path_store, result);
                        this.state.project_thumbnail_inflight -= 1;
                        this.load_thumbnails(cx);
                        cx.notify();
                    })
                });
            })
            .detach();
        }
        if self.state.template_thumbnail_inflight == 0
            && !self.state.template_thumbnail_queue.is_empty()
        {
            let template = self.state.template_thumbnail_queue.pop_front().unwrap();
            let name = template.name.clone();
            self.state.template_thumbnail_inflight += 1;
            cx.notify();
            cx.spawn(async move |entity_tmpl, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { ThumbnailService::load_template_thumbnail(&template) })
                    .await;
                let _ = cx.update(|cx| {
                    entity_tmpl.update(cx, |this, cx| {
                        this.state.template_thumbnails.insert(name, result);
                        this.state.template_thumbnail_inflight -= 1;
                        this.load_thumbnails(cx);
                        cx.notify();
                    })
                });
            })
            .detach();
        }
    }

    fn save_cloud_servers(&self) {
        if let Ok(json) = serde_json::to_string(&self.state.cloud_servers) {
            let _ = std::fs::write(&self.state.cloud_servers_path, json);
        }
    }

    fn save_installed_plugins(&self) {
        if let Ok(json) = serde_json::to_string(&self.state.installed_plugins) {
            let _ = std::fs::write(self.state.plugins_path.join("plugins.json"), json);
        }
    }

    // ── Version Management ──────────────────────────────────────────────

    /// The repos backing the currently selected channels (deduped).
    fn needed_repos(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for channel in &self.state.versions.selected_channels {
            if !out.contains(&channel.repo()) {
                out.push(channel.repo());
            }
        }
        out
    }

    /// Recompose `available_releases` from all sources, filtered by the
    /// selected channels and ordered newest-first by publish date.
    fn rebuild_available_releases(&mut self) {
        use crate::service::installer_service as svc;
        let channels = self.state.versions.selected_channels.clone();
        let mut out: Vec<svc::GitHubRelease> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for src in &self.state.versions.channel_sources {
            for release in &src.fetched {
                let included = channels
                    .iter()
                    .any(|ch| ch.repo() == src.repo && ch.includes(release));
                if included && seen.insert(release.tag_name.clone()) {
                    out.push(release.clone());
                }
            }
        }
        svc::sort_releases_newest_first(&mut out);
        self.state.versions.available_releases = out;
    }

    /// Recompute `has_more` from the sources that back the selected channels.
    fn recompute_has_more(&mut self) {
        let needed = self.needed_repos();
        self.state.versions.has_more = self
            .state
            .versions
            .channel_sources
            .iter()
            .any(|s| needed.contains(&s.repo) && s.has_more);
    }

    pub(crate) fn refresh_versions(&mut self, cx: &mut Context<Self>) {
        self.state.versions.installed = self.installed_versions();
        self.state.versions.fetching = true;
        self.state.versions.loading_more = false;
        for src in &mut self.state.versions.channel_sources {
            src.page = 0;
            src.has_more = true;
            src.loading = false;
            src.error = None;
            src.fetched.clear();
        }
        self.rebuild_available_releases();
        cx.notify();

        let repos = self.needed_repos();
        use crate::service::installer_service as svc;
        cx.spawn(async move |entity, cx| {
            let results = cx
                .background_executor()
                .spawn(async move {
                    repos
                        .iter()
                        .map(|repo| (*repo, svc::fetch_repo_releases_blocking(repo, 1)))
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = cx.update(|cx| {
                entity.update(cx, |this, cx| {
                    this.state.versions.fetching = false;
                    for (repo, result) in results {
                        let Some(src) = this
                            .state
                            .versions
                            .channel_sources
                            .iter_mut()
                            .find(|s| s.repo == repo)
                        else {
                            continue;
                        };
                        match result {
                            Ok(list) => {
                                let has_more = list.len()
                                    == svc::RELEASES_PER_PAGE as usize;
                                let mut seen: std::collections::HashSet<String> =
                                    src.fetched.iter().map(|r| r.tag_name.clone()).collect();
                                for release in list {
                                    if seen.insert(release.tag_name.clone()) {
                                        src.fetched.push(release);
                                    }
                                }
                                src.page = 1;
                                src.has_more = has_more;
                                src.loading = false;
                            }
                            Err(e) => {
                                src.error = Some(e);
                                src.loading = false;
                            }
                        }
                    }
                    this.rebuild_available_releases();
                    this.recompute_has_more();
                    this.try_install_pending_engine(cx);
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// Fetch the next page of each needed source, then recompose the list.
    pub(crate) fn load_more_releases(&mut self, cx: &mut Context<Self>) {
        if self.state.versions.fetching || self.state.versions.loading_more {
            return;
        }
        let needed = self.needed_repos();
        let targets: Vec<(&'static str, u32)> = self
            .state
            .versions
            .channel_sources
            .iter()
            .filter(|s| needed.contains(&s.repo) && s.has_more)
            .map(|s| (s.repo, s.page + 1))
            .collect();
        if targets.is_empty() {
            self.state.versions.has_more = false;
            return;
        }

        self.state.versions.loading_more = true;
        for src in &mut self.state.versions.channel_sources {
            if needed.contains(&src.repo) {
                src.loading = true;
            }
        }
        cx.notify();

        use crate::service::installer_service as svc;
        cx.spawn(async move |entity, cx| {
            let results = cx
                .background_executor()
                .spawn(async move {
                    targets
                        .iter()
                        .map(|(repo, page)| (*repo, svc::fetch_repo_releases_blocking(repo, *page)))
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = cx.update(|cx| {
                entity.update(cx, |this, cx| {
                    this.state.versions.loading_more = false;
                    for (repo, result) in results {
                        let Some(src) = this
                            .state
                            .versions
                            .channel_sources
                            .iter_mut()
                            .find(|s| s.repo == repo)
                        else {
                            continue;
                        };
                        match result {
                            Ok(list) => {
                                let page = src.page + 1;
                                let has_more = list.len() == svc::RELEASES_PER_PAGE as usize;
                                let mut seen: std::collections::HashSet<String> =
                                    src.fetched.iter().map(|r| r.tag_name.clone()).collect();
                                for release in list {
                                    if seen.insert(release.tag_name.clone()) {
                                        src.fetched.push(release);
                                    }
                                }
                                src.page = page;
                                src.has_more = has_more;
                                src.loading = false;
                            }
                            Err(e) => {
                                src.error = Some(e);
                                src.loading = false;
                            }
                        }
                    }
                    this.rebuild_available_releases();
                    this.recompute_has_more();
                    cx.notify();
                });
            });
        })
        .detach();
    }

    /// Toggle a release channel on/off and recompose the visible list.
    pub(crate) fn toggle_channel(
        &mut self,
        channel: crate::service::installer_service::ReleaseChannel,
        selected: bool,
        cx: &mut Context<Self>,
    ) {
        if selected {
            let v = &mut self.state.versions;
            if !v.selected_channels.contains(&channel) {
                v.selected_channels.push(channel);
            }
        } else {
            self.state.versions.selected_channels.retain(|c| *c != channel);
        }
        self.rebuild_available_releases();
        self.recompute_has_more();

        // If a newly-enabled channel's source repo was never fetched (e.g.
        // Nightly, which is off by default), go fetch its first page now.
        let needed = self.needed_repos();
        let to_fetch: Vec<&'static str> = self
            .state
            .versions
            .channel_sources
            .iter()
            .filter(|s| {
                needed.contains(&s.repo) && s.page == 0 && s.fetched.is_empty() && !s.loading
            })
            .map(|s| s.repo)
            .collect();
        if !to_fetch.is_empty() {
            self.state.versions.fetching = true;
            for src in &mut self.state.versions.channel_sources {
                if to_fetch.contains(&src.repo) {
                    src.loading = true;
                }
            }
            use crate::service::installer_service as svc;
            cx.spawn(async move |entity, cx| {
                let results = cx
                    .background_executor()
                    .spawn(async move {
                        to_fetch
                            .iter()
                            .map(|repo| (*repo, svc::fetch_repo_releases_blocking(repo, 1)))
                            .collect::<Vec<_>>()
                    })
                    .await;
                let _ = cx.update(|cx| {
                    entity.update(cx, |this, cx| {
                        this.state.versions.fetching = false;
                        for (repo, result) in results {
                            let Some(src) = this
                                .state
                                .versions
                                .channel_sources
                                .iter_mut()
                                .find(|s| s.repo == repo)
                            else {
                                continue;
                            };
                            match result {
                                Ok(list) => {
                                    let has_more = list.len()
                                        == svc::RELEASES_PER_PAGE as usize;
                                    let mut seen: std::collections::HashSet<String> =
                                        src.fetched.iter().map(|r| r.tag_name.clone()).collect();
                                    for release in list {
                                        if seen.insert(release.tag_name.clone()) {
                                            src.fetched.push(release);
                                        }
                                    }
                                    src.page = 1;
                                    src.has_more = has_more;
                                    src.loading = false;
                                }
                                Err(e) => {
                                    src.error = Some(e);
                                    src.loading = false;
                                }
                            }
                        }
                        this.rebuild_available_releases();
                        this.recompute_has_more();
                        cx.notify();
                    });
                });
            })
            .detach();
        }
        cx.notify();
    }

    /// The engine version a project requires, if its `Pulsar.toml` declares one.
    pub(crate) fn required_engine_for_project(&self, path: &std::path::Path) -> Option<String> {
        ProjectService::project_engine_version(path)
    }

    /// Whether the currently installed engine(s) satisfy the project requirement.
    pub(crate) fn engine_requirement_satisfied(&self, required: &str) -> bool {
        crate::service::installer_service::any_installed_satisfies(
            &self.state.versions.installed,
            required,
        )
    }

    /// `Some(required)` if `path` needs an engine version we don't have installed.
    pub(crate) fn missing_engine_for_project(&self, path: &std::path::Path) -> Option<String> {
        let required = self.required_engine_for_project(path)?;
        if self.engine_requirement_satisfied(&required) {
            None
        } else {
            Some(required)
        }
    }

    pub(crate) fn request_engine_install(
        &mut self,
        project_path: std::path::PathBuf,
        required: String,
        cx: &mut Context<Self>,
    ) {
        let project_name = project_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| project_path.to_string_lossy().to_string());
        self.state.ui.engine_prompt = Some(crate::core::types::EnginePrompt {
            project_name,
            project_path: project_path.to_string_lossy().to_string(),
            required,
        });
        cx.notify();
    }

    pub(crate) fn close_engine_prompt(&mut self, cx: &mut Context<Self>) {
        self.state.ui.engine_prompt = None;
        cx.notify();
    }

    /// Close the prompt and begin installing an engine that satisfies `required`.
    pub(crate) fn install_engine_from_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(prompt) = self.state.ui.engine_prompt.take() else {
            return;
        };
        let required = prompt.required.clone();
        // If the project pins a nightly, make sure the nightly channel is enabled.
        if required.to_lowercase().starts_with("nightly-")
            && !self
                .state
                .versions
                .selected_channels
                .contains(&crate::service::installer_service::ReleaseChannel::Nightly)
        {
            self.toggle_channel(
                crate::service::installer_service::ReleaseChannel::Nightly,
                true,
                cx,
            );
        }
        self.state.ui.pending_engine_install = Some(required);
        if self.state.versions.available_releases.is_empty() && !self.state.versions.fetching {
            self.refresh_versions(cx);
        }
        self.try_install_pending_engine(cx);
        cx.notify();
    }

    /// If there's a pending engine install and a satisfying release is loaded,
    /// install it.
    fn try_install_pending_engine(&mut self, cx: &mut Context<Self>) {
        let Some(required) = self.state.ui.pending_engine_install.clone() else {
            return;
        };
        if self.engine_requirement_satisfied(&required) {
            self.state.ui.pending_engine_install = None;
            return;
        }
        let Some(tag) = self.pick_engine_tag(&required) else {
            return;
        };
        self.state.ui.pending_engine_install = None;
        self.install_release_by_tag(tag, cx);
    }

    fn pick_engine_tag(&self, required: &str) -> Option<String> {
        use crate::service::installer_service as svc;
        let releases = &self.state.versions.available_releases;
        // Exact tag match first (covers nightly hashes and exact versions).
        if let Some(r) = releases.iter().find(|r| r.tag_name == required) {
            return Some(r.tag_name.clone());
        }
        if required.to_lowercase().starts_with("nightly-") {
            return None;
        }
        let min = svc::required_min_version(required)?;
        // Releases are sorted newest-first, so the first satisfying release wins.
        for r in releases {
            if svc::find_platform_asset(r).is_some()
                && svc::parse_version(&r.tag_name)
                    .map(|v| v >= min)
                    .unwrap_or(false)
            {
                return Some(r.tag_name.clone());
            }
        }
        None
    }

    /// Open the full-screen release-notes modal for the given installed engine version.
    pub(crate) fn open_release_notes(&mut self, version: String, cx: &mut Context<Self>) {
        self.state.ui.release_notes_modal = Some(crate::core::types::ReleaseNotesModal {
            title: format!("Release Notes · {}", version),
            body: "Loading release notes…".to_string(),
        });
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let version = version.clone();
            let body = cx
                .background_executor()
                .spawn(async move {
                    crate::service::installer_service::release_notes_for_version(&version)
                })
                .await;
            let _ = cx.update(|cx| {
                entity.update(cx, |this, cx| {
                    if let Some(modal) = &mut this.state.ui.release_notes_modal {
                        modal.body = body;
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub(crate) fn close_release_notes_modal(&mut self, cx: &mut Context<Self>) {
        self.state.ui.release_notes_modal = None;
        cx.notify();
    }

    /// Start downloading + extracting the given release, driven by the tag name.
    pub(crate) fn install_release_by_tag(&mut self, tag: String, cx: &mut Context<Self>) {
        let Some(release) = self
            .state
            .versions
            .available_releases
            .iter()
            .find(|r| r.tag_name == tag)
            .cloned()
        else {
            return;
        };
        let Some(asset) = crate::service::installer_service::find_platform_asset(&release) else {
            return;
        };
        let url = asset.browser_download_url.clone();
        let dest = crate::service::installer_service::default_install_path()
            .join(tag.trim_start_matches('v'));
        let dl_id = format!("engine-{}", tag);

        let dm_view = self.state.download_manager_view.clone();
        dm_view.update(cx, |view, cx| {
            view.add_item(crate::core::types::DownloadItem {
                id: dl_id.clone(),
                kind: crate::core::types::DownloadKind::EngineVersion {
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
        self.state.versions.install_state =
            crate::service::installer_service::VersionInstallState::Downloading {
                version: tag.clone(),
                progress: 0.0,
            };
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let dl_tag = tag.clone();
            let progress = std::sync::Arc::new(parking_lot::Mutex::new(
                crate::service::installer_service::DownloadProgress::default(),
            ));
            let progress_clone = progress.clone();

            let _download_task = cx.background_executor().spawn(async move {
                crate::service::installer_service::download_and_extract_with_progress(
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
                let (bytes, total, speed, done, _error) = snapshot;
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
                            crate::service::installer_service::VersionInstallState::Error {
                                version: tag.clone(),
                                message: e.clone(),
                            };
                    } else {
                        dm_view.update(cx, |view, cx| {
                            view.complete(&dl_id);
                            cx.notify();
                        });
                        this.state.versions.install_state =
                            crate::service::installer_service::VersionInstallState::Complete {
                                version: tag.clone(),
                            };
                        this.state.versions.installed = this.installed_versions();
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub(crate) fn install_latest_version(&mut self, cx: &mut Context<Self>) {
        if let Some(release) = self.state.versions.available_releases.first().cloned() {
            let tag = release.tag_name.clone();
            if let Some(asset) = crate::service::installer_service::find_platform_asset(&release) {
                let url = asset.browser_download_url.clone();
                let dest = crate::service::installer_service::default_install_path()
                    .join(tag.trim_start_matches('v'));

                self.state.versions.install_state =
                    crate::service::installer_service::VersionInstallState::Downloading {
                        version: tag.clone(),
                        progress: 0.0,
                    };
                cx.notify();

                cx.spawn(async move |entity, cx| {
                    let dl_tag = tag.clone();
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            crate::service::installer_service::download_and_extract_blocking(
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
                                        crate::service::installer_service::VersionInstallState::Complete {
                                            version: tag,
                                        };
                                    this.state.versions.installed = this.installed_versions();
                                }
                                Err(e) => {
                                    this.state.versions.install_state =
                                        crate::service::installer_service::VersionInstallState::Error {
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
    }

    pub(crate) fn remove_version(&mut self, version: &str, cx: &mut Context<Self>) {
        if version.eq_ignore_ascii_case("src") {
            // The "src" entry is a configured path, not a directory to delete.
            self.state.src_engine_path = None;
            let _ = std::fs::remove_file(&self.state.src_engine_config_path);
            self.state.versions.installed = self.installed_versions();
            cx.notify();
            return;
        }
        let installed = &self.state.versions.installed;
        if let Some(ver) = installed.iter().find(|v| v.metadata.version == version) {
            let path = ver.metadata.install_path.clone();
            cx.spawn(async move |entity, cx| {
                cx.background_executor()
                    .spawn(async move {
                        let _ = crate::service::installer_service::remove_version(&path);
                    })
                    .await;
                let _ = cx.update(|cx| {
                    entity.update(cx, |this, cx| {
                        this.state.versions.installed = this.installed_versions();
                        cx.notify();
                    });
                });
            })
            .detach();
        }
    }
}

impl Render for EntryScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.state.auth.device_code.is_none() && self.state.ui.auth_device_modal_shown {
            self.state.ui.auth_device_modal_shown = false;
            window.close_modal(cx);
        }
        if let Some(ref code) = self.state.auth.device_code {
            if !self.state.ui.auth_device_modal_shown {
                self.state.ui.auth_device_modal_shown = true;
                let url = self.state.auth.device_verification_url
                    .as_deref()
                    .unwrap_or("https://github.com/login/device")
                    .to_string();
                ui_auth::modal::open_device_code_modal(code, &url, window, cx);
            }
        }
        crate::screen::layout::render_layout(self, window, cx)
    }
}

mod layout;

impl EventEmitter<ProjectSelected> for EntryScreen {}
impl EventEmitter<GitManagerRequested> for EntryScreen {}
impl EventEmitter<SettingsRequested> for EntryScreen {}
impl EventEmitter<FabSearchRequested> for EntryScreen {}

#[cfg(target_os = "windows")]
fn native_plugin_ext() -> &'static str {
    "dll"
}
#[cfg(target_os = "macos")]
fn native_plugin_ext() -> &'static str {
    "dylib"
}
#[cfg(target_os = "linux")]
fn native_plugin_ext() -> &'static str {
    "so"
}

fn normalize_url(raw: &str) -> String {
    let raw = raw.trim().trim_end_matches('/');
    if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else {
        format!("http://{}", raw)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, path::PathBuf};

    use super::{
        apply_git_status_if_current, git_fetch_paths, git_fetch_status, next_git_status_generation,
        GitFetchStatus,
    };
    use crate::service::project_service::{ProjectService, RecentProject};

    fn tracking_snapshot(behind: usize) -> ui_git_manager::TrackingSnapshot {
        ui_git_manager::TrackingSnapshot {
            branch: "main".to_string(),
            upstream_oid: Some("0123456789abcdef".to_string()),
            behind,
        }
    }

    #[test]
    fn git_fetch_result_maps_to_card_status() {
        let mut untracked = tracking_snapshot(0);
        untracked.upstream_oid = None;
        assert!(matches!(
            git_fetch_status(Ok(ui_git_manager::AutoFetchOutcome::Fetched(untracked))),
            Some(GitFetchStatus::NotStarted)
        ));
        assert!(matches!(
            git_fetch_status(Ok(ui_git_manager::AutoFetchOutcome::Fetched(
                tracking_snapshot(0)
            ))),
            Some(GitFetchStatus::UpToDate)
        ));
        assert!(matches!(
            git_fetch_status(Ok(ui_git_manager::AutoFetchOutcome::Fetched(
                tracking_snapshot(3)
            ))),
            Some(GitFetchStatus::UpdatesAvailable(3))
        ));
        assert!(git_fetch_status(Ok(ui_git_manager::AutoFetchOutcome::Busy)).is_none());
        assert!(matches!(
            git_fetch_status(Err(git2::Error::from_str("network unavailable"))),
            Some(GitFetchStatus::Error(message)) if message == "network unavailable"
        ));
    }

    #[test]
    fn stale_git_status_result_does_not_replace_newer_operation() {
        let key = "C:/projects/game";
        let repository_key = PathBuf::from("C:/projects/game/.git");
        let mut statuses = HashMap::from([(key.to_string(), GitFetchStatus::Fetching)]);
        let mut generations = HashMap::new();
        let stale_generation = next_git_status_generation(&mut generations, &repository_key);
        let current_generation = next_git_status_generation(&mut generations, &repository_key);

        assert!(!apply_git_status_if_current(
            &mut statuses,
            &generations,
            key.to_string(),
            &repository_key,
            stale_generation,
            Some(GitFetchStatus::UpdatesAvailable(2)),
        ));
        assert!(matches!(statuses.get(key), Some(GitFetchStatus::Fetching)));

        assert!(apply_git_status_if_current(
            &mut statuses,
            &generations,
            key.to_string(),
            &repository_key,
            current_generation,
            Some(GitFetchStatus::UpToDate),
        ));
        assert!(matches!(statuses.get(key), Some(GitFetchStatus::UpToDate)));
    }

    #[test]
    fn busy_git_fetch_leaves_current_status_unchanged() {
        let key = "C:/projects/game";
        let repository_key = PathBuf::from("C:/projects/game/.git");
        let mut statuses = HashMap::from([(key.to_string(), GitFetchStatus::UpdatesAvailable(1))]);
        let generations = HashMap::new();

        assert!(!apply_git_status_if_current(
            &mut statuses,
            &generations,
            key.to_string(),
            &repository_key,
            0,
            None,
        ));
        assert!(matches!(
            statuses.get(key),
            Some(GitFetchStatus::UpdatesAvailable(1))
        ));
    }

    #[test]
    fn bulk_fetch_discovers_parent_repository_despite_stale_non_git_flag() {
        let temp = tempfile::tempdir().expect("create temp directory");
        git2::Repository::init(temp.path()).expect("initialize parent repository");
        let project_path = temp.path().join("nested-project");
        fs::create_dir(&project_path).expect("create nested project directory");
        let project_path_string = project_path.to_string_lossy().into_owned();
        let projects = [RecentProject {
            name: "nested-project".to_string(),
            path: project_path_string.clone(),
            last_opened: None,
            is_git: false,
        }];

        let paths = git_fetch_paths(&projects);

        assert_eq!(paths, vec![(project_path_string, project_path.clone())]);
        assert!(ProjectService::is_git_repo(&project_path));
        assert_eq!(
            ui_git_manager::canonical_repository_path(&project_path)
                .expect("discover repository from project subdirectory"),
            ui_git_manager::canonical_repository_path(temp.path())
                .expect("resolve parent repository identity"),
        );
    }

    #[test]
    fn repository_aliases_share_generation_but_keep_separate_status_keys() {
        let temp = tempfile::tempdir().expect("create temp directory");
        git2::Repository::init(temp.path()).expect("initialize repository");
        let nested_path = temp.path().join("nested-project");
        fs::create_dir(&nested_path).expect("create nested project directory");
        let root_repository_key = ui_git_manager::canonical_repository_path(temp.path())
            .expect("resolve root repository identity");
        let nested_repository_key = ui_git_manager::canonical_repository_path(&nested_path)
            .expect("resolve nested repository identity");
        let root_status_key = temp.path().to_string_lossy().into_owned();
        let nested_status_key = nested_path.to_string_lossy().into_owned();
        let mut statuses = HashMap::from([
            (root_status_key.clone(), GitFetchStatus::Fetching),
            (nested_status_key.clone(), GitFetchStatus::Fetching),
        ]);
        let mut generations = HashMap::new();

        assert_eq!(root_repository_key, nested_repository_key);
        let stale_generation = next_git_status_generation(&mut generations, &root_repository_key);
        let current_generation =
            next_git_status_generation(&mut generations, &nested_repository_key);

        assert!(!apply_git_status_if_current(
            &mut statuses,
            &generations,
            root_status_key.clone(),
            &root_repository_key,
            stale_generation,
            Some(GitFetchStatus::UpdatesAvailable(2)),
        ));
        assert!(apply_git_status_if_current(
            &mut statuses,
            &generations,
            nested_status_key.clone(),
            &nested_repository_key,
            current_generation,
            Some(GitFetchStatus::UpToDate),
        ));
        assert!(matches!(
            statuses.get(&root_status_key),
            Some(GitFetchStatus::Fetching)
        ));
        assert!(matches!(
            statuses.get(&nested_status_key),
            Some(GitFetchStatus::UpToDate)
        ));
    }
}
