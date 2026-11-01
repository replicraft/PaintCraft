//! Injected raw outcomes test Main transactions, not Worker render/ACK correctness.
use super::*;
use paintcraft_core::types::{
    Rgba,
    filters::{AddNoiseRgbV1, Descriptor},
    resources::SharedBudget,
};
use paintcraft_document::{FilterInstance, FilterStack, source_metadata::*};
use paintcraft_tools::Action;

fn setup() -> (egui::Context, WebApp) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
    let mut doc = Editor::new(2, 1).unwrap().session.document().clone();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(99),
        false,
        Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
    )])
    .unwrap();
    doc.source_metadata = Some(
        SourceMetadata::capture(
            SourceDescriptor {
                format: SourceFormat::Png,
                source_model: SourceColorModel::Rgb,
                source_sha256: [7; 32],
                source_bytes: 123,
                selection: None,
                source_dimensions: (2, 1),
                output_dimensions: (2, 1),
                orientation: None,
                orientation_policy: OrientationPolicy::Unmodified,
                resolution: None,
            },
            None,
            1024,
            &SharedBudget::new(4096),
            &|| Ok(()),
        )
        .unwrap(),
    );
    app.editor = Editor::from_document(doc).unwrap();
    (ctx, app)
}
fn ready(app: &mut WebApp, bytes: Vec<u8>) {
    let job = app.filter_display.copy_action.as_mut().unwrap();
    let account = job.flatten.as_mut().unwrap().account.take().unwrap();
    job.outcome = Some(Rc::new(RefCell::new(Some(Ok(Output::Flattened(
        flatten::Pixels { bytes, account },
    ))))));
}
fn literal() -> Vec<u8> {
    vec![37, 38, 39, 255, 91, 92, 93, 127]
}
#[test]
fn whole_canvas_transaction_preserves_metadata_selection_and_complete_filter_undo() {
    let (ctx, mut app) = setup();
    app.editor
        .commit(vec![Value::object([
            ("op", Value::str("selection.set_rect")),
            (
                "rect",
                Value::object([
                    ("x", Value::Int(1)),
                    ("y", Value::Int(0)),
                    ("width", Value::Int(1)),
                    ("height", Value::Int(1)),
                ]),
            ),
        ])])
        .unwrap();
    let before = app.editor.session.snapshot();
    let selected = app.editor.session.selection();
    let depth = app.editor.session.undo_depth();
    assert!(app.queue_noise_flatten(&Action::FlattenImage));
    let job = app.filter_display.copy_action.as_ref().unwrap();
    assert_eq!(job.region, before.canvas_rect());
    assert_eq!(
        app.clipboard.budget.usage().reserved,
        crate::editor::MAX_PIXELS * 4
    );
    ready(&mut app, literal());
    app.pump_flat_action(&ctx);
    let after = app.editor.session.snapshot();
    assert_eq!(after.layers.len(), 1);
    let layer = &after.layers[&after.roots[0]];
    assert!(layer.filters.is_empty());
    assert_eq!(layer.origin, (0, 0));
    assert_eq!(layer.raster.get(0, 0), Rgba([37, 38, 39, 255]));
    assert_eq!(layer.raster.get(1, 0), Rgba([91, 92, 93, 127]));
    assert_eq!(after.source_metadata, before.source_metadata);
    assert_eq!(app.editor.session.selection(), selected);
    assert_eq!(app.editor.session.undo_depth(), depth + 1);
    assert_eq!(app.clipboard.budget.usage().reserved, 0);
    app.editor.apply(Action::Undo).unwrap();
    assert!(app.editor.session.document().same_semantics(&before));
    assert_eq!(app.editor.session.selection(), selected);
    assert_eq!(before.layers[&before.roots[0]].filters.as_slice().len(), 1);
    assert_eq!(
        before.layers[&before.roots[0]].raster.get(0, 0),
        crate::editor::WHITE
    );
    app.editor.apply(Action::Redo).unwrap();
    assert!(app.editor.session.document().same_semantics(&after));
}
#[test]
fn cancelled_or_foreign_owner_ready_pixels_never_publish_and_release_main_lease() {
    for replace in [false, true] {
        let (ctx, mut app) = setup();
        let before = app.editor.session.snapshot();
        app.queue_noise_flatten(&Action::FlattenImage);
        ready(&mut app, literal());
        if replace {
            app.editor = Editor::from_document((*before).clone()).unwrap();
        } else {
            app.cancel_background_image();
        }
        let ticket = app.editor.session.ticket();
        app.pump_flat_action(&ctx);
        assert_eq!(app.editor.session.ticket(), ticket);
        assert_eq!(app.editor.session.undo_depth(), 0);
        assert_eq!(app.clipboard.budget.usage().reserved, 0);
        assert!(app.editor.session.document().same_semantics(&before));
        assert!(app.filter_display.copy_action.is_none());
    }
}
#[test]
fn malformed_raw_output_aborts_atomic_ingestion_without_history_or_leaked_lease() {
    let (ctx, mut app) = setup();
    let source = app.editor.session.snapshot();
    app.queue_noise_flatten(&Action::FlattenImage);
    ready(&mut app, vec![1; 7]);
    app.pump_flat_action(&ctx);
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert_eq!(app.clipboard.budget.usage().reserved, 0);
    assert!(app.message.starts_with("Edit failed:"));
}
#[test]
fn preadmission_and_private_draft_refusals_never_enter_worker_or_mutate_document() {
    let (_, mut app) = setup();
    let source = app.editor.session.snapshot();
    app.clipboard.budget = SharedBudget::new(1024);
    app.queue_noise_flatten(&Action::FlattenImage);
    assert!(app.filter_display.copy_action.is_none());
    assert_eq!(app.clipboard.budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    app.clipboard.budget = SharedBudget::new(64 << 20);
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (0, 0));
    app.editor.tools.type_text("unplaced");
    app.queue_noise_flatten(&Action::FlattenImage);
    assert!(app.filter_display.copy_action.is_none());
    assert!(app.editor.tools.is_editing_text());
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
}
#[test]
fn later_source_publication_is_rejected_even_with_a_completed_raw_frame() {
    let (ctx, mut app) = setup();
    app.queue_noise_flatten(&Action::FlattenImage);
    ready(&mut app, literal());
    app.editor
        .commit(vec![Value::object([
            ("op", Value::str("selection.set_rect")),
            (
                "rect",
                Value::object([
                    ("x", Value::Int(0)),
                    ("y", Value::Int(0)),
                    ("width", Value::Int(1)),
                    ("height", Value::Int(1)),
                ]),
            ),
        ])])
        .unwrap();
    let ticket = app.editor.session.ticket();
    let selected = app.editor.session.selection();
    app.pump_flat_action(&ctx);
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.selection(), selected);
    assert_eq!(app.clipboard.budget.usage().reserved, 0);
    assert!(
        !app.editor.session.document().layers[&app.editor.target.unwrap()]
            .filters
            .is_empty()
    );
}

#[test]
fn detached_preparation_itself_refuses_foreign_publication_even_without_ui_fence() {
    let (_, mut app) = setup();
    let source = app.editor.session.snapshot();
    app.queue_noise_flatten(&Action::FlattenImage);
    let mut job = app.filter_display.copy_action.take().unwrap();
    let mut transaction = job.flatten.take().unwrap();
    let account = transaction.account.take().unwrap();
    app.editor = Editor::from_document((*source).clone()).unwrap();
    let ticket = app.editor.session.ticket();
    let error = flatten::publish(
        &mut app,
        transaction,
        flatten::Pixels {
            bytes: literal(),
            account,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.code,
        paintcraft_core::types::ErrorCode::ConflictStaleBase
    );
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert_eq!(app.clipboard.budget.usage().reserved, 0);
}
#[test]
fn legacy_plain_flatten_remains_the_existing_noop_route() {
    let (_, mut app) = setup();
    app.editor = Editor::new(2, 1).unwrap();
    let source = app.editor.session.snapshot();
    assert!(!app.queue_noise_flatten(&Action::FlattenImage));
    app.editor.apply(Action::FlattenImage).unwrap();
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert!(app.filter_display.copy_action.is_none());
}
