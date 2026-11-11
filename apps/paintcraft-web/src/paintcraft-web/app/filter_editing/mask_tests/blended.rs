//! Existing committed filter-mask route; numeric private previews remain separate.
use super::*;
use paintcraft_commands::{FilterOp, FilterRef, LayerRef, MaskOp};
use paintcraft_core::types::{
    LayerBlendMode, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
const MODES: [LayerBlendMode; 3] = [
    LayerBlendMode::Normal,
    LayerBlendMode::Multiply,
    LayerBlendMode::Screen,
];
fn scene(mode: LayerBlendMode) -> (egui::Context, WebApp, Uuid) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    app.editor = Editor::new(2, 1).unwrap();
    let id = Uuid::from_u128(0x9698);
    let mut doc = (*app.editor.session.snapshot()).clone();
    let bottom = doc.roots[0];
    let layer = doc.layers.get_mut(&bottom).unwrap();
    layer.raster = paintcraft_raster::RasterLayer::new(2, 1, Rgba([31, 127, 241, 128]));
    let mut top = layer.clone();
    layer.filters = FilterStack::new(vec![]).unwrap();
    top.id = Uuid::from_u128(0x9697);
    top.filters =
        FilterStack::new(vec![FilterInstance::new(id, true, Descriptor::InvertRgbV1)]).unwrap();
    top.blend_mode = mode;
    top.raster = paintcraft_raster::RasterLayer::new(2, 1, Rgba([204, 77, 13, 128]));
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);
    app.editor = Editor::from_source8_document(doc).unwrap();
    app.canvas.filter_view = None;
    selection(&mut app, Rect::new(1, 0, 1, 1).unwrap());
    (ctx, app, id)
}
fn capture(app: &WebApp, id: Uuid) -> FilterEdit {
    let ticket = app.editor.session.ticket();
    FilterEdit {
        owner: app.editor.session.owner_id(),
        state_id: ticket.state_id,
        publication_epoch: ticket.publication_epoch,
        operation: state::operation(app, Intent::UseSelection(id)).unwrap(),
    }
}
fn pair(a: [u8; 4], b: [u8; 4]) -> Vec<u8> {
    [a.as_slice(), b.as_slice()].concat()
}
fn expected(mode: LayerBlendMode) -> ([u8; 4], [u8; 4]) {
    // Frozen rational alpha/blend oracle, independent of product renderer.
    match mode {
        LayerBlendMode::Normal => ([146, 94, 89, 192], [44, 161, 242, 192]),
        LayerBlendMode::Multiply => ([86, 81, 89, 192], [29, 131, 237, 192]),
        LayerBlendMode::Screen => ([149, 123, 165, 192], [53, 174, 246, 192]),
    }
}
fn read(bytes: &[u8]) -> paintcraft_document::Document {
    use paintcraft_formats::pcnd::{self, CoverageAdmission, MetadataAdmission, ReadAdmission};
    let original = SharedBudget::new(1 << 20);
    pcnd::read_with_admission_checked(
        bytes,
        8,
        65536,
        2,
        ReadAdmission {
            metadata: MetadataAdmission {
                bytes: 0,
                budget: &original,
            },
            coverage: CoverageAdmission {
                bytes: 1 << 20,
                budget: &original,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn real_studio_mask_intents_all_modes_pixels_history_noop_and_saved_fields() {
    let budget = SharedBudget::new(1 << 20);
    let original = WorkspaceAdmission::new(
        budget
            .reserve_capacity(1 << 20)
            .unwrap()
            .scope(AllocationClass::SharedBudget),
        1 << 20,
        268_435_456,
    );
    for mode in MODES {
        let (ctx, mut app, id) = scene(mode);
        let target = app.editor.target;
        let before = app.editor.session.snapshot();
        let depth = app.editor.session.undo_depth();
        let (raw, filtered) = expected(mode);
        let pixels = |app: &WebApp| {
            with_workspace(original.clone(), || {
                paintcraft_render::render_flat(app.editor.session.document())
            })
            .unwrap()
        };
        assert_eq!(pixels(&app), pair(filtered, filtered));
        app.begin_studio_filter(&ctx, Intent::UseSelection(id));
        assert!(app.filter_editing.is_none());
        assert_eq!(app.editor.session.undo_depth(), depth + 1);
        assert_eq!(pixels(&app), pair(raw, filtered));
        let selected = app.editor.session.snapshot();
        assert_eq!(mask(&app).coverage().coverage(0, 0), 0);
        assert_eq!(mask(&app).coverage().coverage(1, 0), 255);
        let ticket = app.editor.session.ticket();
        app.begin_studio_filter(&ctx, Intent::UseSelection(id));
        assert!(Arc::ptr_eq(&selected, &app.editor.session.snapshot()));
        assert_eq!(app.editor.session.ticket(), ticket);
        assert_eq!(app.editor.session.undo_depth(), depth + 1);
        let wire = paintcraft_formats::pcnd::write_compact(app.editor.session.document()).unwrap();
        assert_eq!(wire[4], if mode == LayerBlendMode::Normal { 4 } else { 8 });
        assert!(selected.same_semantics(&read(&wire)));
        app.editor.apply(Edit::Undo).unwrap();
        assert!(before.same_semantics(app.editor.session.document()));
        app.editor.apply(Edit::Redo).unwrap();
        assert!(selected.same_semantics(app.editor.session.document()));
        app.begin_studio_filter(&ctx, Intent::SetMaskEnabled { id, enabled: false });
        assert!(!mask(&app).enabled());
        assert_eq!(
            mask(&app).coverage(),
            selected.layers[&target.unwrap()].filters.as_slice()[0]
                .mask()
                .unwrap()
                .coverage()
        );
        assert_eq!(pixels(&app), pair(filtered, filtered));
        let disabled = app.editor.session.snapshot();
        let wire = paintcraft_formats::pcnd::write_compact(&disabled).unwrap();
        assert!(disabled.same_semantics(&read(&wire)));
        app.begin_studio_filter(&ctx, Intent::RemoveMask(id));
        assert!(
            app.editor.session.document().layers[&target.unwrap()]
                .filters
                .as_slice()[0]
                .mask()
                .is_none()
        );
        assert_eq!(pixels(&app), pair(filtered, filtered));
        app.editor.apply(Edit::Undo).unwrap();
        assert!(disabled.same_semantics(app.editor.session.document()));
        app.editor.apply(Edit::Redo).unwrap();
        assert_eq!(
            app.editor.session.document().layers[&target.unwrap()].raster,
            before.layers[&target.unwrap()].raster
        );
        assert_eq!(app.editor.target, target);
        if mode != LayerBlendMode::Normal {
            assert_eq!(
                snapshot::encode(app.editor.session.document(), &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
        }
    }
    assert!(original.work_used() > 0);
    assert_eq!(budget.usage().reserved, 1 << 20);
    drop(original);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn captured_mask_stale_owner_state_epoch_and_unknown_references_refuse() {
    for mode in MODES {
        let (_, mut app, id) = scene(mode);
        let valid = capture(&app, id);
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        for kind in 0..5 {
            let mut edit = valid.clone();
            match kind {
                0 => edit.owner = Uuid::new_v4(),
                1 => edit.state_id = Uuid::new_v4(),
                2 => edit.publication_epoch += 1,
                _ => {
                    edit.operation = FilterOp::Mask {
                        layer: LayerRef::Id(if kind == 3 {
                            Uuid::new_v4()
                        } else {
                            app.editor.target.unwrap()
                        }),
                        filter: FilterRef::Id(if kind == 4 { Uuid::new_v4() } else { id }),
                        operation: MaskOp::FromSelection { enabled: true },
                    }
                }
            }
            let error = app.editor.apply(Edit::Filter(edit)).unwrap_err();
            assert_eq!(
                error.code,
                if kind < 3 {
                    ErrorCode::ConflictStaleBase
                } else {
                    ErrorCode::ReferenceUnknownId
                }
            );
            assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
            assert_eq!(app.editor.session.ticket(), ticket);
        }
        selection(&mut app, Rect::new(0, 0, 1, 1).unwrap());
        let selected = app.editor.session.snapshot();
        let now = app.editor.session.ticket();
        assert_eq!(now.state_id, ticket.state_id);
        assert_ne!(now.publication_epoch, ticket.publication_epoch);
        assert_eq!(
            app.editor.apply(Edit::Filter(valid)).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        assert!(Arc::ptr_eq(&selected, &app.editor.session.snapshot()));
        assert_eq!(app.editor.session.ticket(), now);
    }
}
#[test]
fn private_input_and_original_admission_refusal_keep_mask_capture_retryable() {
    fn refuse(_: &paintcraft_document::Document, _: &dyn Fn() -> Result<()>) -> Result<()> {
        Err(Error::new(
            ErrorCode::ResourceLimit,
            "filter mask original admission sentinel",
        ))
    }
    for mode in MODES {
        let (ctx, mut app, id) = scene(mode);
        let request = capture(&app, id);
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        let depth = app.editor.session.undo_depth();
        let selected = app.editor.session.selection();
        app.editor.session.set_persistent_validator(Some(refuse));
        let error = app.editor.apply(Edit::Filter(request.clone())).unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(error.message, "filter mask original admission sentinel");
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
        assert_eq!(app.editor.session.ticket(), ticket);
        app.editor.session.set_persistent_validator(Some(
            paintcraft_formats::pcnd::check_compact_metadata_admission,
        ));
        app.editor
            .tools
            .begin_text(app.editor.target.unwrap(), (0, 0));
        app.editor.tools.type_text("retain private text");
        let stamp = app.editor.tools.text_stamp();
        app.begin_studio_filter(&ctx, Intent::UseSelection(id));
        assert_eq!(app.editor.tools.text_stamp(), stamp);
        assert_eq!(
            app.editor
                .apply(Edit::Filter(request.clone()))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
        app.editor.tools.cancel_text();
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
        assert_eq!(app.editor.session.ticket(), ticket);
        assert_eq!(app.editor.session.undo_depth(), depth);
        assert_eq!(app.editor.session.selection(), selected);
        app.editor.apply(Edit::Filter(request)).unwrap();
        assert_eq!(app.editor.session.undo_depth(), depth + 1);
        assert!(mask(&app).enabled());
    }
}

#[test]
fn mask_intents_preserve_gesture_and_floating_private_owners() {
    use paintcraft_input::PointerButton;
    for mode in MODES {
        for floating in [false, true] {
            let (ctx, mut app, id) = scene(mode);
            let target = app.editor.target.unwrap();
            let before = app.editor.session.snapshot();
            let ticket = app.editor.session.ticket();
            let request = capture(&app, id);
            if floating {
                app.editor.tools.tool = paintcraft_tools::Tool::Select;
                assert!(
                    app.editor
                        .begin_floating(
                            Rect::new(1, 0, 1, 1).unwrap(),
                            (1, 0),
                            PointerButton::Primary
                        )
                        .unwrap()
                );
            } else {
                app.editor.tools.tool = paintcraft_tools::Tool::Pencil;
                app.editor
                    .tools
                    .pointer_down_subpixel(target, (256, 0), PointerButton::Primary);
                assert!(app.editor.tools.gesture_in_progress());
            }
            app.begin_studio_filter(&ctx, Intent::UseSelection(id));
            assert_eq!(
                app.editor.apply(Edit::Filter(request)).unwrap_err().code,
                ErrorCode::ConflictStaleBase
            );
            assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
            assert_eq!(app.editor.session.ticket(), ticket);
            if floating {
                assert!(app.editor.tools.floating_selection().is_some());
                assert_eq!(
                    app.editor.display_selection().unwrap(),
                    app.editor.session.selection()
                );
            } else {
                assert!(app.editor.tools.gesture_in_progress());
            }
        }
    }
}
