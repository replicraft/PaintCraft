//! Full-ROI composition/filter versus bounded and cached native display readiness.
use super::*;
use crate::{Action, App, Editor, Tool, egui, gpu_display};
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
    let edge = if mode == "gpu64" { 64 } else { 32 };
    let gpu_mode = matches!(mode, "gpu32" | "gpu64");
    let (target, gpu) = gpu_display::tests::target_with_scaled_edge(0, edge)
        .expect("real GPU required for completed display readiness");
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
        "large" | "uniform" => (Editor::new_document(4096, 2160).unwrap(), 2),
        _ => panic!("unknown multires case"),
    };
    let mut app = App::new(None);
    app.editor = editor;
    app.render_level = level;
    if gpu_mode {
        app.gpu = Some(gpu.clone());
        app.gpu_target = Some(target.clone());
    }
    app.editor.select_tool(Tool::Brush);
    app.editor.options.size = 16;
    app.editor.color = Rgba([17, 88, 156, 128]);
    if mode == "bounded" {
        *app.cpu_renderer.2.lock().unwrap() = None;
    }
    let original = (verify && case != "uniform")
        .then(|| paintcraft_render::render_flat(app.editor.document()).unwrap());
    let ticket = app.editor.render_key();
    if case != "uniform" {
        app.editor
            .pointer_down(if case == "tiny" { (8, 8) } else { (132, 68) });
    }
    let request = DisplayRequest::new(
        app.editor.document(),
        app.editor.document().canvas_rect(),
        level,
    )
    .unwrap();
    app.render_region = Some(request.region());
    let mut uploads = 0u64;
    let mut native_pixels = 0u64;
    let mut uniform_pixels = 0u64;
    let mut source_allocations = 0u64;
    let mut source_hits = 0u64;
    for frame in 0..8 {
        let staged;
        let batch;
        let doc = if case == "uniform" {
            let layer = app.editor.document().roots[0];
            app.editor.request(
                90000 + frame as u64,
                Action::SetOpacity {
                    layer,
                    opacity: 128 + frame as u8,
                },
            );
            app.editor.run_pending();
            app.cpu_renderer.clear_geometry();
            app.editor.document()
        } else {
            for point in frame * 8..(frame + 1) * 8 {
                app.editor.pointer_move(if case == "tiny" {
                    (8 + point % 16, 8 + point % 8)
                } else {
                    (132 + point, 68 + point % 32)
                });
            }
            batch = app.editor.preview_batch().unwrap().unwrap();
            staged = app
                .cpu_renderer
                .stage(
                    app.editor.document(),
                    app.editor.selection(),
                    &batch,
                    &|| Ok(()),
                )
                .unwrap();
            &staged.doc
        };
        let expected = if verify {
            if case == "uniform" {
                Some(full_filter(doc, request))
            } else {
                let independent = paintcraft_editor::execute::execute(
                    app.editor.document(),
                    app.editor.selection(),
                    &app.editor.preview_batch().unwrap().unwrap(),
                )
                .unwrap();
                Some(full_filter(&independent.doc, request))
            }
        } else {
            None
        };
        app.preview_revision = frame as u64 + 1;
        let key = app.presentation_key();
        if gpu_mode {
            let texture = gpu.render_display_texture(doc, request, || Ok(())).unwrap();
            uploads += texture.stats.uploaded_bytes;
            native_pixels += texture.stats.scaled_native_pixels;
            uniform_pixels += texture.stats.scaled_uniform_pixels;
            source_allocations += texture.stats.source_allocations;
            source_hits += texture.stats.source_hits;
            if let Some(expected) = expected {
                let expected: Vec<_> = egui::ColorImage::from_rgba_unmultiplied(
                    [request.size()[0] as usize, request.size()[1] as usize],
                    &expected,
                )
                .pixels
                .iter()
                .flat_map(|p| p.to_array())
                .collect();
                assert!(
                    texture.readback(|| Ok(())).unwrap() == expected,
                    "GPU scaled pixels differ from independent command/native/box output"
                );
            }
            app.install_gpu_texture(&ctx, key, texture);
            assert!(app.gpu_texture.is_some());
            assert!(app.rendered.is_empty());
        } else {
            let pixels = match mode {
                "full" => {
                    let native = app
                        .cpu_renderer
                        .render_region(doc, request.region(), case != "uniform", || Ok(()))
                        .unwrap();
                    filter_pixels(&native, request)
                }
                "cpu" | "bounded" | "cached" => app
                    .cpu_renderer
                    .render_display(doc, request.region(), level, case != "uniform", || Ok(()))
                    .unwrap(),
                _ => panic!("unknown display mode"),
            };
            if let Some(expected) = expected {
                assert!(
                    pixels == expected,
                    "CPU display differs from independent command/native/box output"
                );
            }
            app.install_render(&ctx, key, Ok(pixels));
            let mut delta = ctx.tex_manager().write().take_delta();
            gpu_display::benchmark::upload(&target, &mut delta);
        }
        assert_eq!(app.texture_key, Some(key));
        assert_eq!(app.rendered_size(), Some(request.size()));
        black_box(&app);
    }
    if case == "uniform" {
        assert_eq!(app.editor.publications(), 8);
        let raster = &app.editor.document().layers[&app.editor.document().roots[0]].raster;
        assert_eq!(raster.resident_tiles(), 0);
        assert_eq!(raster.default_pixel(), Rgba([255; 4]));
    }
    if let Some(original) = original {
        assert!(paintcraft_render::render_flat(app.editor.document()).unwrap() == original);
    }
    if case != "uniform" {
        assert_eq!(app.editor.render_key(), ticket);
    }
    eprintln!(
        "gpu_upload_bytes={uploads} gpu_native_pixels={native_pixels} gpu_uniform_pixels={uniform_pixels} gpu_source_allocations={source_allocations} gpu_source_hits={source_hits} gpu_cache={:?}",
        gpu.source_cache_usage()
    );
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
        Ok("gpu-verify") => {
            for mode in ["cpu", "gpu32", "gpu64"] {
                replay(&case, mode, true);
            }
        }
        Ok("cpu") => replay(&case, "cpu", false),
        Ok("gpu32") => replay(&case, "gpu32", false),
        Ok("gpu64") => replay(&case, "gpu64", false),
        Ok("gpu-criterion") => {
            use criterion::{BenchmarkId, Criterion, SamplingMode};
            let mut c = Criterion::default()
                .sample_size(20)
                .warm_up_time(Duration::from_millis(300))
                .measurement_time(Duration::from_secs(1))
                .without_plots();
            let mut group = c.benchmark_group(format!("native_gpu_scaled/{case}"));
            group.sampling_mode(SamplingMode::Flat);
            for mode in ["cpu", "gpu32", "gpu64"] {
                group.bench_function(BenchmarkId::from_parameter(mode), |b| {
                    b.iter(|| replay(&case, mode, false))
                });
            }
            group.finish();
            c.final_summary();
        }
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
