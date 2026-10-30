//! Mask operations are editor publications, not worker preview payloads.
use super::*;
use paintcraft_core::types::{Rect, filters::Descriptor, json::Value};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_editor::execute::Selection;
use std::sync::Arc;
fn setup_filter() -> (egui::Context, WebApp, Uuid) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    app.editor = Editor::new(8, 4).unwrap();
    app.canvas.filter_view = Some(
        View::new(
            app.editor.session.document(),
            Rect::new(0, 0, 8, 4).unwrap(),
            0,
        )
        .unwrap(),
    );
    let mut doc = (*app.editor.session.snapshot()).clone();
    let id = Uuid::from_u128(91);
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(id, true, Descriptor::InvertRgbV1)]).unwrap();
    app.editor = Editor::from_document(doc).unwrap();
    (ctx, app, id)
}
fn selection(app: &mut WebApp, rect: Rect) {
    app.editor
        .commit(vec![Value::object([
            ("op", Value::str("selection.set_rect")),
            (
                "rect",
                Value::object([
                    ("x", Value::Int(rect.x0)),
                    ("y", Value::Int(rect.y0)),
                    ("width", Value::Int(rect.width())),
                    ("height", Value::Int(rect.height())),
                ]),
            ),
        ])])
        .unwrap();
}
fn mask(app: &WebApp) -> paintcraft_document::FilterMask {
    app.editor.session.document().layers[&app.editor.target.unwrap()]
        .filters
        .as_slice()[0]
        .mask()
        .unwrap()
        .clone()
}
#[test]
fn selection_snapshot_empty_noop_disable_remove_undo_and_pcnd4() {
    let (ctx, mut app, id) = setup_filter();
    let source = app.editor.session.snapshot();
    app.begin_studio_filter(&ctx, Intent::UseSelection(id));
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    selection(&mut app, Rect::new(1, 0, 1, 1).unwrap());
    app.begin_studio_filter(&ctx, Intent::UseSelection(id));
    assert!(app.filter_editing.is_none());
    let captured = mask(&app);
    assert_eq!(captured.coverage().coverage(1, 0), 255);
    assert_eq!(captured.coverage().coverage(0, 0), 0);
    let pixels = paintcraft_render::render_flat(app.editor.session.document()).unwrap();
    assert_eq!(
        &pixels[..12],
        &[255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255]
    );
    let depth = app.editor.session.undo_depth();
    app.begin_studio_filter(&ctx, Intent::UseSelection(id));
    assert_eq!(app.editor.session.undo_depth(), depth);
    selection(&mut app, Rect::new(90, 90, 1, 1).unwrap());
    assert_eq!(app.editor.session.selection(), Selection::Empty);
    app.begin_studio_filter(&ctx, Intent::UseSelection(id));
    assert_eq!(mask(&app).coverage().stats().runs, 0);
    assert!(mask(&app).enabled());
    app.begin_studio_filter(&ctx, Intent::SetMaskEnabled { id, enabled: false });
    assert!(!mask(&app).enabled());
    app.begin_studio_filter(&ctx, Intent::RemoveMask(id));
    assert!(
        app.editor.session.document().layers[&app.editor.target.unwrap()]
            .filters
            .as_slice()[0]
            .mask()
            .is_none()
    );
    app.editor.apply(Edit::Undo).unwrap();
    assert!(!mask(&app).enabled());
    let doc = app.editor.session.document();
    assert_eq!(
        doc.layers[&doc.roots[0]].raster,
        source.layers[&source.roots[0]].raster
    );
    let bytes = paintcraft_formats::pcnd::write_compact(doc).unwrap();
    assert_eq!(bytes[4], 4);
    assert!(doc.same_semantics(&read_project(&bytes)));
}
#[test]
fn mask_refusal_preserves_selection_and_private_text_and_can_retry() {
    fn refuse(_: &paintcraft_document::Document, _: &dyn Fn() -> Result<()>) -> Result<()> {
        Err(Error::new(
            ErrorCode::ResourceLimit,
            "mask admission sentinel",
        ))
    }
    let (ctx, mut app, id) = setup_filter();
    selection(&mut app, Rect::new(0, 0, 1, 1).unwrap());
    let selection = app.editor.session.selection();
    let source = app.editor.session.snapshot();
    let depth = app.editor.session.undo_depth();
    let dirty = app.editor.session.is_dirty();
    app.editor.session.set_persistent_validator(Some(refuse));
    app.begin_studio_filter(&ctx, Intent::UseSelection(id));
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.undo_depth(), depth);
    assert_eq!(app.editor.session.selection(), selection);
    assert_eq!(app.editor.session.is_dirty(), dirty);
    app.editor.session.set_persistent_validator(None);
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (0, 0));
    app.editor.tools.type_text("retain authored text");
    let stamp = app.editor.tools.text_stamp();
    app.begin_studio_filter(&ctx, Intent::UseSelection(id));
    assert_eq!(app.editor.tools.text_stamp(), stamp);
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    app.editor.tools.cancel_text();
    app.begin_studio_filter(&ctx, Intent::UseSelection(id));
    assert_eq!(app.editor.session.undo_depth(), depth + 1);
    assert!(mask(&app).enabled());
}

#[test]
fn nonrectangular_selection_and_descriptor_preview_keep_exact_stored_coverage() {
    let (ctx, mut app, id) = setup_filter();
    let points = [(0, 0), (3, 0), (3, 1), (1, 1), (1, 3), (0, 3)];
    app.editor
        .commit(vec![Value::object([
            ("op", Value::str("selection.set_polygon")),
            (
                "points",
                Value::Array(
                    points
                        .into_iter()
                        .map(|(x, y)| Value::object([("x", Value::Int(x)), ("y", Value::Int(y))]))
                        .collect(),
                ),
            ),
        ])])
        .unwrap();
    app.begin_studio_filter(&ctx, Intent::UseSelection(id));
    let mask = mask(&app);
    for y in 0..4 {
        for x in 0..8 {
            assert_eq!(
                mask.coverage().coverage(x, y),
                if (x < 3 && y == 0) || (x == 0 && y < 3) {
                    255
                } else {
                    0
                }
            );
        }
    }
    app.begin_studio_filter(&ctx, Intent::Edit(id));
    let draft = &mut app.filter_editing.as_mut().unwrap().draft;
    draft.parameters = Parameters::new(filters::Kind::BrightnessContrast);
    draft.parameters.brightness = 40;
    let edit = draft.operation().unwrap();
    let preview = app
        .editor
        .display_preview(Some(paintcraft_tools::Preview::Filter(edit)))
        .unwrap()
        .unwrap();
    assert_eq!(
        preview.layers[&app.editor.target.unwrap()]
            .filters
            .as_slice()[0]
            .mask(),
        Some(&mask)
    );
    let before = app.editor.session.snapshot();
    app.cancel_filter_edit();
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
}

fn read_project(bytes: &[u8]) -> paintcraft_document::Document {
    use paintcraft_formats::pcnd::{self, CoverageAdmission, MetadataAdmission, ReadAdmission};
    let realm = paintcraft_core::types::resources::default_coverage_budget();
    pcnd::read_with_admission_checked(
        bytes,
        32,
        65536,
        1,
        ReadAdmission {
            metadata: MetadataAdmission {
                bytes: 0,
                budget: &realm,
            },
            coverage: CoverageAdmission {
                bytes: 1 << 20,
                budget: &realm,
                limits: paintcraft_raster::coverage_mask::Limits::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}

#[test]
fn selection_only_epoch_stales_mask_capture_before_private_text_can_publish() {
    for private_text in [false, true] {
        let (_, mut app, id) = setup_filter();
        selection(&mut app, Rect::new(0, 0, 1, 1).unwrap());
        let ticket_a = app.editor.session.ticket();
        let captured = FilterEdit {
            owner: app.editor.session.owner_id(),
            state_id: ticket_a.state_id,
            publication_epoch: ticket_a.publication_epoch,
            operation: state::operation(&app, Intent::UseSelection(id)).unwrap(),
        };
        assert!(matches!(
            &captured.operation,
            paintcraft_commands::FilterOp::Mask {
                operation: paintcraft_commands::MaskOp::FromSelection { enabled: true },
                ..
            }
        ));
        selection(&mut app, Rect::new(1, 0, 1, 1).unwrap());
        let ticket_b = app.editor.session.ticket();
        assert_eq!(
            ticket_a.state_id, ticket_b.state_id,
            "selection B must not change document state ID"
        );
        assert_ne!(ticket_a.publication_epoch, ticket_b.publication_epoch);
        let source = app.editor.session.snapshot();
        let selected = app.editor.session.selection();
        let depth = app.editor.session.undo_depth();
        let dirty = app.editor.session.is_dirty();
        if private_text {
            app.editor
                .tools
                .begin_text(app.editor.target.unwrap(), (0, 0));
            app.editor.tools.type_text("keep selection-era text");
        }
        let stamp = app.editor.tools.text_stamp();
        let error = app.editor.apply(Edit::Filter(captured)).unwrap_err();
        assert_eq!(error.code, ErrorCode::ConflictStaleBase);
        assert_eq!(app.editor.session.ticket(), ticket_b);
        assert_eq!(app.editor.session.selection(), selected);
        assert_eq!(app.editor.session.undo_depth(), depth);
        assert_eq!(app.editor.session.is_dirty(), dirty);
        assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
        assert!(
            app.editor.session.document().layers[&app.editor.target.unwrap()]
                .filters
                .as_slice()[0]
                .mask()
                .is_none()
        );
        assert_eq!(app.editor.tools.text_stamp(), stamp);
        assert_eq!(app.editor.tools.is_editing_text(), private_text);
    }
}
