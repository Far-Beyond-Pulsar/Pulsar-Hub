//! Pulsar Engine Installer
//!
//! Downloads and installs Pulsar engine from GitHub releases.

use gpui::{prelude::*, *};
use pulsar_installer::ui::InstallerView;
use gpui_component::Root;
use gpui::Focusable;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Pulsar Installer");

    let app = Application::new();

    app.run(move |cx| {
        gpui_component::init(cx);
        // Force dark theme by default
        {
            use gpui_component::theme::{Theme, ThemeMode};
            Theme::change(ThemeMode::Dark, None, cx);
        }
        cx.activate(true);

        let window_size = size(px(960.0), px(640.0));
        let window_bounds = Bounds::centered(None, window_size, cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(window_bounds)),
            titlebar: Some(TitlebarOptions {
                title: None,
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.0), px(10.0))),
            }),
            window_min_size: Some(Size {
                width: px(720.0),
                height: px(500.0),
            }),
            kind: WindowKind::Normal,
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let installer_view = InstallerView::view(window, cx);

            let focus_handle = installer_view.focus_handle(cx);
            window.defer(cx, move |window, cx| {
                focus_handle.focus(window);
            });

            cx.new(|cx| Root::new(installer_view.into(), window, cx))
        })
        .expect("Failed to open installer window");
    });
}
