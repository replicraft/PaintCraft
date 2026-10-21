//! Full-ROI composition/filter versus bounded and cached native display readiness.
use super::*;
use crate::{App, Editor, Tool, egui, gpu_display};
use paintcraft_core::types::Rgba;
use paintcraft_render::display::DisplayRequest;
use std::{hint::black_box, time::Duration};

/// Independent straightforward policy oracle: first materialize exact native ROI,
/// then sum every source pixel of each box. Only used by the benchmark/control.
fn full_filter(doc: &Document, request: DisplayRequest) -> Vec<u8> {
    let native = paintcraft_render::render_region(doc, request.region()).unwrap();
    filter_pixels(&native, request)
}
fn filter_pixels(native: &[u8], request: DisplayRequest) -> Vec<u8> {
    let region = request.region();
    let [w, h] = request.size();
    let step = i64::from(request.step());
    let mut output = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let x0 = i64::from(x) * step;
            let y0 = i64::from(y) * step;
            let x1 = (x0 + step).min(region.width());
            let y1 = (y0 + step).min(region.height());
            let count = ((x1 - x0) * (y1 - y0)) as u64;
            let mut sum = [0u64; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let pixel =
                        &native[(sy as usize * region.width() as usize + sx as usize) * 4..][..4];
                    let alpha = u64::from(pixel[3]);
                    for c in 0..3 {
                        sum[c] += u64::from(pixel[c]) * alpha;
                    }
                    sum[3] += alpha;
                }
            }
            let alpha = (sum[3] + count / 2) / count;
            if alpha == 0 {
                output.extend([0; 4]);
            } else {
                output.extend([
                    ((sum[0] + sum[3] / 2) / sum[3]) as u8,
                    ((sum[1] + sum[3] / 2) / sum[3]) as u8,
                    ((sum[2] + sum[3] / 2) / sum[3]) as u8,
                    alpha as u8,
                ]);
            }
        }
    }
    output
}
fn replay(case: &str, mode: &str, verify: bool) {
    let (target, _gpu) =
        gpu_display::tests::target().expect("real GPU required for completed managed upload");
    if verify {
        eprintln!("adapter={:?}", target.adapter_info());
    }
    let ctx = egui::Context::default();
    let mut initial = ctx.run_ui(Default::default(), |ui| {
        ui.label("display initialization");
    });
    gpu_display::benchmark::upload(&target, &mut initial.textures_delta);
    let (editor, level) = match case {
        "tiny" => (Editor::new_document(32, 32).unwrap(), 1),
        "dense" => (super::geometry_benchmark::fixture_for_base("dense5"), 1),
        "large" => (Editor::new_document(4096, 2160).unwrap(), 2),
        _ => panic!("unknown multires case"),
    };
    let mut app = App::new(None);
    app.editor = editor;
    app.render_level = level;
    app.editor.select_tool(Tool::Brush);
    app.editor.options.size = 16;
    app.editor.color = Rgba([17, 88, 156, 128]);
    if mode == "bounded" {
        *app.cpu_renderer.2.lock().unwrap() = None;
    }
    let original = verify.then(|| paintcraft_render::render_flat(app.editor.document()).unwrap());
    let ticket = app.editor.render_key();
    app.editor
        .pointer_down(if case == "tiny" { (8, 8) } else { (132, 68) });
    let request = DisplayRequest::new(
        app.editor.document(),
        app.editor.document().canvas_rect(),
        level,
    )
    .unwrap();
    app.render_region = Some(request.region());
    for frame in 0..8 {
        for point in frame * 8..(frame + 1) * 8 {
            app.editor.pointer_move(if case == "tiny" {
                (8 + point % 16, 8 + point % 8)
            } else {
                (132 + point, 68 + point % 32)
            });
        }
        let batch = app.editor.preview_batch().unwrap().unwrap();
        let staged = app
            .cpu_renderer
            .stage(
                app.editor.document(),
                app.editor.selection(),
                &batch,
                &|| Ok(()),
            )
            .unwrap();
        let pixels = match mode {
            "full" => {
                let native = app
                    .cpu_renderer
                    .render_region(&staged.doc, request.region(), true, || Ok(()))
                    .unwrap();
                filter_pixels(&native, request)
            }
            "bounded" | "cached" => app
                .cpu_renderer
                .render_display(&staged.doc, request.region(), level, true, || Ok(()))
                .unwrap(),
            _ => panic!("unknown display mode"),
        };
        if verify {
            let independent = paintcraft_editor::execute::execute(
                app.editor.document(),
                app.editor.selection(),
                &batch,
            )
            .unwrap();
            assert!(
                pixels == full_filter(&independent.doc, request),
                "display differs from independent command/scalar/filter output"
            );
        }
        app.preview_revision = frame as u64 + 1;
        let key = app.presentation_key();
        app.install_render(&ctx, key, Ok(pixels));
        assert_eq!(app.texture_key, Some(key));
        assert_eq!(app.rendered_size(), Some(request.size()));
        let mut delta = ctx.tex_manager().write().take_delta();
        gpu_display::benchmark::upload(&target, &mut delta);
        black_box(&app);
    }
    if let Some(original) = original {
        assert!(paintcraft_render::render_flat(app.editor.document()).unwrap() == original);
    }
    assert_eq!(app.editor.render_key(), ticket);
    let usage = app
        .cpu_renderer
        .2
        .lock()
        .unwrap()
        .as_ref()
        .map(|cache| cache.usage(Domain::Preview));
    let native_usage = app
        .cpu_renderer
        .0
        .lock()
        .unwrap()
        .as_ref()
        .map(|cache| cache.usage(Domain::Preview));
    eprintln!(
        "case={case} mode={mode} level={level} source={:?} output={:?} coarse_preview_cache={usage:?} native_preview_cache={native_usage:?}",
        app.editor.document().canvas,
        request.size()
    );
    drop(app);
    let mut free = ctx.tex_manager().write().take_delta();
    gpu_display::benchmark::upload(&target, &mut free);
}
#[test]
#[ignore = "optimized native multiresolution comparison; execute recorded statistical/process configs"]
fn native_multires_process() {
    let case = std::env::var("PAINTCRAFT_VIEWPORT_CASE").unwrap_or_else(|_| "dense".into());
    match std::env::var("PAINTCRAFT_VIEWPORT_VARIANT").as_deref() {
        Ok("verify") => {
            for mode in ["full", "bounded", "cached"] {
                replay(&case, mode, true);
            }
        }
        Ok("full") => replay(&case, "full", false),
        Ok("bounded") => replay(&case, "bounded", false),
        Ok("cached") => replay(&case, "cached", false),
        Ok("calibration") => {
            black_box(0u64);
        }
        Ok("criterion") => {
            use criterion::{BenchmarkId, Criterion, SamplingMode};
            let mut c = Criterion::default()
                .sample_size(20)
                .warm_up_time(Duration::from_millis(300))
                .measurement_time(Duration::from_secs(1))
                .without_plots();
            let mut group = c.benchmark_group(format!("native_multires/{case}"));
            group.sampling_mode(SamplingMode::Flat);
            for mode in ["full", "bounded", "cached"] {
                group.bench_function(BenchmarkId::from_parameter(mode), |b| {
                    b.iter(|| replay(&case, mode, false))
                });
            }
            group.finish();
            c.final_summary();
        }
        _ => panic!("explicit multires variant required"),
    }
}
