//! NonNormal sources stay gated until the actual browser Source8 lane exists.
use super::*;
use paintcraft_commands::{LayerMaskOp, LayerRef};
use paintcraft_input::PointerButton;
use paintcraft_tools::LayerMaskEdit;
use std::sync::Arc;
#[test]
fn non_normal_layer_mask_refuses_before_gesture_cancel_or_any_session_mutation() {
    let mut e = Editor::new(2, 2).unwrap();
    let id = e.target.unwrap();
    let mut doc = (*e.session.snapshot()).clone();
    doc.layers.get_mut(&id).unwrap().blend_mode = paintcraft_core::types::LayerBlendMode::Multiply;
    e.session = paintcraft_editor::session::Session::new(doc);
    e.tools.tool = paintcraft_tools::Tool::Pencil;
    e.tools
        .pointer_down_subpixel(id, (256, 256), PointerButton::Primary);
    assert!(e.tools.gesture_in_progress());
    let before = e.session.snapshot();
    let ticket = e.session.ticket();
    let selection = e.session.selection();
    let depth = e.session.undo_depth();
    let edit = LayerMaskEdit {
        owner: e.session.owner_id(),
        state_id: ticket.state_id,
        publication_epoch: ticket.publication_epoch,
        operation: LayerMaskOp::Remove {
            layer: LayerRef::Id(id),
        },
    };
    let error = e.apply(Action::LayerMask(edit)).unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert_eq!(
        error.message,
        "Layer masks with layer blends are not available in the browser yet"
    );
    assert!(e.tools.gesture_in_progress());
    assert!(Arc::ptr_eq(&before, &e.session.snapshot()));
    assert_eq!(e.session.ticket(), ticket);
    assert_eq!(e.session.selection(), selection);
    assert_eq!(e.session.undo_depth(), depth);
    assert_eq!(e.target, Some(id));
}
