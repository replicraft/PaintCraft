//! Flatten means the visible projection even when only one rich layer exists.
use paintcraft_core::types::{
    Error, ErrorCode, Result, Rgba,
    filters::{BoxBlurV1, Descriptor},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterStack, POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_raster::{Admission, RasterLayer};
use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};

fn render(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    let budget = SharedBudget::new(1 << 20);
    with_workspace(
        WorkspaceAdmission::new(
            budget.scope(AllocationClass::SharedBudget),
            1 << 20,
            u64::MAX,
        ),
        || paintcraft_render::render_cancellable(doc, check),
    )
}
fn boxed(enabled: bool) -> Document {
    let mut doc = new_document(2, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.raster = RasterLayer::from_rgba(
        2,
        1,
        Rgba::TRANSPARENT,
        &[255, 0, 0, 255, 0, 255, 0, 255],
        &mut Admission::new(1 << 20),
    )
    .unwrap();
    layer.filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["flatten-box"]),
        enabled,
        Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
    )])
    .unwrap();
    doc
}

#[test]
fn single_live_and_disabled_intentions_bake_with_one_undo_and_result_downgrade() {
    for enabled in [true, false] {
        let doc = boxed(enabled);
        let before = doc.clone();
        let mut session = Session::new(doc);
        let before_ticket = session.ticket();
        let receipt = session.flatten(render).unwrap();
        assert!(receipt.committed);
        assert!(receipt.document_features.is_empty());
        assert_eq!(session.document().roots.len(), 1);
        let layer = &session.document().layers[&session.document().roots[0]];
        assert!(layer.filters.is_empty());
        assert_eq!(layer.origin, (0, 0));
        assert_eq!(layer.opacity, 255);
        let expected = if enabled {
            vec![170, 85, 0, 255, 85, 170, 0, 255]
        } else {
            vec![255, 0, 0, 255, 0, 255, 0, 255]
        };
        assert_eq!(render(session.document(), &|| Ok(())).unwrap(), expected);
        assert_eq!(session.undo_depth(), 1);
        session.undo().unwrap();
        assert!(session.document().same_semantics(&before));
        assert_eq!(session.document().state_id, before_ticket.state_id);
        session.redo().unwrap();
        assert!(
            session
                .document()
                .layers
                .values()
                .all(|layer| layer.filters.is_empty())
        );
        assert_eq!(render(session.document(), &|| Ok(())).unwrap(), expected);
    }
}

#[test]
fn lone_hidden_offset_partial_or_translucent_layers_are_not_already_flat() {
    for (visible, opacity, origin, width, expected) in [
        (false, 255, (0, 0), 2, vec![0; 8]),
        (true, 128, (0, 0), 2, vec![255, 0, 0, 128, 255, 0, 0, 128]),
        (true, 255, (1, 0), 2, vec![0, 0, 0, 0, 255, 0, 0, 255]),
        (true, 255, (0, 0), 1, vec![255, 0, 0, 255, 0, 0, 0, 0]),
    ] {
        let mut doc = new_document(2, 1, Some(Rgba([255, 0, 0, 255])), 0, POLICY).unwrap();
        let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
        layer.visible = visible;
        layer.opacity = opacity;
        layer.origin = origin;
        layer.raster = RasterLayer::new(width, 1, Rgba([255, 0, 0, 255]));
        let before = doc.clone();
        let mut session = Session::new(doc);
        assert!(session.flatten(render).unwrap().committed);
        assert_eq!(
            paintcraft_render::render_flat(session.document()).unwrap(),
            expected
        );
        session.undo().unwrap();
        assert!(session.document().same_semantics(&before));
    }
}

#[test]
fn failed_single_layer_bake_preserves_source_history_and_ticket() {
    let mut session = Session::new(boxed(true));
    let before = session.snapshot();
    let ticket = session.ticket();
    let error = session
        .flatten(|_, _| Err(Error::new(ErrorCode::OperationCancelled, "test cancel")))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(session.ticket(), ticket);
    assert_eq!(session.undo_depth(), 0);
    assert!(session.document().same_semantics(&before));
}
