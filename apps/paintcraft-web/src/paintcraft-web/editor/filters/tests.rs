use super::*;
use paintcraft_commands::FilterOp;
use paintcraft_core::types::{Rect, filters::Descriptor};
use paintcraft_input::PointerButton;
use paintcraft_tools::Tool;
use std::sync::Arc;

fn edit(editor: &Editor, operation: FilterOp) -> FilterEdit {
    let ticket = editor.session.ticket();
    FilterEdit {
        owner: editor.session.owner_id(),
        state_id: ticket.state_id,
        publication_epoch: ticket.publication_epoch,
        operation,
    }
}
fn add(editor: &Editor) -> FilterEdit {
    edit(
        editor,
        FilterOp::Add {
            layer: LayerRef::Id(editor.target.unwrap()),
            alias: "effect".into(),
            descriptor: Descriptor::InvertRgbV1,
            enabled: true,
            to_index: None,
        },
    )
}
fn refused(editor: &mut Editor, request: FilterEdit) {
    let source = editor.session.snapshot();
    let ticket = editor.session.ticket();
    let depth = editor.session.undo_depth();
    let revision = editor.tools.preview_revision();
    assert!(
        editor
            .display_preview(Some(Preview::Filter(request.clone())))
            .is_err()
    );
    assert!(editor.apply(Action::Filter(request)).is_err());
    assert!(Arc::ptr_eq(&source, &editor.session.snapshot()));
    assert_eq!(editor.session.ticket(), ticket);
    assert_eq!(editor.session.undo_depth(), depth);
    assert_eq!(editor.tools.preview_revision(), revision);
}

#[test]
fn private_preview_matches_single_atomic_commit_and_undo() {
    let mut editor = Editor::new(2, 1).unwrap();
    let request = add(&editor);
    let source = editor.session.snapshot();
    let ticket = editor.session.ticket();
    let preview = editor
        .display_preview(Some(Preview::Filter(request.clone())))
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&source, &editor.session.snapshot()));
    assert_eq!(editor.session.ticket(), ticket);
    assert_eq!(editor.session.undo_depth(), 0);
    let pixels = paintcraft_render::render_flat(&preview).unwrap();
    assert_eq!(pixels, vec![0, 0, 0, 255, 0, 0, 0, 255]);
    editor.apply(Action::Filter(request)).unwrap();
    assert_eq!(
        paintcraft_render::render_flat(editor.session.document()).unwrap(),
        pixels
    );
    assert_eq!(editor.session.undo_depth(), 1);
    assert_eq!(
        editor.session.document().layers[&editor.target.unwrap()]
            .filters
            .len(),
        1
    );
    editor.apply(Action::Undo).unwrap();
    assert!(editor.session.document().same_semantics(&source));
}

#[test]
fn owner_state_and_epoch_are_independently_checked() {
    let mut editor = Editor::new(2, 1).unwrap();
    let request = add(&editor);
    let mut foreign = request.clone();
    foreign.owner = Uuid::new_v4();
    refused(&mut editor, foreign);
    let mut stale = request.clone();
    stale.state_id = Uuid::new_v4();
    refused(&mut editor, stale);
    let mut epoch = request;
    epoch.publication_epoch += 1;
    refused(&mut editor, epoch);
}

#[test]
fn incoming_aliases_and_missing_ids_fail_without_publication() {
    let mut editor = Editor::new(2, 1).unwrap();
    for layer in [
        LayerRef::Alias("layer".into()),
        LayerRef::Id(Uuid::new_v4()),
    ] {
        let mut request = add(&editor);
        if let FilterOp::Add { layer: value, .. } = &mut request.operation {
            *value = layer;
        }
        refused(&mut editor, request);
    }
    for filter in [
        FilterRef::Alias("effect".into()),
        FilterRef::Id(Uuid::new_v4()),
    ] {
        let request = edit(
            &editor,
            FilterOp::Remove {
                layer: LayerRef::Id(editor.target.unwrap()),
                filter,
            },
        );
        refused(&mut editor, request);
    }
}

#[test]
fn text_and_floating_drafts_survive_refused_filter_actions() {
    let mut editor = Editor::new(32, 24).unwrap();
    editor.tools.begin_text(editor.target.unwrap(), (1, 1));
    let empty = add(&editor);
    refused(&mut editor, empty);
    assert!(editor.tools.is_editing_text());
    editor.tools.type_text("retained");
    let stamp = editor.tools.text_stamp();
    let request = add(&editor);
    refused(&mut editor, request);
    assert_eq!(editor.tools.text_stamp(), stamp);
    assert!(editor.tools.is_editing_text());
    editor.tools.cancel_text();
    editor
        .apply(Action::Select {
            corners: Some(((0, 0), (1, 1))),
        })
        .unwrap();
    editor.tools.tool = Tool::Select;
    assert!(
        editor
            .begin_floating(
                Rect::new(0, 0, 2, 2).unwrap(),
                (0, 0),
                PointerButton::Primary
            )
            .unwrap()
    );
    let request = add(&editor);
    refused(&mut editor, request);
    assert!(editor.floating_source.is_some());
    assert!(editor.tools.floating_selection().is_some());
}

#[test]
fn existing_filter_noop_has_no_history_and_real_update_is_atomic() {
    let mut editor = Editor::new(2, 1).unwrap();
    let request = add(&editor);
    editor.apply(Action::Filter(request)).unwrap();
    let id = editor.session.document().layers[&editor.target.unwrap()]
        .filters
        .iter()
        .next()
        .unwrap()
        .id();
    let ticket = editor.session.ticket();
    let depth = editor.session.undo_depth();
    let noop = edit(
        &editor,
        FilterOp::SetEnabled {
            layer: LayerRef::Id(editor.target.unwrap()),
            filter: FilterRef::Id(id),
            enabled: true,
        },
    );
    editor.apply(Action::Filter(noop)).unwrap();
    assert_eq!(editor.session.ticket(), ticket);
    assert_eq!(editor.session.undo_depth(), depth);
    let update = edit(
        &editor,
        FilterOp::SetEnabled {
            layer: LayerRef::Id(editor.target.unwrap()),
            filter: FilterRef::Id(id),
            enabled: false,
        },
    );
    editor.apply(Action::Filter(update)).unwrap();
    assert_eq!(editor.session.undo_depth(), depth + 1);
    assert_eq!(
        paintcraft_render::render_flat(editor.session.document()).unwrap(),
        vec![255; 8]
    );
}

#[test]
fn invalid_grammar_and_live_pointer_gesture_are_not_consumed() {
    let mut editor = Editor::new(8, 8).unwrap();
    let mut request = add(&editor);
    if let FilterOp::Add { to_index, .. } = &mut request.operation {
        *to_index = Some(257);
    }
    refused(&mut editor, request);
    editor.tools.tool = Tool::Brush;
    editor.tools.pointer_down(editor.target.unwrap(), (1, 1));
    let request = add(&editor);
    refused(&mut editor, request);
    assert!(editor.tools.gesture_in_progress());
    assert!(editor.tools.pointer_up().is_some());
}

#[test]
fn filter_output_alias_does_not_retarget_the_active_layer() {
    let mut editor = Editor::new(2, 1).unwrap();
    let original = editor.target;
    editor
        .commit(vec![Value::object([
            ("op", Value::str("layer.add")),
            ("ref", Value::str("overlay")),
            ("name", Value::str("Overlay")),
            ("width", Value::Int(2)),
            ("height", Value::Int(1)),
            (
                "origin",
                Value::object([("x", Value::Int(0)), ("y", Value::Int(0))]),
            ),
            ("default_pixel", Value::str("#00000000")),
        ])])
        .unwrap();
    assert_ne!(editor.target, original);
    editor.target = original;
    let request = add(&editor);
    editor.apply(Action::Filter(request)).unwrap();
    assert_eq!(editor.target, original);
}
