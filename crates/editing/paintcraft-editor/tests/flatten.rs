//! Flatten publication, independent dense composition and exact recorded history.
use paintcraft_commands::{Batch, Op};
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};
use paintcraft_document::{Document, Layer, POLICY, new_document};
use paintcraft_editor::{execute::Selection, session::Session};
use paintcraft_raster::{Admission, RasterLayer};
use std::cell::Cell;
use uuid::Uuid;

fn render(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    paintcraft_render::render_cancellable(doc, check)
}
fn fixture() -> Session {
    let mut doc = new_document(8, 6, Some(Rgba([40, 80, 120, 127])), 0, POLICY).unwrap();
    doc.title = Some("Flatten fixture 日本語".into());
    for (index, (origin, visible, opacity, color)) in [
        ((-2, 1), true, 173, Rgba([210, 35, 80, 99])),
        ((3, -1), true, 237, Rgba([30, 200, 95, 201])),
        ((1, 1), false, 255, Rgba([255, 0, 255, 255])),
    ]
    .into_iter()
    .enumerate()
    {
        let id = Uuid::from_u128(0x100 + index as u128);
        let mut raster = RasterLayer::new(6, 5, color);
        raster
            .blend_pixel(
                2,
                2,
                Rgba([8, 9, 10, 0]),
                paintcraft_commands::Mode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        doc.layers.insert(
            id,
            Layer {
                id,
                name: format!("Layer {index}"),
                origin,
                visible,
                opacity,
                filters: Default::default(),
                raster,
            },
        );
        doc.roots.push(id);
    }
    let mut session = Session::new(doc);
    let ticket = session.ticket();
    session
        .apply(&Batch {
            version: 2,
            policy: POLICY.into(),
            base_state: ticket.state_id,
            base_epoch: ticket.publication_epoch,
            seed: None,
            digest: format!("{:064x}", 1),
            operations: vec![Op::SelectionSetRect {
                rect: Rect::new(1, 1, 3, 2).unwrap(),
            }],
        })
        .unwrap();
    session.mark_saved();
    session
}
fn dense(doc: &Document) -> Vec<u8> {
    let mut pixels = Vec::new();
    for y in 0..doc.canvas.1 {
        for x in 0..doc.canvas.0 {
            let mut dst = [0u8; 4];
            for id in &doc.roots {
                let layer = &doc.layers[id];
                if !layer.visible {
                    continue;
                }
                let p = (i64::from(x) - layer.origin.0, i64::from(y) - layer.origin.1);
                if p.0 < 0
                    || p.1 < 0
                    || p.0 >= i64::from(layer.raster.width())
                    || p.1 >= i64::from(layer.raster.height())
                {
                    continue;
                }
                let src = layer.raster.get(p.0, p.1).0;
                let a = (u64::from(src[3]) * u64::from(layer.opacity) + 127) / 255;
                if a == 0 {
                    continue;
                }
                let b = u64::from(dst[3]);
                let total = a * 255 + b * (255 - a);
                for channel in 0..3 {
                    dst[channel] = ((u64::from(src[channel]) * a * 255
                        + u64::from(dst[channel]) * b * (255 - a)
                        + total / 2)
                        / total) as u8;
                }
                dst[3] = ((total + 127) / 255) as u8;
            }
            pixels.extend(dst);
        }
    }
    pixels
}
#[test]
fn flatten_matches_dense_oracle_preserves_selection_title_and_records_one_undo() {
    let mut session = fixture();
    let before = session.snapshot();
    let selection = session.selection();
    let ticket = session.ticket();
    let depth = session.undo_depth();
    let expected = dense(&before);
    let receipt = session.flatten(render).unwrap();
    assert!(receipt.committed);
    assert_eq!(
        session.ticket().publication_epoch,
        ticket.publication_epoch + 1
    );
    assert_eq!(session.undo_depth(), depth + 1);
    assert!(session.is_dirty());
    assert_eq!(session.document().document_id, before.document_id);
    assert_eq!(session.document().title, before.title);
    assert_eq!(session.document().canvas, before.canvas);
    assert_eq!(session.selection(), selection);
    assert_eq!(session.document().roots.len(), 1);
    let id = receipt.aliases["flattened"];
    let layer = &session.document().layers[&id];
    assert_eq!(
        (layer.origin, layer.visible, layer.opacity),
        ((0, 0), true, 255)
    );
    assert_eq!(
        paintcraft_render::render_flat(session.document()).unwrap(),
        expected
    );
    assert_eq!(
        session.history_view().entries.last().unwrap().label,
        paintcraft_history::Label::FlattenImage
    );
    let flattened = session.snapshot();
    session.undo().unwrap();
    assert!(session.document().same_semantics(&before));
    assert_eq!(session.selection(), selection);
    assert!(!session.is_dirty());
    session.redo().unwrap();
    assert!(session.document().same_semantics(&flattened));
    assert_eq!(
        paintcraft_render::render_flat(session.document()).unwrap(),
        expected
    );
}
#[test]
fn failure_invalid_output_and_decoded_denial_publish_nothing() {
    let mut session = fixture();
    let before = session.snapshot();
    let ticket = session.ticket();
    let history = session.history_view();
    for size in [0, 1, 8 * 6 * 4 - 1, 8 * 6 * 4 + 1] {
        assert!(session.flatten(|_, _| Ok(vec![0; size])).is_err());
    }
    assert!(
        session
            .flatten(|_, _| Err(Error::new(
                ErrorCode::CodecFailed,
                "injected renderer failure"
            )))
            .is_err()
    );
    session.set_staged_pixel_budget(0);
    assert_eq!(
        session.flatten(render).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(session.ticket(), ticket);
    assert!(session.document().same_semantics(&before));
    assert_eq!(session.history_view(), history);
    assert!(!session.is_dirty());
}
#[test]
fn single_layer_noop_never_calls_renderer_or_changes_selection() {
    let mut session = Session::new(new_document(8, 6, Some(Rgba([255; 4])), 0, POLICY).unwrap());
    let ticket = session.ticket();
    let result = session
        .flatten(|_, _| panic!("single layer is already flat"))
        .unwrap();
    assert!(!result.committed);
    assert_eq!(session.ticket(), ticket);
    assert_eq!(session.selection(), Selection::Inactive);
    assert_eq!(session.undo_depth(), 0);
}
#[test]
fn detached_results_reject_stale_foreign_policy_and_cancelled_publication() {
    let mut session = fixture();
    let prepared = session
        .preparation()
        .unwrap()
        .flatten(render, Admission::new(u64::MAX), || Ok(()))
        .unwrap();
    let before = session.snapshot();
    assert!(session.document().same_semantics(&before));
    assert_eq!(
        session
            .publish_prepared(prepared, || Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel"
            )))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert!(session.document().same_semantics(&before));
    let prepared = session
        .preparation()
        .unwrap()
        .flatten(render, Admission::new(u64::MAX), || Ok(()))
        .unwrap();
    let mut foreign = Session::new((*before).clone());
    assert_eq!(
        foreign
            .publish_prepared(prepared, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    let prepared = session
        .preparation()
        .unwrap()
        .flatten(render, Admission::new(u64::MAX), || Ok(()))
        .unwrap();
    session.undo().unwrap();
    assert_eq!(
        session
            .publish_prepared(prepared, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    let prepared = session
        .preparation()
        .unwrap()
        .flatten(render, Admission::new(u64::MAX), || Ok(()))
        .unwrap();
    session.set_staged_pixel_budget(16);
    assert_eq!(
        session
            .publish_prepared(prepared, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
}
#[test]
fn every_observed_preparation_checkpoint_cancels_without_publishing() {
    let session = fixture();
    let count = Cell::new(0usize);
    session
        .preparation()
        .unwrap()
        .flatten(render, Admission::new(u64::MAX), || {
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap();
    for stop in 1..=count.get() {
        let seen = Cell::new(0usize);
        let result =
            session
                .preparation()
                .unwrap()
                .flatten(render, Admission::new(u64::MAX), || {
                    seen.set(seen.get() + 1);
                    if seen.get() == stop {
                        Err(Error::new(ErrorCode::OperationCancelled, "injected"))
                    } else {
                        Ok(())
                    }
                });
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::OperationCancelled,
            "checkpoint {stop}"
        );
    }
}
