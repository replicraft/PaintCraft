//! `paintcraft`: the desktop application. The headless tool is `paintcraft-cli`.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use paintcraft::App;
mod startup;
struct StartupInstallation(Option<paintcraft_update::InstallLock>);
impl Drop for StartupInstallation {
    fn drop(&mut self) {
        // App construction may already own threads before the lease is moved
        // into App. Preserve the original FD through startup unwinding too.
        if let Some(lock) = self.0.take() {
            std::mem::forget(lock);
        }
    }
}

fn main() -> eframe::Result {
    // Reading a locale hint is side-effect-free even for malformed preferences.
    let settings_dir = paintcraft::settings::default_dir();
    let startup_language = paintcraft::settings::Settings::language_hint(&settings_dir);
    // Acquire before any document construction or background/editor service startup.
    let platform = paintcraft_update::Platform::detect();
    let mut installation = StartupInstallation(
        match paintcraft_update::InstallLock::acquire(&platform) {
            Ok(lock) => lock,
            Err(error) => {
                use paintcraft_ui::i18n::translate;
                let language = startup_language;
                let title = translate(language, "PaintCraft is updating");
                let message = match &error {
                    paintcraft_update::Error::InstallationBusy => translate(
                        language,
                        "Another PaintCraft editor is using this installation. Close other editors and try again.",
                    ),
                    _ => paintcraft_ui::i18n::format_message(
                        language,
                        "PaintCraft could not obtain its installation lock: { $detail }",
                        &[("detail", error.to_string())],
                    ),
                };
                paintcraft_platform::dialogs::show_error(
                    &title,
                    &message,
                    &translate(language, "OK"),
                );
                return Err(eframe::Error::AppCreation(Box::new(error)));
            }
        },
    );
    let _ = paintcraft_platform::spill::remove_abandoned(
        &std::env::temp_dir().join("paintcraft-history"),
    );
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let mut app =
        App::new_with_settings(path, settings_dir).with_installation_lock(installation.0.take());
    let language = app.language();
    // Smoke runs: close after N frames, e.g. PAINTCRAFT_EXIT_AFTER_FRAMES=30.
    if let Some(n) = std::env::var("PAINTCRAFT_EXIT_AFTER_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        app = app.exit_after(n);
    }
    let result = run(app);
    if let Err(error) = &result {
        startup::report_error(language, &error.to_string());
    }
    result
}

fn run(app: App) -> eframe::Result {
    #[cfg(target_os = "macos")]
    {
        paintcraft::desktop::run(app).map_err(eframe::Error::AppCreation)
    }
    #[cfg(not(target_os = "macos"))]
    {
        run_framework(app)
    }
}

#[cfg(not(target_os = "macos"))]
fn run_framework(app: App) -> eframe::Result {
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
