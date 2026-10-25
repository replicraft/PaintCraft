//! Independent document/history checks for active-layer external image placement.
use paintcraft_core::types::{
    Error, ErrorCode, Rgba,
    clipboard::Image,
    policy::{BlendMode, POLICY},
    resources::SharedBudget,
};
use paintcraft_document::new_document;
use paintcraft_editor::{
    clipboard::{PasteCanvas, PasteDraft},
    session::Session,
};
use paintcraft_raster::{Admission, RasterLayer, Selection};
use std::{cell::Cell, sync::Arc};
fn session() -> Session {
    let mut doc = new_document(4, 3, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.origin = (1, 1);
    layer.opacity = 127;
    layer.raster = RasterLayer::new(2, 2, Rgba([0, 0, 255, 255]));
    let id = paintcraft_core::types::ids::deterministic_id(&["paste-onto-unrelated"]);
    let mut other = doc.layers[&doc.roots[0]].clone();
    other.id = id;
    other.origin = (0, 0);
    other.name = "Unrelated".into();
    doc.roots.push(id);
    doc.layers.insert(id, other);
    Session::new(doc)
}
fn image() -> Image {
    Image::from_rgba(
        3,
        2,
        vec![
            17, 44, 99, 0, 255, 0, 0, 127, 90, 80, 70, 255, 0, 0, 0, 0, 1, 2, 3, 1, 4, 5, 6, 200,
        ],
        &SharedBudget::new(24),
        &|| Ok(()),
    )
    .unwrap()
}
fn select(session: &mut Session) {
    let t = session.ticket();
    session.apply_json(format!(r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"selection.set_rect","rect":{{"x":0,"y":0,"width":1,"height":1}}}}]}}"#,t.state_id,t.publication_epoch).as_bytes()).unwrap();
}
#[test]
fn existing_layer_pixels_metadata_and_canvas_are_one_undoable_placement() {
    for (origin, mode, canvas) in [
        ((-1, 0), PasteCanvas::Keep, (4, 3)),
        ((3, 2), PasteCanvas::Grow, (6, 4)),
        ((10, 10), PasteCanvas::Keep, (4, 3)),
    ] {
        let mut session = session();
        select(&mut session);
        let before = session.snapshot();
        let old_selection = session.selection();
        let history = session.history_view();
        let target = before.roots[0];
        let pixels = image();
        let mut draft = PasteDraft::new(&session, pixels.clone()).unwrap();
        draft.place(origin, mode).unwrap();
        let receipt = session
            .paste_image_onto(&draft, target, BlendMode::Replace, None)
            .unwrap();
        assert_eq!(receipt.aliases["pasted"], target);
        assert_eq!(session.document().roots, before.roots);
        assert_eq!(session.document().canvas, canvas);
        assert_eq!(session.document().document_id, before.document_id);
        let out = &session.document().layers[&target];
        let old = &before.layers[&target];
        for (&id, layer) in &before.layers {
            if id != target {
                assert_eq!(&session.document().layers[&id], layer);
            }
        }
        assert_eq!(out.name, old.name);
        assert_eq!(out.opacity, old.opacity);
        assert_eq!(out.visible, old.visible);
        let bounds = draft.bounds();
        let extent = out.extent();
        for y in extent.y0..extent.y1 {
            for x in extent.x0..extent.x1 {
                let expected = if bounds.contains(x, y) {
                    let at = (((y - origin.1) * 3 + (x - origin.0)) * 4) as usize;
                    Rgba(pixels.rgba()[at..at + 4].try_into().unwrap())
                } else if old.extent().contains(x, y) {
                    old.raster.get(x - old.origin.0, y - old.origin.1)
                } else {
                    Rgba::TRANSPARENT
                };
                assert_eq!(out.raster.get(x - out.origin.0, y - out.origin.1), expected);
            }
        }
        assert_eq!(
            session.selection(),
            bounds
                .intersect(&session.document().canvas_rect())
                .map_or(Selection::Empty, Selection::Rect)
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
        assert_eq!(session.selection(), old_selection);
        session.redo().unwrap().unwrap();
        assert!(session.document().same_semantics(&pasted));
    }
}
#[test]
fn source_over_and_rgb_key_have_explicit_distinct_policies() {
    let mut session = session();
    let target = session.document().roots[0];
    let mut draft = PasteDraft::new(&session, image()).unwrap();
    draft.place((1, 1), PasteCanvas::Keep).unwrap();
    session
        .paste_image_onto(
            &draft,
            target,
            BlendMode::SourceOver,
            Some(Rgba([90, 80, 70, 0])),
        )
        .unwrap();
    let raster = &session.document().layers[&target].raster;
    assert_eq!(raster.get(0, 0), Rgba([0, 0, 255, 255]));
    assert_eq!(raster.get(1, 0), Rgba([127, 0, 128, 255]));
    assert_eq!(raster.get(2, 0), Rgba::TRANSPARENT);
}
#[test]
fn prepared_publication_rejects_foreign_selection_and_resource_changes() {
    for mode in 0..4 {
        let mut session = session();
        let target = session.document().roots[0];
        let draft = PasteDraft::new(&session, image()).unwrap();
        let prepared = session
            .preparation()
            .unwrap()
            .paste_image_onto(
                &draft,
                target,
                BlendMode::Replace,
                None,
                Admission::new(1 << 20),
                || Ok(()),
            )
            .unwrap();
        match mode {
            0 => {
                session = Session::new((*session.snapshot()).clone());
            }
            1 => select(&mut session),
            2 => session.set_staged_pixel_budget(0),
            _ => {}
        }
        let before = session.snapshot();
        let result = session.publish_prepared(prepared, || {
            if mode == 3 {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result.unwrap_err().code,
            if mode == 3 {
                ErrorCode::OperationCancelled
            } else {
                ErrorCode::ConflictStaleBase
            }
        );
        assert!(Arc::ptr_eq(&before, &session.snapshot()));
    }
}
#[test]
fn invalid_hidden_targets_stale_drafts_and_quota_failure_do_not_publish() {
    let mut session = session();
    let target = session.document().roots[0];
    let draft = PasteDraft::new(&session, image()).unwrap();
    let before = session.snapshot();
    session.set_staged_pixel_budget(0);
    assert_eq!(
        session
            .paste_image_onto(&draft, target, BlendMode::Replace, None)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
    let mut doc = (*before).clone();
    doc.layers.get_mut(&target).unwrap().visible = false;
    let mut session = Session::new(doc);
    let draft = PasteDraft::new(&session, image()).unwrap();
    assert_eq!(
        session
            .paste_image_onto(&draft, target, BlendMode::Replace, None)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let mut session = Session::new((*before).clone());
    let draft = PasteDraft::new(&session, image()).unwrap();
    select(&mut session);
    assert_eq!(
        session
            .paste_image_onto(&draft, target, BlendMode::Replace, None)
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
}
#[test]
fn every_observed_preparation_cancellation_restores_capacity_and_preserves_owner() {
    let session = session();
    let target = session.document().roots[0];
    let draft = PasteDraft::new(&session, image()).unwrap();
    let before = session.snapshot();
    let history = session.history_view();
    let budget = SharedBudget::new(RasterLayer::TILE_BYTES as u64 * 4);
    let count = Cell::new(0);
    drop(
        session
            .preparation()
            .unwrap()
            .paste_image_onto(
                &draft,
                target,
                BlendMode::Replace,
                None,
                Admission::with_residency(1 << 20, budget.clone()),
                || {
                    count.set(count.get() + 1);
                    Ok(())
                },
            )
            .unwrap(),
    );
    for fail in 1..=count.get() {
        let calls = Cell::new(0);
        let error = session
            .preparation()
            .unwrap()
            .paste_image_onto(
                &draft,
                target,
                BlendMode::Replace,
                None,
                Admission::with_residency(1 << 20, budget.clone()),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == fail {
                        Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, 0);
        assert!(Arc::ptr_eq(&before, &session.snapshot()));
        assert_eq!(session.history_view(), history);
    }
}

#[test]
fn deterministic_policy_inputs_and_private_successful_publication_are_preserved() {
    fn run(mode: BlendMode, key: Option<Rgba>, origin: (i64, i64)) -> Session {
        let mut s = session();
        let target = s.document().roots[0];
        let mut d = PasteDraft::new(&s, image()).unwrap();
        d.place(origin, PasteCanvas::Keep).unwrap();
        s.paste_image_onto(&d, target, mode, key).unwrap();
        s
    }
    let a = run(BlendMode::Replace, None, (0, 0));
    let b = run(BlendMode::Replace, None, (0, 0));
    assert_eq!(a.ticket().state_id, b.ticket().state_id);
    assert!(a.document().same_semantics(b.document()));
    for other in [
        run(BlendMode::SourceOver, None, (0, 0)),
        run(BlendMode::Replace, Some(Rgba([17, 44, 99, 255])), (0, 0)),
        run(BlendMode::Replace, None, (1, 0)),
    ] {
        assert_ne!(a.ticket().state_id, other.ticket().state_id);
    }
    let a = run(BlendMode::Replace, Some(Rgba([17, 44, 99, 0])), (0, 0));
    let b = run(BlendMode::Replace, Some(Rgba([17, 44, 99, 255])), (0, 0));
    assert_eq!(a.ticket().state_id, b.ticket().state_id);
    let mut s = session();
    let before = s.snapshot();
    let target = s.document().roots[0];
    let d = PasteDraft::new(&s, image()).unwrap();
    let prepared = s
        .preparation()
        .unwrap()
        .paste_image_onto(
            &d,
            target,
            BlendMode::Replace,
            None,
            Admission::new(1 << 20),
            || Ok(()),
        )
        .unwrap();
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    let expected = prepared.document_snapshot();
    s.publish_prepared(prepared, || Ok(())).unwrap();
    assert!(s.document().same_semantics(&expected));
}
#[test]
fn late_history_denial_discards_all_private_tiles() {
    let mut s = session();
    s.set_history_budget(0);
    let before = s.snapshot();
    let history = s.history_view();
    let target = s.document().roots[0];
    let d = PasteDraft::new(&s, image()).unwrap();
    let budget = SharedBudget::new(4 * RasterLayer::TILE_BYTES as u64);
    let error = s
        .preparation()
        .unwrap()
        .paste_image_onto(
            &d,
            target,
            BlendMode::Replace,
            None,
            Admission::with_residency(1 << 20, budget.clone()),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(budget.usage().reserved, 0);
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    assert_eq!(history, s.history_view());
}

#[test]
fn exact_private_preview_needs_no_history_credit_and_matches_published_pixels() {
    let mut s = session();
    let before = s.snapshot();
    let target = s.document().roots[0];
    let mut d = PasteDraft::new(&s, image()).unwrap();
    d.place((-1, 0), PasteCanvas::Keep).unwrap();
    s.set_history_budget(0);
    let history = s.history_view();
    let preview = s
        .preview_paste_image_onto(
            &d,
            target,
            BlendMode::SourceOver,
            None,
            Admission::new(1 << 20),
            &|| Ok(()),
        )
        .unwrap();
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    assert_eq!(history, s.history_view());
    assert_eq!(
        s.paste_image_onto(&d, target, BlendMode::SourceOver, None)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    s.set_history_budget(64 << 20);
    s.paste_image_onto(&d, target, BlendMode::SourceOver, None)
        .unwrap();
    assert_eq!(preview.canvas, s.document().canvas);
    assert_eq!(preview.layers, s.document().layers);
    assert_eq!(preview.roots, s.document().roots);
}
