//! Dedicated own-mask activation preserves every other browser consumer boundary.
use super::*;
use paintcraft_commands::{LayerMaskOp, LayerRef};
use paintcraft_core::types::{
    LayerBlendMode, Rect,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_input::PointerButton;
use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
use paintcraft_tools::LayerMaskEdit;
use std::sync::Arc;

const MODES: [LayerBlendMode; 3] = [
    LayerBlendMode::Normal,
    LayerBlendMode::Multiply,
    LayerBlendMode::Screen,
];
fn scene(mode: LayerBlendMode) -> Editor {
    let original = Editor::new(2, 1).unwrap();
    let mut doc = (*original.session.snapshot()).clone();
    let bottom = doc.roots[0];
    let layer = doc.layers.get_mut(&bottom).unwrap();
    layer.raster = paintcraft_raster::RasterLayer::new(2, 1, Rgba([31, 127, 241, 128]));
    let mut top = layer.clone();
    top.id = Uuid::from_u128(0x9192);
    top.name = "Mask subject".into();
    top.blend_mode = mode;
    top.raster = paintcraft_raster::RasterLayer::new(2, 1, Rgba([204, 77, 13, 128]));
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);
    Editor::from_source8_document(doc).unwrap()
}
fn select(editor: &mut Editor, x: i64) {
    editor
        .commit(vec![Value::object([
            ("op", Value::str("selection.set_rect")),
            (
                "rect",
                Value::object([
                    ("x", Value::Int(x)),
                    ("y", Value::Int(0)),
                    ("width", Value::Int(1)),
                    ("height", Value::Int(1)),
                ]),
            ),
        ])])
        .unwrap();
}
fn capture(editor: &Editor, operation: u8) -> LayerMaskEdit {
    let layer = LayerRef::Id(editor.target.unwrap());
    editor.layer_mask_capture(match operation {
        0 => LayerMaskOp::FromSelection {
            layer,
            enabled: true,
        },
        1 => LayerMaskOp::Invert { layer },
        2 => LayerMaskOp::SetEnabled {
            layer,
            enabled: false,
        },
        _ => LayerMaskOp::Remove { layer },
    })
}
fn pixels(editor: &Editor, original: &WorkspaceAdmission) -> Vec<u8> {
    with_workspace(original.clone(), || {
        paintcraft_render::render_flat(editor.session.document())
    })
    .unwrap()
}
fn read(bytes: &[u8]) -> Document {
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
fn pair(left: [u8; 4], right: [u8; 4]) -> Vec<u8> {
    [left.as_slice(), right.as_slice()].concat()
}
fn composite(mode: LayerBlendMode) -> [u8; 4] {
    // Independent exact rational source-over/Multiply/Screen expectations.
    match mode {
        LayerBlendMode::Normal => [146, 94, 89, 192],
        LayerBlendMode::Multiply => [86, 81, 89, 192],
        LayerBlendMode::Screen => [149, 123, 165, 192],
    }
}
#[test]
fn all_modes_own_masks_preserve_real_pixels_history_noop_and_pcnd() {
    // One original declared host/counter for the entire test, not observed grants
    // or a replacement factory on retry. This is the existing1MiB render profile.
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
        let mut editor = scene(mode);
        let target = editor.target.unwrap();
        let raw = editor.session.document().layers[&target].raster.clone();
        let roots = editor.session.document().roots.clone();
        select(&mut editor, 1);
        let baseline = editor.session.snapshot();
        let depth = editor.session.undo_depth();
        let full = pair(composite(mode), composite(mode));
        assert_eq!(pixels(&editor, &original), full);
        assert!(editor.layer_mask_available());
        editor
            .apply(Action::LayerMask(capture(&editor, 0)))
            .unwrap();
        assert_eq!(editor.session.undo_depth(), depth + 1);
        let selected = editor.session.snapshot();
        assert_eq!(
            pixels(&editor, &original),
            pair([31, 127, 241, 128], composite(mode))
        );
        let field = selected.layers[&target].mask.as_ref().unwrap();
        assert_eq!(field.coverage().domain(), Rect::new(0, 0, 2, 1).unwrap());
        assert_eq!(field.coverage().coverage(0, 0), 0);
        assert_eq!(field.coverage().coverage(1, 0), 255);
        let ticket = editor.session.ticket();
        editor
            .apply(Action::LayerMask(capture(&editor, 0)))
            .unwrap();
        assert!(Arc::ptr_eq(&selected, &editor.session.snapshot()));
        assert_eq!(editor.session.ticket(), ticket);
        assert_eq!(editor.session.undo_depth(), depth + 1);
        editor.apply(Action::Undo).unwrap();
        assert!(editor.session.document().same_semantics(&baseline));
        editor.apply(Action::Redo).unwrap();
        assert!(editor.session.document().same_semantics(&selected));
        editor
            .apply(Action::LayerMask(capture(&editor, 1)))
            .unwrap();
        assert_eq!(
            pixels(&editor, &original),
            pair(composite(mode), [31, 127, 241, 128])
        );
        let inverted = editor.session.snapshot();
        editor
            .apply(Action::LayerMask(capture(&editor, 2)))
            .unwrap();
        let disabled = editor.session.snapshot();
        let field = disabled.layers[&target].mask.as_ref().unwrap();
        assert!(!field.enabled());
        assert_eq!(field.coverage().coverage(0, 0), 255);
        assert_eq!(pixels(&editor, &original), full);
        editor.apply(Action::Undo).unwrap();
        assert!(editor.session.document().same_semantics(&inverted));
        editor.apply(Action::Redo).unwrap();
        assert!(editor.session.document().same_semantics(&disabled));
        let wire = paintcraft_formats::pcnd::write_compact(editor.session.document()).unwrap();
        assert_eq!(wire[4], if mode == LayerBlendMode::Normal { 7 } else { 8 });
        assert!(editor.session.document().same_semantics(&read(&wire)));
        editor
            .apply(Action::LayerMask(capture(&editor, 3)))
            .unwrap();
        assert!(editor.session.document().layers[&target].mask.is_none());
        assert_eq!(pixels(&editor, &original), full);
        let wire = paintcraft_formats::pcnd::write_compact(editor.session.document()).unwrap();
        assert_eq!(wire[4], if mode == LayerBlendMode::Normal { 1 } else { 8 });
        assert!(editor.session.document().same_semantics(&read(&wire)));
        editor.apply(Action::Undo).unwrap();
        assert!(editor.session.document().same_semantics(&disabled));
        assert_eq!(editor.session.document().layers[&target].raster, raw);
        assert_eq!(editor.session.document().layers[&target].blend_mode, mode);
        assert_eq!(editor.session.document().roots, roots);
        assert_eq!(editor.target, Some(target));
    }
    assert!(original.work_used() > 0);
    assert_eq!(budget.usage().reserved, 1 << 20);
    drop(original);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn all_modes_stale_owner_epoch_target_gesture_and_text_refuse_before_mutation() {
    for mode in MODES {
        let mut editor = scene(mode);
        select(&mut editor, 1);
        let late = capture(&editor, 0);
        let state = editor.session.ticket().state_id;
        select(&mut editor, 0);
        assert_eq!(editor.session.ticket().state_id, state);
        let before = editor.session.snapshot();
        let ticket = editor.session.ticket();
        let selection = editor.session.selection();
        let depth = editor.session.undo_depth();
        let target = editor.target;
        assert_eq!(
            editor.apply(Action::LayerMask(late)).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        let mut foreign = capture(&editor, 0);
        foreign.owner = Uuid::new_v4();
        assert_eq!(
            editor.apply(Action::LayerMask(foreign)).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        let mut foreign = capture(&editor, 0);
        foreign.state_id = Uuid::new_v4();
        assert_eq!(
            editor.apply(Action::LayerMask(foreign)).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        let edit = capture(&editor, 0);
        editor.target = Some(editor.session.document().roots[0]);
        assert_eq!(
            editor.apply(Action::LayerMask(edit)).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        editor.target = target;
        editor.tools.tool = paintcraft_tools::Tool::Pencil;
        editor
            .tools
            .pointer_down_subpixel(target.unwrap(), (256, 0), PointerButton::Primary);
        assert!(!editor.layer_mask_available());
        assert_eq!(
            editor
                .apply(Action::LayerMask(capture(&editor, 0)))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
        assert!(editor.tools.gesture_in_progress());
        editor.tools.cancel_gesture();
        editor.tools.begin_text(target.unwrap(), (0, 0));
        editor.tools.type_text("retain private text");
        let text = editor.tools.text_stamp();
        assert!(!editor.layer_mask_available());
        assert_eq!(
            editor
                .apply(Action::LayerMask(capture(&editor, 0)))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
        assert_eq!(editor.tools.text_stamp(), text);
        assert!(Arc::ptr_eq(&before, &editor.session.snapshot()));
        assert_eq!(editor.session.ticket(), ticket);
        assert_eq!(editor.session.selection(), selection);
        assert_eq!(editor.session.undo_depth(), depth);
        assert_eq!(editor.target, target);
    }
}
#[test]
fn all_modes_persistent_admission_refusal_can_retry_same_captured_intent() {
    fn refuse(_: &Document, _: &dyn Fn() -> Result<()>) -> Result<()> {
        Err(Error::new(
            ErrorCode::ResourceLimit,
            "own-mask original admission sentinel",
        ))
    }
    for mode in MODES {
        let mut editor = scene(mode);
        select(&mut editor, 1);
        let edit = capture(&editor, 0);
        let before = editor.session.snapshot();
        let ticket = editor.session.ticket();
        let selection = editor.session.selection();
        let depth = editor.session.undo_depth();
        let target = editor.target;
        editor.session.set_persistent_validator(Some(refuse));
        let error = editor.apply(Action::LayerMask(edit.clone())).unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(error.message, "own-mask original admission sentinel");
        assert!(Arc::ptr_eq(&before, &editor.session.snapshot()));
        assert_eq!(editor.session.ticket(), ticket);
        assert_eq!(editor.session.selection(), selection);
        assert_eq!(editor.session.undo_depth(), depth);
        assert_eq!(editor.target, target);
        editor.session.set_persistent_validator(Some(
            paintcraft_formats::pcnd::check_compact_metadata_admission,
        ));
        editor.apply(Action::LayerMask(edit)).unwrap();
        assert_eq!(editor.session.undo_depth(), depth + 1);
        assert!(
            editor.session.document().layers[&target.unwrap()]
                .mask
                .is_some()
        );
    }
}
#[test]
fn ordinary_constructor_stays_normal_only_after_own_mask_activation() {
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        let source = scene(mode).session.snapshot();
        assert_eq!(
            Editor::from_document((*source).clone()).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
