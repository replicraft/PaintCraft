use super::*;
use std::sync::Arc;
#[path = "../../../../../crates/services/paintcraft-formats/tests/support/metadata_limit.rs"]
pub(crate) mod fixture;

struct Before {
    document: Arc<Document>,
    ticket: paintcraft_editor::session::Ticket,
    history: paintcraft_editor::session::HistoryView,
    selection: execute::Selection,
    target: Option<Uuid>,
    dirty: bool,
}
impl Before {
    fn capture(editor: &Editor) -> Self {
        Self {
            document: editor.session.snapshot(),
            ticket: editor.session.ticket(),
            history: editor.session.history_view(),
            selection: editor.session.selection(),
            target: editor.target,
            dirty: editor.session.is_dirty(),
        }
    }
    fn unchanged(&self, editor: &Editor) {
        assert!(Arc::ptr_eq(&self.document, &editor.session.snapshot()));
        assert_eq!(self.ticket, editor.session.ticket());
        assert_eq!(self.history, editor.session.history_view());
        assert_eq!(self.selection, editor.session.selection());
        assert_eq!(self.target, editor.target);
        assert_eq!(self.dirty, editor.session.is_dirty());
    }
}
fn add_layer() -> Value {
    Value::object([
        ("op", Value::str("layer.add")),
        ("ref", Value::str("overflow")),
        ("name", Value::str("Overflow")),
        ("width", Value::Int(64)),
        ("height", Value::Int(64)),
        ("origin", point((0, 0))),
        ("default_pixel", Value::str("#00000000")),
    ])
}
fn paint(layer: Uuid, color: &str) -> Value {
    Value::object([
        ("op", Value::str("raster.fill_rect")),
        ("layer_ref", layer_ref(layer)),
        (
            "rect",
            Value::object([
                ("x", Value::Int(1)),
                ("y", Value::Int(1)),
                ("width", Value::Int(1)),
                ("height", Value::Int(1)),
            ]),
        ),
        ("color", Value::str(color)),
        ("mode", Value::str("replace")),
    ])
}
fn rename(layer: Uuid, name: &str) -> Value {
    Value::object([
        ("op", Value::str("layer.set_properties")),
        ("layer_ref", layer_ref(layer)),
        ("name", Value::str(name)),
    ])
}

#[test]
fn actual_capacity_rejects_layer_and_paint_growth_without_owner_publication() {
    for with_layer in [false, true] {
        let mut editor = Editor::from_document(fixture::exact_doc(64, with_layer)).unwrap();
        let before = Before::capture(&editor);
        assert!(!editor.is_dirty());
        assert!(editor.take_publication_witness().is_none());
        let bytes = paintcraft_formats::pcnd::write_compact(editor.session.document()).unwrap();
        assert_eq!(
            editor.commit(vec![add_layer()]).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        before.unchanged(&editor);
        assert!(editor.take_publication_witness().is_none());
        if let Some(layer) = editor.target {
            assert_eq!(
                editor
                    .commit(vec![paint(layer, "#ff0000ff")])
                    .unwrap_err()
                    .code,
                ErrorCode::ResourceLimit
            );
            before.unchanged(&editor);
            assert!(editor.take_publication_witness().is_none());
            assert_eq!(
                editor.session.document().layers[&layer]
                    .raster
                    .resident_tiles(),
                0
            );
        }
        assert_eq!(
            bytes,
            paintcraft_formats::pcnd::write_compact(editor.session.document()).unwrap()
        );
        assert!(
            paintcraft_formats::pcnd::read(&bytes)
                .unwrap()
                .same_semantics(editor.session.document())
        );
    }
}

#[test]
fn same_size_edit_noop_selection_and_draft_free_navigation_remain_available() {
    let initial = fixture::exact_doc(64, true);
    let source_id = initial.state_id;
    let mut editor = Editor::from_document(initial).unwrap();
    assert_eq!(
        editor.session.document().state_id,
        source_id,
        "factory does not rewrite initial source"
    );
    let layer = editor.target.unwrap();
    let original = editor.session.snapshot();
    editor.commit(vec![paint(layer, "#ffffffff")]).unwrap();
    assert_eq!(editor.session.ticket().state_id, source_id);
    assert_eq!(editor.session.undo_depth(), 0);
    editor
        .apply(Action::Select {
            corners: Some(((0, 0), (4, 4))),
        })
        .unwrap();
    assert!(!editor.session.is_dirty());
    let replacement = "y".repeat(editor.session.document().layers[&layer].name.len());
    editor.commit(vec![rename(layer, &replacement)]).unwrap();
    assert!(editor.session.is_dirty());
    let changed = editor.session.snapshot();
    assert_ne!(changed.state_id, source_id);
    let bytes = paintcraft_formats::pcnd::write_compact(&changed).unwrap();
    assert_eq!(
        bytes.len(),
        paintcraft_document::source_metadata::BYTES_MAX as usize + 32
    );
    assert!(
        paintcraft_formats::pcnd::read(&bytes)
            .unwrap()
            .same_semantics(&changed)
    );
    editor.apply(Action::Undo).unwrap();
    assert!(editor.session.document().same_semantics(&original));
    assert!(!editor.is_dirty());
    editor.apply(Action::Redo).unwrap();
    assert!(editor.session.document().same_semantics(&changed));
    assert!(editor.is_dirty());
}

#[test]
fn refused_text_keeps_editable_ink_and_private_omission_copy_without_mutating_authoring() {
    use paintcraft_formats::export::{
        self,
        metadata_policy::{Intent, Policy, Target},
    };
    use paintcraft_formats::{
        image::ImageFormat,
        registry::{Family, Operation},
    };
    let mut editor = Editor::from_document(fixture::exact_doc(64, true)).unwrap();
    let before = Before::capture(&editor);
    let layer = editor.target.unwrap();
    editor.tools.color = Rgba([0, 0, 0, 255]);
    editor.tools.begin_text(layer, (4, 4));
    editor.tools.type_text("Ink");
    let draft = editor.tools.preview_revision();
    let style = editor.tools.text_style.clone();
    let copy = editor.copy_snapshot().unwrap();
    let pixels = paintcraft_render::render_flat(&copy).unwrap();
    assert!(
        pixels.as_chunks::<4>().0.iter().any(|p| p[..3] != [255; 3]),
        "visible text truly changes pixels"
    );
    assert_ne!(copy.state_id, before.document.state_id);
    assert_eq!(
        paintcraft_formats::pcnd::check_compact_metadata_admission(&copy, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        editor.place_text().unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    before.unchanged(&editor);
    assert_eq!(editor.tools.text_content(), Some("Ink"));
    assert_eq!(editor.tools.preview_revision(), draft);
    assert_eq!(editor.tools.text_style, style);
    assert!(editor.is_dirty());
    let policy = Policy::capture(
        &copy,
        Target::new(Family::Png, Operation::ExportFlat),
        Intent::Omit,
    )
    .unwrap();
    let mut options = export::FlatOptions::new(ImageFormat::Png, 90);
    options.source_metadata = policy;
    let result = export::export_flat_options(&copy, options, &[]).unwrap();
    assert_eq!(result.losses, [export::metadata_policy::OMITTED_REASON]);
    assert_eq!(
        paintcraft_formats::image::decode(&result.bytes)
            .unwrap()
            .rgba,
        pixels
    );
    assert!(copy.source_metadata.is_some());
    before.unchanged(&editor);
    assert_eq!(editor.tools.text_content(), Some("Ink"));
    editor.tools.cancel_text();
    assert!(!editor.is_dirty());
    before.unchanged(&editor);
}
