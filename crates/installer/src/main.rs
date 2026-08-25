use gpui::{prelude::*, *};
use pulsar_hub::EntryWindow;
use ui::Assets;
use ui::Root;
use ui::theme::Theme;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let skip_update_check = args.iter().any(|a| a == "--updated");

    if !skip_update_check {
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        let should_relaunch = rt.block_on(async {
            match pulsar_installer::updater::check_and_update().await {
                Ok(relaunch) => relaunch,
                Err(e) => {
                    tracing::warn!("Update check failed: {}", e);
                    false
                }
            }
        });

        if should_relaunch {
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe)
                    .arg("--updated")
                    .spawn();
            }
            std::process::exit(0);
        }
    }

    tracing::info!("Starting Pulsar Hub");

    // Register a real HTTP client so GPUI's image/renderer can fetch remote
    // assets (e.g. screenshots in release notes). The default is a null client.
    let http_client = std::sync::Arc::new(
        reqwest_client::ReqwestClient::user_agent("Pulsar-Installer/1.0")
            .expect("failed to build HTTP client"),
    );

    let app = Application::new()
        .with_http_client(http_client)
        .with_assets(Assets);

    app.run(move |cx| {
        ui::init(cx);
        Theme::change(WindowAppearance::Dark, None, cx);
        cx.activate(true);

        // Hub-wide keyboard shortcuts, scoped to the "Hub" key context set on
        // the EntryWindow root so they don't fire inside other windows.
        use pulsar_hub::{
            CheckGitUpdates, CloneGitModal, NewProjectModal, OpenAppSettings, OpenFolderDialog,
        };
        cx.bind_keys([
            gpui::KeyBinding::new("ctrl-n", NewProjectModal, Some("Hub")),
            gpui::KeyBinding::new("ctrl-o", OpenFolderDialog, Some("Hub")),
            gpui::KeyBinding::new("ctrl-g", CloneGitModal, Some("Hub")),
            gpui::KeyBinding::new("ctrl-shift-r", CheckGitUpdates, Some("Hub")),
            gpui::KeyBinding::new("ctrl-,", OpenAppSettings, Some("Hub")),
        ]);

        let window_size = size(px(1300.0), px(800.0));
        let window_bounds = Bounds::centered(None, window_size, cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(window_bounds)),
            titlebar: None,
            window_min_size: Some(Size {
                width: px(960.0),
                height: px(640.0),
            }),
            window_decorations: Some(WindowDecorations::Client),
            kind: WindowKind::Normal,
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let entry_window = cx.new(|cx| EntryWindow::new(window, cx));
            cx.new(|cx| Root::new(entry_window.into(), window, cx))
        })
        .expect("Failed to open hub window");

        if skip_update_check {
            if let Ok(exe) = std::env::current_exe() {
                let backup = exe.with_extension("bak");
                if backup.exists() {
                    match std::fs::remove_file(&backup) {
                        Ok(()) => tracing::info!(
                            "Removed pre-update backup {}",
                            backup.display()
                        ),
                        Err(e) => tracing::debug!(
                            "Could not remove backup {}: {}",
                            backup.display(),
                            e
                        ),
                    }
                }
            }
        }
    });
}
