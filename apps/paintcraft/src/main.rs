//! `paintcraft`: the desktop application. The headless tool is `paintcraft-cli`.

use paintcraft::App;

fn main() -> eframe::Result {
    let _ = paintcraft_platform::spill::remove_abandoned(
        &std::env::temp_dir().join("paintcraft-history"),
    );
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let mut app = App::new(path).with_settings(paintcraft::settings::default_dir());
    // Smoke runs: close after N frames, e.g. PAINTCRAFT_EXIT_AFTER_FRAMES=30.
    if let Some(n) = std::env::var("PAINTCRAFT_EXIT_AFTER_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        app = app.exit_after(n);
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        // eframe also uses the launch title for the macOS application menu.
        // Keep its default (the app name); App::ui publishes document window titles.
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 900.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../../../assets/paintcraft-icon.iconset/icon_256x256@2x.png"
                ))
                .expect("bundled PaintCraft icon is a valid PNG"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "PaintCraft",
        options,
        Box::new(move |cc| {
            Ok(Box::new(
                app.with_native_services(cc)
                    .with_recovery(paintcraft_formats::recovery::default_dir()),
            ))
        }),
    )
}
