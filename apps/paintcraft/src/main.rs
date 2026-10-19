//! `paintcraft`: the desktop application. The headless tool is `paintcraft-cli`.

use paintcraft::App;

fn main() -> eframe::Result {
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let mut app = App::new(path)
        .with_recovery(paintcraft_formats::recovery::default_dir())
        .with_settings(paintcraft_ui::settings::default_dir());
    // Smoke runs: close after N frames, e.g. PAINTCRAFT_EXIT_AFTER_FRAMES=30.
    if let Some(n) = std::env::var("PAINTCRAFT_EXIT_AFTER_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        app = app.exit_after(n);
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("PaintCraft")
            .with_inner_size([960.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PaintCraft",
        options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
}
