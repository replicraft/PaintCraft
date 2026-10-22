use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba, resources::SharedBudget, text::MaskSpan,
};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_raster::{Admission, RasterLayer};
use std::cell::Cell;
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;

fn compound(doc: &Document, second: Op) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: 0,
        seed: None,
        digest: "0".repeat(64),
        operations: vec![
            Op::FillRect {
                layer: LayerRef::Id(doc.roots[0]),
                rect: Rect::new(0, 0, 1, 1).unwrap(),
                color: Rgba([1, 2, 3, 255]),
                mode: Mode::Replace,
            },
            second,
        ],
    }
}

#[test]
fn cancelling_text_and_clipped_mask_scans_discards_private_prefixes() {
    let doc = new_document(512, 64, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    for second in [
        Op::Text {
            layer: LayerRef::Id(id),
            at: (0, 0),
            text: "AAAA".into(),
            scale: 16,
            color: Rgba([17, 88, 156, 128]),
            mode: Mode::SourceOver,
        },
        Op::MaskSpans {
            layer: LayerRef::Id(id),
            color: Rgba([17, 88, 156, 128]),
            spans: (100..4196)
                .map(|y| MaskSpan {
                    y,
                    x0: 0,
                    x1: 0,
                    coverage: 128,
                })
                .collect(),
        },
    ] {
        let batch = compound(&doc, second);
        let session = Session::new(doc.clone());
        let snapshot = session.snapshot();
        let ticket = session.ticket();
        let history = session.history_view();
        let total = Cell::new(0);
        let budget = SharedBudget::new(4 * BYTES);
        let result = session
            .preparation()
            .unwrap()
            .apply_with_admission(
                &batch,
                Admission::with_residency(4 * 65_536, budget.clone()),
                || {
                    total.set(total.get() + 1);
                    Ok(())
                },
            )
            .unwrap();
        drop(result);
        assert_eq!(budget.usage().reserved, 0);
        // Include checks near the end: for an entirely offscreen mask these
        // cover the final clipping traversal, not only initial validation.
        for cancel in [
            20,
            total.get() - 6,
            total.get() - 5,
            total.get() - 4,
            total.get() - 3,
            total.get() - 2,
            total.get() - 1,
            total.get(),
        ] {
            let calls = Cell::new(0);
            let error = session
                .preparation()
                .unwrap()
                .apply_with_admission(
                    &batch,
                    Admission::with_residency(4 * 65_536, budget.clone()),
                    || {
                        calls.set(calls.get() + 1);
                        if calls.get() == cancel {
                            Err(Error::new(
                                ErrorCode::OperationCancelled,
                                "cancel text preparation",
                            ))
                        } else {
                            Ok(())
                        }
                    },
                )
                .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert_eq!(calls.get(), cancel);
            assert_eq!(budget.usage().reserved, 0);
            assert_eq!(session.ticket(), ticket);
            assert_eq!(session.history_view(), history);
            assert!(std::sync::Arc::ptr_eq(&snapshot, &session.snapshot()));
        }
    }
}

#[test]
fn streamed_text_metadata_denial_and_unsupported_glyph_keep_owner_history() {
    for (text, limit, expected) in [
        ("AAAA", 4096, ErrorCode::ResourceLimit),
        ("A漢", 64 << 20, ErrorCode::CapabilityUnsupported),
    ] {
        let doc = new_document(512, 64, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
        let id = doc.roots[0];
        let batch = compound(
            &doc,
            Op::Text {
                layer: LayerRef::Id(id),
                at: (0, 0),
                text: text.into(),
                scale: 16,
                color: Rgba([17, 88, 156, 128]),
                mode: Mode::SourceOver,
            },
        );
        let session = Session::new(doc);
        let snapshot = session.snapshot();
        let ticket = session.ticket();
        let history = session.history_view();
        let budget = SharedBudget::new(4 * BYTES);
        let error = session
            .preparation()
            .unwrap()
            .apply_with_admission(
                &batch,
                Admission::with_residency(4 * 65_536, budget.clone())
                    .with_paint_metadata_limit(limit),
                || Ok(()),
            )
            .unwrap_err();
        assert_eq!(error.code, expected);
        assert_eq!(error.operation_index, Some(1));
        assert!(error.allocation_denial().is_none());
        assert_eq!(budget.usage().peak, BYTES);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(session.ticket(), ticket);
        assert_eq!(session.history_view(), history);
        assert!(std::sync::Arc::ptr_eq(&snapshot, &session.snapshot()));
    }
}
