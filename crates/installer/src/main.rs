use gpui::{prelude::*, *};
use pulsar_hub::EntryWindow;
use ui::Assets;
use ui::Root;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Pulsar Hub");

    let app = Application::new().with_assets(Assets);

    app.run(move |cx| {
        ui::init(cx);
        cx.activate(true);

        let window_size = size(px(1200.0), px(800.0));
        let window_bounds = Bounds::centered(None, window_size, cx);

        let window_title = gpui::SharedString::from("Pulsar Engine");

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(window_bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(window_title),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.0), px(10.0))),
            }),
            window_min_size: Some(Size {
                width: px(960.0),
                height: px(640.0),
            }),
            kind: WindowKind::Normal,
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let entry_window = cx.new(|cx| EntryWindow::new(window, cx));
            cx.new(|cx| Root::new(entry_window.into(), window, cx))
        })
        .expect("Failed to open hub window");
    });
}
