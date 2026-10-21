//! Completed single-layer native display versus the existing ordered compositor.
use super::*;
use paintcraft_core::types::Rect as DocumentRect;
use std::hint::black_box;

fn replay(case: &str, enabled: bool, verify: bool) {
    let (target, gpu) = tests::target_with_single_layer(0, enabled).expect("real GPU required");
    if verify {
        eprintln!("adapter={:?}", target.adapter_info());
    }
    let mut app = App::new(None);
    app.editor = match case {
        "tiny" => Editor::new_document(32, 32).unwrap(),
        "large" => Editor::new_document(4096, 2160).unwrap(),
        "opacity" => Editor::new_document(512, 256).unwrap(),
        "dense" | "partial" | "multiple" => {
            let editor = crate::cpu_rendering::geometry_benchmark::fixture_for_base("dense5");
            let mut doc = editor.document().clone();
            if case != "multiple" {
                doc.roots.truncate(1);
                doc.layers.retain(|id, _| doc.roots.contains(id));
            }
            if case == "partial" {
                doc.layers.get_mut(&doc.roots[0]).unwrap().origin = (1, 1);
            }
            Editor::with_document(doc, None)
        }
        _ => panic!("unknown single-layer display case"),
    };
    let region = if case == "large" {
        DocumentRect::new(0, 0, 1024, 768).unwrap()
    } else {
        app.editor.document().canvas_rect()
    };
    let original =
        verify.then(|| paintcraft_render::render_region(app.editor.document(), region).unwrap());
    let ctx = egui::Context::default();
    app.render_region = Some(region);
    app.gpu = Some(gpu.clone());
    app.gpu_target = Some(target);
    app.editor.select_tool(Tool::Brush);
    app.editor.options.size = 16;
    app.editor.color = Rgba([17, 88, 156, 128]);
    if case != "opacity" {
        app.editor.pointer_down((8, 8));
    }
    let mut uploads = 0;
    let mut allocations = 0;
    let mut submissions = 0;
    let mut bypasses = 0;
    for frame in 0..8i64 {
        let staged;
        let batch;
        let doc = if case == "opacity" {
            let layer = app.editor.document().roots[0];
            app.editor.request(
                90000 + frame as u64,
                Action::SetOpacity {
                    layer,
                    opacity: 128 + frame as u8,
                },
            );
            app.editor.run_pending();
            app.editor.document()
        } else {
            for point in frame * 8..(frame + 1) * 8 {
                app.editor.pointer_move((
                    8 + if case == "tiny" { point % 16 } else { point },
                    8 + point % 8,
                ));
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
        let texture = gpu.render_region_texture(doc, region, || Ok(())).unwrap();
        uploads += texture.stats.uploaded_bytes;
        allocations += texture.stats.output_allocations;
        submissions += texture.stats.composition_submissions;
        bypasses += texture.stats.single_layer_displays;
        if verify {
            let oracle;
            let reference = if case == "opacity" {
                doc
            } else {
                oracle = paintcraft_editor::execute::execute(
                    app.editor.document(),
                    app.editor.selection(),
                    &app.editor.preview_batch().unwrap().unwrap(),
                )
                .unwrap();
                &oracle.doc
            };
            let straight = paintcraft_render::render_region(reference, region).unwrap();
            let expected: Vec<_> = egui::ColorImage::from_rgba_unmultiplied(
                [region.width() as usize, region.height() as usize],
                &straight,
            )
            .pixels
            .iter()
            .flat_map(|p| p.to_array())
            .collect();
            assert_eq!(texture.readback(|| Ok(())).unwrap(), expected);
            assert_eq!(
                texture.stats.single_layer_displays,
                u64::from(enabled && !matches!(case, "partial" | "multiple"))
            );
        }
        app.preview_revision = frame as u64 + 1;
        let key = app.presentation_key();
        app.install_gpu_texture(&ctx, key, texture);
        assert_eq!(app.texture_key, Some(key));
        assert!(app.gpu_texture.is_some());
        assert!(app.rendered.is_empty());
        black_box(&app);
    }
    if let Some(original) = original
        && case != "opacity"
    {
        assert_eq!(
            paintcraft_render::render_region(app.editor.document(), region).unwrap(),
            original
        );
    }
    eprintln!(
        "uploaded_bytes={uploads} output_allocations={allocations} compositor_submissions={submissions} single_layer_displays={bypasses}"
    );
    drop(app);
    drop(gpu);
}

#[test]
#[ignore = "optimized native single-layer comparison; use recorded statistical/process configs"]
fn native_single_layer_process() {
    let case = std::env::var("PAINTCRAFT_VIEWPORT_CASE").unwrap();
    match std::env::var("PAINTCRAFT_VIEWPORT_VARIANT").as_deref() {
        Ok("verify") => {
            replay(&case, false, true);
            replay(&case, true, true);
        }
        Ok("ordered") => replay(&case, false, false),
        Ok("single") => replay(&case, true, false),
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
            let mut group = c.benchmark_group(format!("native_single_layer/{case}"));
            group.sampling_mode(SamplingMode::Flat);
            for enabled in [false, true] {
                group.bench_function(
                    BenchmarkId::from_parameter(if enabled { "single" } else { "ordered" }),
                    |b| b.iter(|| replay(&case, enabled, false)),
                );
            }
            group.finish();
            c.final_summary();
        }
        _ => panic!("explicit native single-layer variant required"),
    }
}
