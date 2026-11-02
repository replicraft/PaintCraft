//! Real Studio Add/Preview/Cancel/Apply with stored layer blending.
use super::*;
use paintcraft_core::types::{LayerBlendMode as Mode, filters::Descriptor, json};
use sha2::{Digest, Sha256};
struct Saved(PathBuf);
impl Drop for Saved {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_file(&self.0).unwrap();
        }
    }
}
fn oracle() -> json::Value {
    let raw = include_bytes!("../../../tests/fixtures/layer-blend-studio-filter/oracle.json");
    assert_eq!(
        paintcraft_core::types::policy::hex(&Sha256::digest(raw)),
        "10ec6d01a9fd21317a254c3251a1cad3b5d8476c8ea42104f2bf5d6cc5e88fbb"
    );
    json::parse(raw).unwrap()
}
fn rgba(value: &json::Value) -> Rgba {
    Rgba(std::array::from_fn(|i| {
        u8::try_from(value.as_array().unwrap()[i].as_i64().unwrap()).unwrap()
    }))
}
fn literal(mode: Mode, stage: &str) -> Vec<u8> {
    let data = oracle();
    let mode = match mode {
        Mode::Multiply => "multiply",
        Mode::Screen => "screen",
        Mode::Normal => panic!("nonNormal cases only"),
    };
    let row = data
        .get("records")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row.get("mode").unwrap().as_str() == Some(mode)
                && row.get("stage").unwrap().as_str() == Some(stage)
        })
        .unwrap();
    rgba(row.get("expected").unwrap()).0.repeat(2)
}
fn mode_app(asynchronous: bool, mode: Mode, target_top: bool) -> App {
    let data = oracle();
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(rgba(data.get("sourceBack").unwrap())),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut top = doc.layers[&doc.roots[0]].clone();
    top.id = Uuid::from_u128(0x62b0);
    top.blend_mode = mode;
    top.raster = paintcraft_raster::RasterLayer::new(2, 1, rgba(data.get("sourceFront").unwrap()));
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);
    let target = doc.roots[usize::from(target_top)];
    // Reuse existing actual Studio controller/worker setup and host install.
    let mut app = app(asynchronous);
    app.install(Editor::with_document(doc, None)).unwrap();
    app.editor.set_target(target);
    app
}
fn save_native(h: &mut Harness<'_, App>, ctx: &egui::Context, path: &Path) {
    h.state_mut().save_selected(ctx, path.to_path_buf());
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let app = h.state_mut();
        app.editor.run_pending();
        app.pump_filter_edit(ctx);
        app.poll_edits(ctx);
        app.finish_save();
        app.poll_native_saves(ctx);
        if path.exists()
            && app.bound.as_deref() == Some(path)
            && app.deferred_files.is_empty()
            && !app.editor.has_pending_edits()
        {
            break;
        }
        assert!(Instant::now() < until, "save: {}", app.editor.status());
        std::thread::yield_now();
    }
}
#[test]
fn actual_studio_invert_preview_cancel_apply_history_and_pcnd8_cover_both_proof_paths() {
    for asynchronous in [false, true] {
        for mode in [Mode::Multiply, Mode::Screen] {
            for target_top in [false, true] {
                let mut h = harness(mode_app(asynchronous, mode, target_top));
                h.run();
                let ctx = h.ctx.clone();
                let source = h.state().editor.document_snapshot();
                let target = h.state().editor.target().unwrap();
                let ticket = h.state().editor.history_view().ticket;
                let dirty = h.state().editor.published_is_dirty();
                let before =
                    paintcraft_render::blend::quote_document(&source, source.canvas_rect())
                        .unwrap();
                assert_eq!((before.opaque_samples, before.general_samples), (2, 0));
                frame(h.state_mut(), &ctx);
                assert_eq!(h.state().rendered, literal(mode, "initial"));
                let stage = if target_top {
                    "invert-top"
                } else {
                    "invert-bottom"
                };
                open_invert(&mut h);
                frame(h.state_mut(), &ctx);
                assert_eq!(h.state().rendered, literal(mode, stage));
                assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
                assert_eq!(h.state().editor.history_view().ticket, ticket);
                assert_eq!(h.state().editor.published_is_dirty(), dirty);
                assert!(h.state().editor.export_snapshot().is_err());
                h.get_by_label("Preview").click();
                h.run();
                frame(h.state_mut(), &ctx);
                assert_eq!(h.state().rendered, literal(mode, "initial"));
                h.get_by_label("Preview").click();
                h.run();
                frame(h.state_mut(), &ctx);
                assert_eq!(h.state().rendered, literal(mode, stage));
                let private_key = h.state().presentation_key();
                h.get_by_label("Cancel").click();
                h.run();
                frame(h.state_mut(), &ctx);
                assert_eq!(h.state().rendered, literal(mode, "initial"));
                assert!(!h.state().accept_render_key(private_key));
                assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
                assert_eq!(h.state().editor.history_view().ticket, ticket);
                assert_eq!(h.state().editor.published_is_dirty(), dirty);
                open_invert(&mut h);
                h.get_by_label("Apply").click();
                h.run_steps(1);
                drain(h.state_mut());
                frame(h.state_mut(), &ctx);
                assert!(h.state().filter_editing.is_none());
                assert_eq!(h.state().rendered, literal(mode, stage));
                assert_eq!(h.state().editor.undo_depth(), 1);
                let applied = h.state().editor.document_snapshot();
                assert_eq!(applied.layers[&target].filters.len(), 1);
                assert_eq!(
                    applied.layers[&target].filters.as_slice()[0].descriptor(),
                    Descriptor::InvertRgbV1
                );
                for id in &source.roots {
                    assert_eq!(applied.layers[id].raster, source.layers[id].raster);
                    assert_eq!(applied.layers[id].blend_mode, source.layers[id].blend_mode);
                }
                let after =
                    paintcraft_render::blend::quote_document(&applied, applied.canvas_rect())
                        .unwrap();
                assert_eq!(
                    (after.opaque_samples, after.general_samples),
                    if target_top { (2, 0) } else { (0, 2) },
                    "filtering opaque backdrop must invalidate its certificate"
                );
                let path = Saved(
                    std::env::temp_dir()
                        .join(format!("paintcraft-studio-blends-{}.pcnd", Uuid::new_v4())),
                );
                save_native(&mut h, &ctx, &path.0);
                let bytes = std::fs::read(&path.0).unwrap();
                assert_eq!(bytes[4], 8);
                let reopened = Editor::open(&path.0).unwrap();
                assert!(reopened.document().same_semantics(&applied));
                assert!(h.state().editor.document().same_semantics(&applied));
                assert!(!h.state().editor.published_is_dirty());
                h.state_mut().editor.request(0x62b1, Action::Undo);
                drain(h.state_mut());
                frame(h.state_mut(), &ctx);
                assert_eq!(h.state().rendered, literal(mode, "initial"));
                assert!(h.state().editor.document().same_semantics(&source));
                assert_eq!(h.state().editor.undo_depth(), 0);
                assert_eq!(h.state().editor.redo_depth(), 1);
                h.state_mut().editor.request(0x62b2, Action::Redo);
                drain(h.state_mut());
                frame(h.state_mut(), &ctx);
                assert_eq!(h.state().rendered, literal(mode, stage));
                assert!(h.state().editor.document().same_semantics(&applied));
                assert_eq!(h.state().editor.undo_depth(), 1);
                assert_eq!(h.state().editor.redo_depth(), 0);
                assert!(!h.state().editor.published_is_dirty());
            }
        }
    }
}
