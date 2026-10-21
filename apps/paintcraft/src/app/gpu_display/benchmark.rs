//! Completed native texture readiness versus the previous CPU readback/upload path.
use super::*;
use std::hint::black_box;

fn replay(direct: bool, verify: bool) {
    let (target, gpu) = super::tests::target().expect("display benchmark requires real GPU");
    eprintln!("Display adapter: {:?}", target.state.adapter.get_info());
    let ctx = egui::Context::default();
    let mut initial = ctx.run_ui(Default::default(), |ui| {
        ui.label("display initialization");
    });
    upload(&target, &mut initial.textures_delta);
    let mut app = App::new(None);
    app.editor = crate::cpu_rendering::geometry_benchmark::fixture();
    app.editor.select_tool(Tool::Brush);
    app.editor.options.size = 16;
    app.editor.color = Rgba([17, 88, 156, 128]);
    app.gpu = Some(gpu.clone());
    app.gpu_target = Some(target.clone());
    let original = verify.then(|| paintcraft_render::render_flat(app.editor.document()).unwrap());
    app.editor.pointer_down((8, 64));
    for frame_index in 0..32 {
        for i in frame_index * 16..(frame_index + 1) * 16 {
            app.editor.pointer_move((8 + i, 64 + i % 32));
        }
        app.preview_revision = frame_index as u64 + 1;
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
        let key = app.presentation_key();
        if direct {
            let frame = gpu.render_texture(&staged.doc, || Ok(())).unwrap();
            if verify {
                let straight = paintcraft_render::render_flat(&staged.doc).unwrap();
                let expected: Vec<u8> =
                    egui::ColorImage::from_rgba_unmultiplied([512, 256], &straight)
                        .pixels
                        .iter()
                        .flat_map(|p| p.to_array())
                        .collect();
                assert_eq!(frame.readback(|| Ok(())).unwrap(), expected);
            }
            app.install_gpu_texture(&ctx, key, frame);
            assert!(app.gpu_texture.is_some());
            assert!(app.rendered.is_empty());
        } else {
            let pixels = gpu.render_cancellable(&staged.doc, || Ok(())).unwrap();
            if verify {
                assert_eq!(pixels, paintcraft_render::render_flat(&staged.doc).unwrap());
            }
            app.install_render(&ctx, key, Ok(pixels));
            assert!(app.texture.is_some());
            assert!(app.gpu_texture.is_none());
            // Perform the backend upload that normally follows App::update, then
            // wait for completion. Submission alone is not texture readiness.
            let mut delta = ctx.tex_manager().write().take_delta();
            upload(&target, &mut delta);
        }
        assert_eq!(app.texture_key, Some(key));
        black_box(&app);
    }
    if let Some(original) = original {
        assert_eq!(
            paintcraft_render::render_flat(app.editor.document()).unwrap(),
            original
        );
    }
    drop(app);
    drop(gpu);
    drop(target);
}

#[test]
#[ignore = "optimized native display comparison; execute recorded statistical/process configs"]
fn native_display_process() {
    match std::env::var("PAINTCRAFT_DISPLAY_VARIANT").as_deref() {
        Ok("verify") => {
            replay(false, true);
            replay(true, true);
        }
        Ok("readback") => replay(false, false),
        Ok("texture") => replay(true, false),
        Ok("calibration") => {
            black_box(0u64);
        }
        Ok("criterion") => {
            use criterion::{BenchmarkId, Criterion, SamplingMode};
            let base = std::env::var("PAINTCRAFT_GEOMETRY_BASE").unwrap();
            let mut c = Criterion::default()
                .sample_size(20)
                .warm_up_time(Duration::from_millis(300))
                .measurement_time(Duration::from_secs(1))
                .without_plots();
            let mut group = c.benchmark_group(format!("native_display/{base}"));
            group.sampling_mode(SamplingMode::Flat);
            for direct in [false, true] {
                group.bench_function(
                    BenchmarkId::from_parameter(if direct { "texture" } else { "readback" }),
                    |b| b.iter(|| replay(direct, false)),
                );
            }
            group.finish();
            c.final_summary();
        }
        _ => panic!("set PAINTCRAFT_DISPLAY_VARIANT"),
    }
}

fn upload(target: &Target, delta: &mut egui::TexturesDelta) {
    {
        let mut renderer = target.state.renderer.write();
        for (id, images) in &delta.set {
            for image in images {
                renderer.update_texture(&target.state.device, &target.state.queue, *id, image);
            }
        }
        for id in &delta.free {
            renderer.free_texture(id);
        }
    }
    delta.clear();
    let submission = target.state.queue.submit([]);
    target
        .state
        .device
        .poll(eframe::wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })
        .unwrap();
}
