//! Explicit native framebuffer review; never runs during ordinary/headless tests.
//! Use isolated PAINTCRAFT_CONFIG_DIR and PAINTCRAFT_RECOVERY_DIR directories.
use eframe::egui;
use paintcraft::App;
use paintcraft_formats::image::{ImageFormat, encode};
use std::{
    error::Error,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

type CaptureResult = Arc<Mutex<Option<io::Result<()>>>>;

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("usage: native_capture OUTPUT.png"))?;
    let result: CaptureResult = Arc::default();
    let captured = Arc::clone(&result);
    let app = App::new(None).with_settings(paintcraft::settings::default_dir());
    eframe::run_native(
        "PaintCraft",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default().with_inner_size([1360.0, 900.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            let app = app
                .with_native_services(cc)
                .with_recovery(paintcraft_formats::recovery::default_dir());
            let ctx = cc.egui_ctx.clone();
            // This also requests painting while occluded. An OS window capture
            // alone cannot tell whether eframe skipped the client or rendering failed.
            cc.egui_ctx.request_screenshot(move |image| {
                let outcome = save(&output, &image);
                if let Err(error) = &outcome {
                    eprintln!("Native capture failed: {error}");
                }
                *captured.lock().expect("capture result lock") = Some(outcome);
                // Leave the presented frame available briefly for an independent
                // native-window capture. Wake/close even if its UI is now occluded.
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(3));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                });
            });
            Ok(Box::new(app))
        }),
    )?;
    result
        .lock()
        .map_err(|_| io::Error::other("capture result lock poisoned"))?
        .take()
        .ok_or_else(|| io::Error::other("window closed before native framebuffer arrived"))??;
    Ok(())
}

fn save(output: &std::path::Path, image: &egui::ColorImage) -> io::Result<()> {
    let width = u32::try_from(image.size[0]).map_err(io::Error::other)?;
    let height = u32::try_from(image.size[1]).map_err(io::Error::other)?;
    let rgba: Vec<_> = image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect();
    let png = encode(ImageFormat::Png, width, height, &rgba, 100).map_err(io::Error::other)?;
    std::fs::write(output, png)
}
