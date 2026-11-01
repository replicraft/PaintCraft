//! No implicit source-hash reseeding through a raster coordinate transform.
use paintcraft_commands::{Anchor, Batch, LayerRef, Op};
use paintcraft_core::types::{
    ErrorCode, Rect, Rgba,
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
    orthogonal::OrthogonalTransform,
};
use paintcraft_document::{Document, FilterInstance, FilterStack, POLICY};
use paintcraft_editor::session::Session;
use uuid::Uuid;
fn scene(enabled: bool, amount: i16) -> Document {
    let mut d =
        paintcraft_document::new_document(4, 4, Some(Rgba([64, 16, 128, 255])), 0, POLICY).unwrap();
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(1),
        enabled,
        Descriptor::AddNoiseRgbV1(
            AddNoiseRgbV1::new(amount, 1, true, NoiseDistribution::Uniform).unwrap(),
        ),
    )])
    .unwrap();
    d
}
fn batch(s: &Session, operations: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: s.document().state_id,
        base_epoch: s.ticket().publication_epoch,
        seed: None,
        digest: "8".repeat(64),
        operations,
    }
}
#[test]
fn stored_default_disabled_and_zero_noise_refuse_coordinate_remapping_atomically() {
    for (enabled, amount) in [(true, 10), (false, 10), (true, 0), (false, 0)] {
        let mut s = Session::new(scene(enabled, amount));
        let id = s.document().roots[0];
        let operations = vec![
            vec![Op::CanvasCrop {
                rect: Rect::new(1, 0, 3, 4).unwrap(),
            }],
            vec![Op::CanvasTransform {
                transform: OrthogonalTransform::RotateRight,
            }],
            vec![Op::CanvasTransform {
                transform: OrthogonalTransform::FlipHorizontal,
            }],
            vec![
                Op::SelectionSetRect {
                    rect: Rect::new(0, 0, 2, 2).unwrap(),
                },
                Op::MoveSelection {
                    layer: LayerRef::Id(id),
                    offset: (-1, 0),
                    clear: Rgba::TRANSPARENT,
                    transparent_key: None,
                },
            ],
            vec![
                Op::SelectionSetRect {
                    rect: Rect::new(0, 0, 2, 2).unwrap(),
                },
                Op::TransformSelection {
                    layer: LayerRef::Id(id),
                    transform: OrthogonalTransform::RotateLeft,
                    clear: Rgba::TRANSPARENT,
                },
            ],
        ];
        for ops in operations {
            let before = s.snapshot();
            let ticket = s.ticket();
            let selection = s.selection();
            let entries = s.history_view().entries.len();
            assert_eq!(
                s.apply(&batch(&s, ops)).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
            assert!(s.document().same_semantics(&before));
            assert_eq!(s.document().state_id, before.state_id);
            assert_eq!(s.ticket().publication_epoch, ticket.publication_epoch);
            assert_eq!(s.history_view().entries.len(), entries);
            assert_eq!(s.selection(), selection);
        }
    }
}
#[test]
fn canvas_resize_keeps_raster_local_intent_and_can_undo_redo() {
    let mut s = Session::new(scene(true, 10));
    let before = s.snapshot();
    let id = before.roots[0];
    s.apply(&batch(
        &s,
        vec![Op::CanvasResize {
            width: 6,
            height: 6,
            anchor: Anchor::Center,
        }],
    ))
    .unwrap();
    let after = s.snapshot();
    let l = &s.document().layers[&id];
    assert_eq!(l.origin, (1, 1));
    assert_eq!(l.filters, before.layers[&id].filters);
    assert_eq!((l.raster.width(), l.raster.height()), (4, 4));
    s.undo().unwrap();
    assert!(s.document().same_semantics(&before));
    s.redo().unwrap();
    assert!(s.document().same_semantics(&after));
}
