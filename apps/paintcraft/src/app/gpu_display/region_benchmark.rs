//! Completed native viewport texture readiness, with full-output control.
use super::*;
use paintcraft_core::types::Rect as DocumentRect;
use std::hint::black_box;

fn requested(case: &str, frame: i64) -> DocumentRect {
    match case {
        "all" => DocumentRect::new(0, 0, 512, 256).unwrap(),
        "quarter" => DocumentRect::new(129, 33, 128, 96).unwrap(),
        "pan" => DocumentRect::new(17 + (frame % 4) * 81, 33 + (frame % 3) * 19, 128, 96).unwrap(),
        _ => panic!("unknown viewport case"),
    }
}
fn replay(case: &str, regional: bool, verify: bool) {
    let (target, gpu) = super::tests::target().expect("real GPU required");
    if verify {
        eprintln!(
            "adapter={:?} limits={:?}",
            target.state.adapter.get_info(),
            target.state.device.limits()
        );
    }
    let mut app = App::new(None);
    app.editor = crate::cpu_rendering::geometry_benchmark::fixture();
    let ticket = app.editor.render_key();
    let original = verify.then(|| paintcraft_render::render_flat(app.editor.document()).unwrap());
    app.gpu = Some(gpu.clone());
    app.gpu_target = Some(target);
    app.editor.select_tool(Tool::Brush);
    app.editor.options.size = 16;
    app.editor.color = Rgba([17, 88, 156, 128]);
    app.editor.pointer_down((132, 68));
    let ctx = egui::Context::default();
    let mut uploaded = 0u64;
    let mut output = 0u128;
    for frame in 0..16 {
        for point in frame * 8..(frame + 1) * 8 {
            app.editor.pointer_move((132 + point, 68 + point % 32));
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
        let visible = requested(case, frame);
        let region = if regional {
            visible
        } else {
            staged.doc.canvas_rect()
        };
        app.render_region = Some(region);
        app.preview_revision = frame as u64 + 1;
        let key = app.presentation_key();
        let texture = gpu
            .render_region_texture(&staged.doc, region, || Ok(()))
            .unwrap();
        uploaded += texture.stats.uploaded_bytes;
        output += region.area() * 4;
        if verify {
            let reference = paintcraft_editor::execute::execute(
                app.editor.document(),
                app.editor.selection(),
                &batch,
            )
            .unwrap();
            let straight = paintcraft_render::render_region(&reference.doc, visible).unwrap();
            let expected: Vec<_> = egui::ColorImage::from_rgba_unmultiplied(
                [visible.width() as usize, visible.height() as usize],
                &straight,
            )
            .pixels
            .iter()
            .flat_map(|p| p.to_array())
            .collect();
            let pixels = texture.readback(|| Ok(())).unwrap();
            let mut cropped = Vec::new();
            for y in visible.y0..visible.y1 {
                let start =
                    ((y - region.y0) * region.width() + visible.x0 - region.x0) as usize * 4;
                cropped.extend_from_slice(&pixels[start..start + visible.width() as usize * 4]);
            }
            assert!(
                cropped == expected,
                "visible output differs from independent command/scalar oracle"
            );
        }
        app.install_gpu_texture(&ctx, key, texture);
        assert_eq!(app.texture_key, Some(key));
        assert!(app.gpu_texture.is_some());
        assert!(app.rendered.is_empty());
        black_box(&app);
    }
    assert_eq!(app.editor.render_key(), ticket);
    if let Some(original) = original {
        assert!(paintcraft_render::render_flat(app.editor.document()).unwrap() == original);
    }
    eprintln!("completed_output_bytes={output} uploaded_source_bytes={uploaded}");
    drop(app);
    drop(gpu);
}

#[test]
#[ignore = "optimized native viewport comparison; execute recorded statistical/process configs"]
fn native_viewport_process() {
    let case = std::env::var("PAINTCRAFT_VIEWPORT_CASE").unwrap_or_else(|_| "quarter".into());
    match std::env::var("PAINTCRAFT_VIEWPORT_VARIANT").as_deref() {
        Ok("verify") => {
            replay(&case, false, true);
            replay(&case, true, true);
        }
        Ok("full") => replay(&case, false, false),
        Ok("region") => replay(&case, true, false),
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
            let mut group = c.benchmark_group(format!("native_viewport/{case}"));
            group.sampling_mode(SamplingMode::Flat);
            for regional in [false, true] {
                group.bench_function(
                    BenchmarkId::from_parameter(if regional { "region" } else { "full" }),
                    |b| b.iter(|| replay(&case, regional, false)),
                );
            }
            group.finish();
            c.final_summary();
        }
        _ => panic!("explicit viewport variant required"),
    }
}
