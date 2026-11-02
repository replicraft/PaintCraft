//! Clearing a pristine source must still discard stored filter intentions.
use paintcraft_commands::{Batch, Op};
use paintcraft_core::types::{
    Rgba,
    filters::{Descriptor, HueSaturationRgbV1},
};
use paintcraft_document::{FilterInstance, FilterStack, POLICY};
use paintcraft_editor::session::Session;
use uuid::Uuid;
const COLOR: Rgba = Rgba([64, 16, 128, 255]);
fn scene(descriptor: Option<(Descriptor, bool)>) -> Session {
    let mut d = paintcraft_document::new_document(2, 1, Some(COLOR), 0, POLICY).unwrap();
    let layer = d.layers.get_mut(&d.roots[0]).unwrap();
    assert_eq!(layer.name, "Background");
    assert_eq!(layer.raster.resident_tiles(), 0);
    if let Some((descriptor, enabled)) = descriptor {
        layer.filters = FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(1),
            enabled,
            descriptor,
        )])
        .unwrap();
    }
    Session::new(d)
}
fn clear(s: &Session) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: s.document().state_id,
        base_epoch: s.ticket().publication_epoch,
        seed: None,
        digest: "c".repeat(64),
        operations: vec![Op::CanvasClear {
            alias: "cleared".into(),
            color: COLOR,
        }],
    }
}
#[test]
fn clear_matching_pristine_source_discards_active_disabled_and_default_filters_with_one_undo() {
    for (descriptor, enabled) in [
        (Descriptor::InvertRgbV1, true),
        (Descriptor::InvertRgbV1, false),
        (
            Descriptor::HueSaturationRgbV1(HueSaturationRgbV1::new(137, 73, -19).unwrap()),
            true,
        ),
        (
            Descriptor::HueSaturationRgbV1(HueSaturationRgbV1::default()),
            false,
        ),
    ] {
        let mut s = scene(Some((descriptor, enabled)));
        let before = s.snapshot();
        let epoch = s.ticket().publication_epoch;
        let entries = s.history_view().entries.len();
        let receipt = s.apply(&clear(&s)).unwrap();
        assert!(receipt.committed);
        assert_ne!(s.document().state_id, before.state_id);
        assert_eq!(s.ticket().publication_epoch, epoch + 1);
        assert_eq!(s.history_view().entries.len(), entries + 1);
        assert_eq!(s.document().roots.len(), 1);
        let after = s.snapshot();
        let layer = &after.layers[&after.roots[0]];
        assert!(layer.filters.is_empty());
        assert_eq!(layer.name, "Background");
        assert_eq!(layer.origin, (0, 0));
        assert_eq!(layer.opacity, 255);
        assert!(layer.visible);
        assert_eq!((layer.raster.width(), layer.raster.height()), (2, 1));
        assert_eq!(layer.raster.resident_tiles(), 0);
        assert_eq!(
            (layer.raster.get(0, 0), layer.raster.get(1, 0)),
            (COLOR, COLOR)
        );
        assert!(s.undo().unwrap().is_some());
        assert!(s.document().same_semantics(&before));
        let original = &s.document().layers[&s.document().roots[0]]
            .filters
            .as_slice()[0];
        assert_eq!(original.descriptor(), descriptor);
        assert_eq!(original.enabled(), enabled);
        assert!(s.redo().unwrap().is_some());
        assert!(s.document().same_semantics(&after));
    }
}
#[test]
fn clear_matching_pristine_source_without_stored_filters_remains_an_exact_noop() {
    let mut s = scene(None);
    let before = s.snapshot();
    let ticket = s.ticket();
    let entries = s.history_view().entries.len();
    let receipt = s.apply(&clear(&s)).unwrap();
    assert!(!receipt.committed);
    assert!(s.document().same_semantics(&before));
    assert_eq!(s.document().state_id, before.state_id);
    assert_eq!(s.ticket().publication_epoch, ticket.publication_epoch);
    assert_eq!(s.history_view().entries.len(), entries);
}

#[test]
fn clear_matching_pristine_source_discards_stored_layer_mask_and_undo_restores_it() {
    use paintcraft_core::types::{Rect, resources::SharedBudget};
    use paintcraft_document::LayerMask;
    use paintcraft_raster::coverage_mask::A8Mask;
    for enabled in [false, true] {
        for coverage in [0, 127, 255] {
            let budget = SharedBudget::new(8192);
            let field = A8Mask::uniform(
                Rect::new(0, 0, 2, 1).unwrap(),
                coverage,
                &budget,
                Default::default(),
                &|| Ok(()),
            )
            .unwrap();
            let mut document =
                paintcraft_document::new_document(2, 1, Some(COLOR), 0, POLICY).unwrap();
            let id = document.roots[0];
            document.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(enabled, field));
            let mut session = Session::new(document);
            let before = session.snapshot();
            let epoch = session.ticket().publication_epoch;
            let entries = session.history_view().entries.len();
            let receipt = session.apply(&clear(&session)).unwrap();
            assert!(receipt.committed);
            assert_eq!(session.ticket().publication_epoch, epoch + 1);
            assert_eq!(session.history_view().entries.len(), entries + 1);
            let after = session.snapshot();
            let layer = &after.layers[&after.roots[0]];
            assert!(layer.mask.is_none());
            assert!(layer.filters.is_empty());
            assert_eq!(layer.raster.get(0, 0), COLOR);
            assert!(session.undo().unwrap().is_some());
            assert!(session.document().same_semantics(&before));
            let restored = session.document().layers[&id].mask.as_ref().unwrap();
            assert_eq!(restored.enabled(), enabled);
            assert_eq!(restored.coverage().coverage(0, 0), coverage);
            assert!(session.redo().unwrap().is_some());
            assert!(session.document().same_semantics(&after));
        }
    }
}

#[test]
fn clear_matching_empty_background_resets_stored_blend_and_undo_restores_it() {
    for mode in [
        paintcraft_core::types::LayerBlendMode::Multiply,
        paintcraft_core::types::LayerBlendMode::Screen,
    ] {
        let mut d = paintcraft_document::new_document(2, 1, Some(COLOR), 0, POLICY).unwrap();
        d.layers.get_mut(&d.roots[0]).unwrap().blend_mode = mode;
        let mut s = Session::new(d);
        let before = s.snapshot();
        assert!(s.apply(&clear(&s)).unwrap().committed);
        let layer = &s.document().layers[&s.document().roots[0]];
        assert_eq!(
            layer.blend_mode,
            paintcraft_core::types::LayerBlendMode::Normal
        );
        assert_eq!(layer.raster.resident_tiles(), 0);
        s.undo().unwrap();
        assert!(s.document().same_semantics(&before));
        s.redo().unwrap();
        assert_eq!(
            s.document().layers[&s.document().roots[0]].blend_mode,
            paintcraft_core::types::LayerBlendMode::Normal
        );
    }
}
