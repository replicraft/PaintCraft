use super::*;
use crate::{App, controller::Editor};
use paintcraft_core::types::{
    Error, ErrorCode, Rgba,
    filters::{BoxBlurV1, Descriptor},
    ids::deterministic_id,
};
use paintcraft_document::{FilterInstance, FilterStack};

fn document() -> Document {
    let mut doc = paintcraft_document::import_image(
        3,
        1,
        &[0, 0, 0, 255, 90, 60, 30, 255, 240, 120, 0, 255],
        "native-spatial-workspace/v1",
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["box"]),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        )])
        .unwrap();
    doc
}

#[test]
fn explicit_native_workspace_is_installed_restored_and_releases_failure_charges() {
    let doc = document();
    let source = doc.clone();
    let private = SharedBudget::new(BYTES);
    assert!(needed(&doc));
    assert_eq!(
        with_admission(private.clone(), || paintcraft_render::render_flat(&doc)).unwrap(),
        [30, 20, 10, 255, 110, 60, 10, 255, 190, 100, 10, 255]
    );
    assert_eq!(private.usage().reserved, 0);
    assert!(private.usage().peak > 0);
    let error = with_admission(private.clone(), || {
        let pixels = paintcraft_render::render_flat(&doc)?;
        assert_eq!(pixels.len(), 12);
        Err::<(), _>(Error::new(
            ErrorCode::OperationCancelled,
            "cancel before acceptance",
        ))
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(private.usage().reserved, 0);
    assert_eq!(
        paintcraft_render::render_flat(&doc).unwrap_err().code,
        ErrorCode::CapabilityUnsupported,
        "Context must not leak outside the actual executing closure"
    );
    assert_eq!(
        with_admission(SharedBudget::new(0), || paintcraft_render::render_flat(
            &doc
        ))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert!(doc.same_semantics(&source));
}

#[test]
fn real_native_sync_and_background_canvas_render_the_same_spatial_pixels() {
    use std::time::{Duration, Instant};
    for asynchronous in [false, true] {
        let ctx = eframe::egui::Context::default();
        let source = document();
        let mut app = App::new(None);
        app.editor = Editor::with_document(source.clone(), None);
        let mut ordinary_reservation = None;
        if asynchronous {
            app.renderer = Some(
                paintcraft_runtime::Executor::with_budget(
                    paintcraft_runtime::Limits {
                        workers: 1,
                        pending_jobs: 2,
                        transient_bytes: 1 << 30,
                    },
                    app.worker_budget.clone(),
                )
                .unwrap(),
            );
            let mut ordinary = source.clone();
            ordinary.layers.get_mut(&ordinary.roots[0]).unwrap().filters = FilterStack::default();
            let mut control = App::new(None);
            control.editor = Editor::with_document(ordinary, None);
            control.renderer = Some(
                paintcraft_runtime::Executor::with_budget(
                    paintcraft_runtime::Limits {
                        workers: 1,
                        pending_jobs: 2,
                        transient_bytes: 1 << 30,
                    },
                    control.worker_budget.clone(),
                )
                .unwrap(),
            );
            control.sync_texture(&ctx);
            ordinary_reservation = Some(control.renderer.as_ref().unwrap().reserved_bytes());
            let until = Instant::now() + Duration::from_secs(10);
            while control.texture_key != Some(control.presentation_key()) {
                assert!(Instant::now() < until);
                control.sync_texture(&ctx);
                std::thread::yield_now();
            }
        }
        app.sync_texture(&ctx);
        if asynchronous {
            assert_eq!(
                app.renderer.as_ref().unwrap().reserved_bytes(),
                ordinary_reservation.unwrap() + BYTES,
                "Same geometry/source adds exactly the spatial ceiling to the parent job, without raising its cap"
            );
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.texture_key != Some(app.presentation_key()) {
            assert!(Instant::now() < deadline, "{}", app.editor.status());
            app.sync_texture(&ctx);
            std::thread::yield_now();
        }
        assert_eq!(
            app.rendered,
            [30, 20, 10, 255, 110, 60, 10, 255, 190, 100, 10, 255]
        );
        assert!(app.editor.document().same_semantics(&source));
        assert_eq!(app.editor.undo_depth(), 0);
        assert_eq!(
            app.editor.document().layers[&source.roots[0]]
                .raster
                .get(1, 0),
            Rgba([90, 60, 30, 255])
        );
        if asynchronous {
            assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 0);
        }
    }
}

#[test]
fn native_picker_and_flatten_route_real_box_appearance_in_sync_and_fifo_modes() {
    use crate::controller::{Action, Tool};
    use paintcraft_core::types::picker::SampleSource;
    use std::time::{Duration, Instant};
    let expected = [30, 20, 10, 255, 110, 60, 10, 255, 190, 100, 10, 255];
    for asynchronous in [false, true] {
        let mut app = App::new(None);
        let mut source = document();
        // Existing flatten is intentionally a no-op for a one-root document.
        // A transparent second root makes this exercise real composite staging.
        let extra = deterministic_id(&["transparent-flatten-root"]);
        let mut layer = source.layers[&source.roots[0]].clone();
        layer.id = extra;
        layer.filters = FilterStack::default();
        layer.raster = paintcraft_raster::RasterLayer::new(3, 1, Rgba([0; 4]));
        source.layers.insert(extra, layer);
        source.roots.insert(0, extra);
        app.editor = Editor::with_document(source.clone(), None);
        if asynchronous {
            app.initialize_editing(app.context.clone());
        }
        let original = app.editor.document_snapshot();
        app.editor.options.picker_source = SampleSource::Image;
        app.editor.select_tool(Tool::ColorPicker);
        app.editor.pointer_down((1, 0));
        app.editor.pointer_up();
        app.editor.run_pending();
        let until = Instant::now() + Duration::from_secs(10);
        while app.editor.has_pending_edits() {
            assert!(Instant::now() < until);
            app.editor.run_pending();
            std::thread::yield_now();
        }
        assert_eq!(app.editor.color, Rgba([110, 60, 10, 255]));
        assert!(std::sync::Arc::ptr_eq(
            &original,
            &app.editor.document_snapshot()
        ));
        assert_eq!(app.editor.undo_depth(), 0);
        app.editor.options.picker_source = SampleSource::Layer;
        app.editor.select_tool(Tool::ColorPicker);
        app.editor.pointer_down((1, 0));
        app.editor.pointer_up();
        app.editor.run_pending();
        while app.editor.has_pending_edits() {
            assert!(Instant::now() < until);
            app.editor.run_pending();
            std::thread::yield_now();
        }
        assert_eq!(
            app.editor.color,
            Rgba([90, 60, 30, 255]),
            "Explicit layer sampling stays raw"
        );
        app.editor.request(993, Action::FlattenImage);
        app.editor.run_pending();
        while app.editor.has_pending_edits() {
            assert!(Instant::now() < until);
            app.editor.run_pending();
            std::thread::yield_now();
        }
        assert_eq!(
            paintcraft_render::render_flat(app.editor.document()).unwrap(),
            expected
        );
        assert!(
            app.editor
                .document()
                .layers
                .values()
                .all(|layer| layer.filters.is_empty())
        );
        assert_eq!(app.editor.undo_depth(), 1);
        app.editor.request(994, Action::Undo);
        app.editor.run_pending();
        while app.editor.has_pending_edits() {
            assert!(Instant::now() < until);
            app.editor.run_pending();
            std::thread::yield_now();
        }
        assert!(app.editor.document().same_semantics(&source));
    }
}

#[test]
fn native_box_gpu_host_installs_real_texture_with_exact_spatial_pixels() {
    use std::time::{Duration, Instant};
    let Some((target, gpu)) = crate::gpu_display::tests::target() else {
        return;
    };
    #[cfg(target_os = "macos")]
    assert_eq!(target.adapter_info().backend, eframe::wgpu::Backend::Metal);
    let source = document();
    let mut app = App::new(None);
    app.editor = Editor::with_document(source.clone(), None);
    app.gpu_completion = Some(paintcraft_render::gpu::CompletionDriver::new(&gpu).unwrap());
    app.gpu = Some(gpu.clone());
    app.gpu_target = Some(target);
    app.renderer = Some(
        paintcraft_runtime::Executor::with_budget(
            paintcraft_runtime::Limits {
                workers: 1,
                pending_jobs: 2,
                transient_bytes: 3 << 30,
            },
            app.worker_budget.clone(),
        )
        .unwrap(),
    );
    let ctx = eframe::egui::Context::default();
    let key = app.presentation_key();
    let until = Instant::now() + Duration::from_secs(20);
    while app.texture_key != Some(key) {
        assert!(Instant::now() < until, "{}", app.editor.status());
        app.sync_texture(&ctx);
        std::thread::yield_now();
    }
    let texture = app
        .gpu_texture
        .as_ref()
        .expect("CPU fallback is not GPU proof");
    assert_eq!(
        texture.frame().readback(|| Ok(())).unwrap(),
        [30, 20, 10, 255, 110, 60, 10, 255, 190, 100, 10, 255]
    );
    assert!(app.rendered.is_empty());
    assert!(std::sync::Arc::ptr_eq(app.gpu.as_ref().unwrap(), &gpu));
    assert!(app.editor.document().same_semantics(&source));
    assert_eq!(app.editor.undo_depth(), 0);
    assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 0);
}

#[test]
fn native_box_save_and_reopen_preserve_pcnd5_intentions_and_raw_source() {
    let path = std::env::temp_dir().join(format!(
        "paintcraft-native-box-save-{}.pcnd",
        crate::Uuid::new_v4()
    ));
    let source = document();
    let mut app = App::new(None);
    app.editor = Editor::with_document(source.clone(), None);
    let before = app.editor.document_snapshot();
    app.save_selected(&eframe::egui::Context::default(), path.clone());
    app.editor.run_pending();
    app.finish_save();
    assert_eq!(app.bound.as_ref(), Some(&path));
    assert!(!app.editor.is_dirty());
    let bytes = std::fs::read(&path).unwrap();
    let header = paintcraft_formats::pcnd::probe(&bytes).unwrap();
    assert_eq!(header.container_version, 5);
    assert_eq!(header.document_schema, "paintcraft.document/v5");
    let reopened = Editor::open(&path).unwrap();
    assert!(reopened.document().same_semantics(&before));
    assert_eq!(reopened.undo_depth(), 0);
    assert_eq!(
        reopened.document().layers[&source.roots[0]].raster,
        source.layers[&source.roots[0]].raster
    );
    assert_eq!(
        with_job(true, || paintcraft_render::render_flat(reopened.document())).unwrap(),
        [30, 20, 10, 255, 110, 60, 10, 255, 190, 100, 10, 255]
    );
    assert_eq!(app.editor.undo_depth(), 0);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_single_layer_box_flatten_bakes_once_downgrades_and_undo_restores() {
    use crate::controller::Action;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    let expected = [30, 20, 10, 255, 110, 60, 10, 255, 190, 100, 10, 255];
    for asynchronous in [false, true] {
        let source = document();
        assert_eq!(source.roots.len(), 1);
        let raw = source.layers[&source.roots[0]].raster.clone();
        let mut app = App::new(None);
        app.editor = Editor::with_document(source.clone(), None);
        if asynchronous {
            app.initialize_editing(app.context.clone());
        }
        let original = app.editor.document_snapshot();
        assert_eq!(
            with_job(true, || paintcraft_render::render_flat(&original)).unwrap(),
            expected
        );
        assert_eq!(
            paintcraft_formats::pcnd::probe(&paintcraft_formats::pcnd::write(&original).unwrap())
                .unwrap()
                .container_version,
            5
        );
        app.editor.request(1101, Action::FlattenImage);
        assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.editor.has_pending_edits() {
            assert!(Instant::now() < deadline, "{}", app.editor.status());
            app.editor.run_pending();
            std::thread::yield_now();
        }
        let baked = app.editor.document_snapshot();
        assert_ne!(baked.state_id, original.state_id);
        assert_eq!(baked.roots.len(), 1);
        assert!(baked.layers[&baked.roots[0]].filters.is_empty());
        assert_eq!(paintcraft_render::render_flat(&baked).unwrap(), expected);
        assert!(paintcraft_editor::session::masked_document_features(&baked).is_empty());
        assert_eq!(
            paintcraft_formats::pcnd::probe(&paintcraft_formats::pcnd::write(&baked).unwrap())
                .unwrap()
                .container_version,
            1
        );
        assert_eq!(app.editor.undo_depth(), 1);
        assert_eq!(original.layers[&source.roots[0]].raster, raw);
        assert!(original.same_semantics(&source));
        app.editor.request(1102, Action::Undo);
        while app.editor.has_pending_edits() {
            assert!(Instant::now() < deadline);
            app.editor.run_pending();
            std::thread::yield_now();
        }
        assert!(app.editor.document().same_semantics(&source));
        assert_eq!(app.editor.document().layers[&source.roots[0]].raster, raw);
        assert_eq!(app.editor.undo_depth(), 0);
        assert_eq!(app.editor.redo_depth(), 1);
    }
}
