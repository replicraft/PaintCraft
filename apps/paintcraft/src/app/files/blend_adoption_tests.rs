//! Native adoption only; Browser and old foreign/clipboard authorities stay closed.
use super::*;
use paintcraft_core::types::{
    ErrorCode, LayerBlendMode as Mode, Rect, Rgba,
    filters::{AddNoiseRgbV1, BoxBlurV1, Descriptor, NoiseDistribution},
    resources::SharedBudget,
};
use paintcraft_document::{Document, FilterInstance, FilterStack, LayerMask, POLICY};
use std::{sync::Arc, time::Instant};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("paintcraft-blend-view-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
pub(crate) fn source(mode: Mode, width: u32, height: u32, decorated: bool) -> Document {
    let mut doc =
        paintcraft_document::new_document(width, height, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.id = Uuid::from_u128(0x628a);
    layer.raster = paintcraft_raster::RasterLayer::new(width, height, Rgba([204, 77, 13, 255]));
    layer.blend_mode = mode;
    if decorated {
        layer.opacity = 128;
        layer.mask = Some(LayerMask::new(
            true,
            paintcraft_raster::coverage_mask::A8Mask::uniform(
                Rect::new(0, 0, i64::from(width), i64::from(height)).unwrap(),
                128,
                &SharedBudget::new(8192),
                paintcraft_raster::coverage_mask::Limits::default(),
                &|| Ok(()),
            )
            .unwrap(),
        ));
        layer.filters = FilterStack::new(vec![
            FilterInstance::new(
                Uuid::from_u128(0x628b),
                true,
                Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
            ),
            FilterInstance::new(
                Uuid::from_u128(0x628c),
                true,
                Descriptor::AddNoiseRgbV1(
                    AddNoiseRgbV1::new(0, 65535, false, NoiseDistribution::Gaussian).unwrap(),
                ),
            ),
        ])
        .unwrap();
    }
    doc.roots.push(layer.id);
    doc.layers.insert(layer.id, layer);
    doc
}
pub(crate) fn expected(mode: Mode, decorated: bool) -> [u8; 4] {
    // White backdrop. Constant Box and amount0 Noise are identities; own128 then
    // opacity128 gives a64. Multiply round((s*64+255*191)/255), Screen=white.
    match (mode, decorated) {
        (Mode::Multiply, true) => [242, 210, 194, 255],
        (Mode::Multiply, false) => [204, 77, 13, 255],
        (Mode::Screen, _) => [255; 4],
        _ => panic!("only adopted nonNormal modes"),
    }
}
fn wait_edits(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while app.editor.has_pending_edits() {
        assert!(Instant::now() < deadline, "{}", app.editor.status());
        app.editor.run_pending();
        std::thread::yield_now();
    }
}
fn executor(app: &mut App) {
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
}
fn display(app: &mut App, ctx: &egui::Context) {
    let key = app.presentation_key();
    let deadline = Instant::now() + Duration::from_secs(20);
    while app.texture_key != Some(key) {
        assert!(Instant::now() < deadline, "{}", app.editor.status());
        app.sync_texture(ctx);
        std::thread::yield_now();
    }
}
fn save_native(app: &mut App, path: PathBuf) {
    let ctx = egui::Context::default();
    app.save_selected(&ctx, path.clone());
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        app.editor.run_pending();
        app.poll_edits(&ctx);
        app.finish_save();
        app.poll_native_saves(&ctx);
        if path.exists()
            && app.bound.as_ref() == Some(&path)
            && app.deferred_files.is_empty()
            && !app.editor.has_pending_edits()
        {
            break;
        }
        assert!(Instant::now() < deadline, "{}", app.editor.status());
        std::thread::yield_now();
    }
}
#[test]
fn acquired_pcnd8_and_typed_install_preserve_hidden_zero_and_visible_intent() {
    let folder = Folder::new();
    for experience in [Experience::Classic, Experience::Studio] {
        for mode in [Mode::Multiply, Mode::Screen] {
            for hidden in [false, true] {
                let mut doc = source(mode, 2, 1, true);
                if hidden {
                    let layer = doc.layers.get_mut(doc.roots.last().unwrap()).unwrap();
                    layer.visible = false;
                    layer.opacity = 0;
                }
                let bytes = paintcraft_formats::pcnd::write(&doc).unwrap();
                assert_eq!(bytes[4], 8);
                let path = folder
                    .0
                    .join(format!("{experience:?}-{mode:?}-{hidden}.pcnd"));
                std::fs::write(&path, &bytes).unwrap();
                let standalone = Editor::open(&path).unwrap();
                assert!(standalone.document().same_semantics(&doc));
                let mut app = App::new(None);
                app.settings.experience = experience;
                app.editor.request(0x62a0, Action::AddLayer);
                app.editor.run_pending();
                let old_owner = app.active_tab_id();
                let old_source = app.editor.document_snapshot();
                let old_ticket = app.editor.history_view().ticket;
                let old_tabs = app.tabs.len();
                app.open_path(&path).unwrap();
                let opened_owner = app.active_tab_id();
                assert!(app.editor.document().same_semantics(&doc));
                assert_eq!(app.editor.undo_depth(), 0);
                if experience == Experience::Studio {
                    assert_eq!(app.tabs.len(), old_tabs + 1);
                    assert_ne!(opened_owner, old_owner);
                    assert!(app.swap_tab_state(old_owner));
                    assert!(Arc::ptr_eq(&old_source, &app.editor.document_snapshot()));
                    assert_eq!(app.editor.history_view().ticket, old_ticket);
                    assert!(app.swap_tab_state(opened_owner));
                    assert!(app.editor.document().same_semantics(&doc));
                } else {
                    assert_eq!(app.tabs.len(), old_tabs);
                    assert_eq!(opened_owner, old_owner);
                    assert!(!Arc::ptr_eq(&old_source, &app.editor.document_snapshot()));
                }
                let opened_source = app.editor.document_snapshot();
                let opened_ticket = app.editor.history_view().ticket;
                let opened_tabs = app.tabs.len();
                app.install(Editor::with_document(doc.clone(), None))
                    .unwrap();
                let installed_owner = app.active_tab_id();
                assert!(app.editor.document().same_semantics(&doc));
                if experience == Experience::Studio {
                    assert_eq!(app.tabs.len(), opened_tabs + 1);
                    assert_ne!(installed_owner, opened_owner);
                    assert!(app.swap_tab_state(opened_owner));
                    assert!(Arc::ptr_eq(&opened_source, &app.editor.document_snapshot()));
                    assert_eq!(app.editor.history_view().ticket, opened_ticket);
                    assert!(app.swap_tab_state(old_owner));
                    assert!(Arc::ptr_eq(&old_source, &app.editor.document_snapshot()));
                    assert_eq!(app.editor.history_view().ticket, old_ticket);
                    assert!(app.swap_tab_state(installed_owner));
                    assert!(app.editor.document().same_semantics(&doc));
                } else {
                    assert_eq!(app.tabs.len(), opened_tabs);
                    assert_eq!(installed_owner, opened_owner);
                }
                assert_eq!(std::fs::read(&path).unwrap(), bytes);
            }
        }
    }
    let normal = source(Mode::Normal, 2, 1, false);
    let path = folder.0.join("normal.pcnd");
    let bytes = paintcraft_formats::pcnd::write(&normal).unwrap();
    assert_eq!(bytes[4], 1);
    std::fs::write(&path, &bytes).unwrap();
    assert!(
        Editor::open(&path)
            .unwrap()
            .document()
            .same_semantics(&normal)
    );
    let mut app = App::new(None);
    app.open_path(&path).unwrap();
    assert!(app.editor.document().same_semantics(&normal));
}
#[test]
fn malformed_acquired_source_cannot_replace_private_draft_history_or_binding() {
    let folder = Folder::new();
    let mut bytes = paintcraft_formats::pcnd::write(&source(Mode::Multiply, 2, 1, true)).unwrap();
    bytes.truncate(bytes.len() - 1);
    let path = folder.0.join("truncated.pcnd");
    std::fs::write(&path, &bytes).unwrap();
    let mut app = App::new(None);
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((3, 2));
    app.editor.type_text("keep pending text");
    let before = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    let stamp = app.editor.preview_stamp();
    let text = app.editor.canvas_text_content().unwrap().to_owned();
    let bound = app.bound.clone();
    let tabs = app.tabs.len();
    assert!(app.open_path(&path).is_err());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(app.editor.history_view().ticket, ticket);
    assert_eq!(app.editor.preview_stamp(), stamp);
    assert_eq!(app.editor.canvas_text_content(), Some(text.as_str()));
    assert_eq!(app.bound, bound);
    assert_eq!(app.tabs.len(), tabs);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
#[test]
fn native_save_reopen_preserves_pcnd8_mask_filter_order_and_raw_source() {
    let folder = Folder::new();
    for mode in [Mode::Multiply, Mode::Screen] {
        let doc = source(mode, 2, 1, true);
        let mut app = App::new(None);
        app.install(Editor::with_document(doc.clone(), None))
            .unwrap();
        let before = app.editor.document_snapshot();
        let path = folder.0.join(format!("saved-{mode:?}.pcnd"));
        save_native(&mut app, path.clone());
        assert_eq!(app.bound.as_ref(), Some(&path));
        assert!(!app.editor.is_dirty());
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            paintcraft_formats::pcnd::probe(&bytes)
                .unwrap()
                .container_version,
            8
        );
        let reopened = Editor::open(&path).unwrap();
        assert!(reopened.document().same_semantics(&doc));
        assert!(app.editor.document().same_semantics(&before));
        assert_eq!(app.editor.undo_depth(), 0);
        assert_eq!(reopened.undo_depth(), 0);
    }
}
#[test]
fn old_clipboard_and_foreign_filter_bake_still_refuse_after_native_view_adoption() {
    for mode in [Mode::Multiply, Mode::Screen] {
        let doc = source(mode, 2, 1, false);
        assert_eq!(
            paintcraft_render::clipboard::capture_visible(
                &doc,
                &paintcraft_raster::selection::Selection::Inactive,
                &SharedBudget::new(8192),
                &|| Ok(()),
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            paintcraft_formats::export::filter_bake::inspect(&doc, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
#[path = "blend_adoption_tests/display.rs"]
mod display_tests;
#[path = "blend_adoption_tests/transactions.rs"]
mod transactions;
