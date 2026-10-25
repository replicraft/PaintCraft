//! Independent exact pixels and owner/history/resource boundaries for external Paste.
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    clipboard::Image,
    policy::{COORD_ABS_MAX, POLICY},
    resources::SharedBudget,
};
use paintcraft_document::new_document;
use paintcraft_editor::{
    clipboard::{PasteCanvas, PasteDraft},
    session::Session,
};
use paintcraft_raster::{Admission, RasterLayer, Selection};
use std::cell::Cell;
use std::sync::Arc;

fn session() -> Session {
    Session::new(new_document(4, 3, Some(Rgba([255; 4])), 0, POLICY).unwrap())
}
fn image() -> Image {
    Image::from_rgba(
        3,
        2,
        vec![
            17, 44, 99, 0, 255, 0, 7, 127, 90, 80, 70, 255, 0, 0, 0, 0, 1, 2, 3, 1, 4, 5, 6, 200,
        ],
        &SharedBudget::new(24),
        &|| Ok(()),
    )
    .unwrap()
}
fn select(session: &mut Session) {
    session.apply_json(format!(r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"selection.set_rect","rect":{{"x":1,"y":1,"width":1,"height":1}}}}]}}"#,session.ticket().state_id,session.ticket().publication_epoch).as_bytes()).unwrap();
}
#[test]
fn repeated_private_placement_then_one_insert_preserves_every_raw_sample_and_existing_layer() {
    for (origin, mode, canvas) in [
        ((-1, 1), PasteCanvas::Keep, (4, 3)),
        ((3, 2), PasteCanvas::Grow, (6, 4)),
        ((10, 10), PasteCanvas::Keep, (4, 3)),
    ] {
        let mut session = session();
        select(&mut session);
        let before = session.snapshot();
        let selection = session.selection();
        let ticket = session.ticket();
        let history = session.history_view();
        let image = image();
        let bytes = image.rgba().to_vec();
        let mut draft = PasteDraft::new(&session, image).unwrap();
        draft.place((2, 2), PasteCanvas::Keep).unwrap();
        draft.place(origin, mode).unwrap();
        assert!(Arc::ptr_eq(&before, &session.snapshot()));
        assert_eq!(session.ticket(), ticket);
        assert_eq!(session.history_view(), history);
        let receipt = session.paste_image(&draft, "貼り付け layer").unwrap();
        let id = receipt.aliases["pasted"];
        let doc = session.document();
        assert_eq!(doc.canvas, canvas);
        assert_eq!(doc.document_id, before.document_id);
        assert_eq!(doc.title, before.title);
        assert_eq!(doc.roots, vec![before.roots[0], id]);
        assert_eq!(
            doc.layers[&before.roots[0]],
            before.layers[&before.roots[0]]
        );
        let layer = &doc.layers[&id];
        assert_eq!(layer.name, "貼り付け layer");
        assert_eq!(layer.origin, origin);
        assert!(layer.visible);
        assert_eq!(layer.opacity, 255);
        for y in 0..2 {
            for x in 0..3 {
                let offset = ((y * 3 + x) * 4) as usize;
                assert_eq!(&layer.raster.get(x, y).0, &bytes[offset..offset + 4]);
            }
        }
        let bounds = Rect::new(origin.0, origin.1, 3, 2)
            .unwrap()
            .intersect(&doc.canvas_rect());
        assert_eq!(
            session.selection(),
            bounds.map_or(Selection::Empty, Selection::Rect)
        );
        assert_eq!(
            session.history_view().entries.len(),
            history.entries.len() + 1
        );
        assert_eq!(
            session.history_view().entries.last().unwrap().label.text(),
            "Paste"
        );
        let pasted = session.snapshot();
        session.undo().unwrap().unwrap();
        assert!(session.document().same_semantics(&before));
        assert_eq!(session.selection(), selection);
        session.redo().unwrap().unwrap();
        assert!(session.document().same_semantics(&pasted));
    }
}
#[test]
fn cancelled_and_invalid_drafts_keep_the_last_valid_position_and_share_one_pixel_lease() {
    let session = session();
    let before = session.snapshot();
    let budget = SharedBudget::new(4);
    let image = Image::from_rgba(1, 1, vec![17, 44, 99, 0], &budget, &|| Ok(())).unwrap();
    let mut draft = PasteDraft::new(&session, image.clone()).unwrap();
    drop(image);
    draft.place((-1, 2), PasteCanvas::Keep).unwrap();
    let clone = draft.clone();
    assert_eq!(budget.usage().reserved, 4);
    for (origin, mode) in [
        ((i64::MAX, 0), PasteCanvas::Keep),
        ((COORD_ABS_MAX, 0), PasteCanvas::Keep),
        ((-1, 0), PasteCanvas::Grow),
        ((1_048_576, 0), PasteCanvas::Grow),
    ] {
        assert_eq!(
            draft.place(origin, mode).unwrap_err().code,
            ErrorCode::InputSchema
        );
        assert_eq!(draft.origin(), (-1, 2));
        assert_eq!(draft.mode(), PasteCanvas::Keep);
    }
    drop(draft);
    assert_eq!(budget.usage().reserved, 4);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
}
#[test]
fn foreign_sessions_selection_changes_undo_epochs_and_resource_policy_reject_late_results() {
    let mut source = session();
    let draft = PasteDraft::new(&source, image()).unwrap();
    let mut other = session();
    assert_eq!(
        other.paste_image(&draft, "Paste").unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let prepared = source
        .preparation()
        .unwrap()
        .paste_image(&draft, "Paste", Admission::new(u64::MAX), || Ok(()))
        .unwrap();
    assert_eq!(
        other
            .publish_prepared(prepared, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    select(&mut source);
    source.undo().unwrap().unwrap();
    assert_eq!(
        source.paste_image(&draft, "Paste").unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let mut source = session();
    let draft = PasteDraft::new(&source, image()).unwrap();
    let prepared = source
        .preparation()
        .unwrap()
        .paste_image(&draft, "Paste", Admission::new(u64::MAX), || Ok(()))
        .unwrap();
    let before = source.snapshot();
    source.set_staged_pixel_budget(0);
    assert_eq!(
        source
            .publish_prepared(prepared, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert!(Arc::ptr_eq(&before, &source.snapshot()));
}
#[test]
fn private_preparation_publishes_once_and_hashes_all_meaningful_image_and_placement_inputs() {
    let mut a = session();
    let mut b = session();
    let da = PasteDraft::new(&a, image()).unwrap();
    let db = PasteDraft::new(&b, image()).unwrap();
    let before = a.snapshot();
    let prepared = a
        .preparation()
        .unwrap()
        .paste_image(&da, "Paste", Admission::new(u64::MAX), || Ok(()))
        .unwrap();
    let snapshot = prepared.document_snapshot();
    assert!(Arc::ptr_eq(&before, &a.snapshot()));
    let receipt = a.publish_prepared(prepared, || Ok(())).unwrap();
    b.paste_image(&db, "Paste").unwrap();
    assert_eq!(a.ticket().state_id, b.ticket().state_id);
    assert!(a.document().same_semantics(&snapshot));
    assert!(a.document().same_semantics(b.document()));
    let mut c = session();
    let mut dc = PasteDraft::new(&c, image()).unwrap();
    dc.place((1, 0), PasteCanvas::Keep).unwrap();
    c.paste_image(&dc, "Paste").unwrap();
    assert_ne!(receipt.state_id, c.ticket().state_id);
    let mut c = session();
    let dc = PasteDraft::new(&c, image()).unwrap();
    c.paste_image(&dc, "Other name").unwrap();
    assert_ne!(receipt.state_id, c.ticket().state_id);
    for grow in [false, true] {
        let mut c = session();
        let mut dc = PasteDraft::new(&c, image()).unwrap();
        dc.place(
            (3, 2),
            if grow {
                PasteCanvas::Grow
            } else {
                PasteCanvas::Keep
            },
        )
        .unwrap();
        c.paste_image(&dc, "Paste").unwrap();
        if grow {
            let mut other = session();
            let mut keep = PasteDraft::new(&other, image()).unwrap();
            keep.place((3, 2), PasteCanvas::Keep).unwrap();
            other.paste_image(&keep, "Paste").unwrap();
            assert_ne!(c.ticket().state_id, other.ticket().state_id);
        }
    }
    for dimensions in [(3, 2), (2, 3)] {
        let mut c = session();
        let mut rgba = image().rgba().to_vec();
        if dimensions == (3, 2) {
            rgba[0] += 1;
        } // Only hidden RGB changes.
        let pixels = Image::from_rgba(
            dimensions.0,
            dimensions.1,
            rgba,
            &SharedBudget::new(24),
            &|| Ok(()),
        )
        .unwrap();
        let dc = PasteDraft::new(&c, pixels).unwrap();
        c.paste_image(&dc, "Paste").unwrap();
        assert_ne!(receipt.state_id, c.ticket().state_id);
    }
    let mut c = session();
    let pixels = Image::from_fn(3, 2, &SharedBudget::new(24), &|| Ok(()), |_, _| {
        Ok(Rgba([0, 0, 0, 0]))
    })
    .unwrap();
    let dc = PasteDraft::new(&c, pixels).unwrap();
    c.paste_image(&dc, "Paste").unwrap();
    assert_ne!(receipt.state_id, c.ticket().state_id);
}
#[test]
fn every_preparation_cancellation_and_final_publication_cancellation_preserves_owner() {
    let mut session = session();
    let draft = PasteDraft::new(&session, image()).unwrap();
    let before = session.snapshot();
    let history = session.history_view();
    let budget = SharedBudget::new(RasterLayer::TILE_BYTES as u64);
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    drop(
        session
            .preparation()
            .unwrap()
            .paste_image(
                &draft,
                "Paste",
                Admission::with_residency(u64::MAX, budget.clone()),
                count,
            )
            .unwrap(),
    );
    for stop in 1..=calls.get() {
        let current = Cell::new(0);
        let check = || {
            current.set(current.get() + 1);
            if current.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            session
                .preparation()
                .unwrap()
                .paste_image(
                    &draft,
                    "Paste",
                    Admission::with_residency(u64::MAX, budget.clone()),
                    check
                )
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(budget.usage().reserved, 0);
        assert!(Arc::ptr_eq(&before, &session.snapshot()));
        assert_eq!(session.history_view(), history);
    }
    let prepared = session
        .preparation()
        .unwrap()
        .paste_image(
            &draft,
            "Paste",
            Admission::with_residency(u64::MAX, budget.clone()),
            || Ok(()),
        )
        .unwrap();
    assert_eq!(
        session
            .publish_prepared(prepared, || Err(Error::new(
                ErrorCode::OperationCancelled,
                "test"
            )))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
}
#[test]
fn decoded_and_history_denials_do_not_publish_or_retain_new_tile_payloads() {
    let mut source = session();
    let draft = PasteDraft::new(&source, image()).unwrap();
    let before = source.snapshot();
    let ticket = source.ticket();
    let budget = SharedBudget::new(0);
    assert_eq!(
        source
            .preparation()
            .unwrap()
            .paste_image(
                &draft,
                "Paste",
                Admission::with_residency(u64::MAX, budget.clone()),
                || Ok(())
            )
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().peak, 0);
    source.set_staged_pixel_budget(0);
    assert!(source.paste_image(&draft, "Paste").is_err());
    assert_eq!(source.ticket(), ticket);
    assert!(Arc::ptr_eq(&before, &source.snapshot()));
    let mut source = session();
    let draft = PasteDraft::new(&source, image()).unwrap();
    let before = source.snapshot();
    let depth = source.undo_depth();
    source.set_history_budget(0);
    let history = source.history_view();
    let budget = SharedBudget::new(RasterLayer::TILE_BYTES as u64);
    assert_eq!(
        source
            .preparation()
            .unwrap()
            .paste_image(
                &draft,
                "Paste",
                Admission::with_residency(u64::MAX, budget.clone()),
                || Ok(())
            )
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().peak, RasterLayer::TILE_BYTES as u64);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(source.history_view(), history);
    assert_eq!(
        source.paste_image(&draft, "Paste").unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(source.undo_depth(), depth);
    assert!(Arc::ptr_eq(&before, &source.snapshot()));
}

#[test]
fn acknowledged_snapshots_retain_tile_capacity_until_the_last_reader_drops() {
    let mut source = session();
    let draft = PasteDraft::new(&source, image()).unwrap();
    let budget = SharedBudget::new(RasterLayer::TILE_BYTES as u64);
    let prepared = source
        .preparation()
        .unwrap()
        .paste_image(
            &draft,
            "Paste",
            Admission::with_residency(u64::MAX, budget.clone()),
            || Ok(()),
        )
        .unwrap();
    let exported = prepared.document_snapshot();
    source.publish_prepared(prepared, || Ok(())).unwrap();
    assert_eq!(budget.usage().reserved, RasterLayer::TILE_BYTES as u64);
    source.undo().unwrap().unwrap();
    assert_eq!(budget.usage().reserved, RasterLayer::TILE_BYTES as u64);
    source.redo().unwrap().unwrap();
    drop(source);
    assert_eq!(budget.usage().reserved, RasterLayer::TILE_BYTES as u64);
    drop(exported);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(&draft.image().rgba()[..4], &[17, 44, 99, 0]);
}
