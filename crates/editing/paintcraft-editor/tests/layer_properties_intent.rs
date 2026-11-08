//! Explicit UI intention changes presentation label, not generic atomic semantics.
use paintcraft_commands::{Batch, LayerRef, Op};
use paintcraft_core::types::{Error, ErrorCode, LayerBlendMode, Result, Rgba};
use paintcraft_document::{Document, POLICY};
use paintcraft_editor::session::Session;
use std::sync::Arc;
fn session() -> Session {
    Session::new(
        paintcraft_document::new_document(2, 1, Some(Rgba([17, 31, 99, 128])), 0, POLICY).unwrap(),
    )
}
fn batch(session: &Session, mode: LayerBlendMode) -> Batch {
    Batch {
        version: 8,
        policy: POLICY.into(),
        base_state: session.ticket().state_id,
        base_epoch: session.ticket().publication_epoch,
        seed: None,
        operations: vec![
            Op::LayerSetProperties {
                layer: LayerRef::Id(session.document().roots[0]),
                name: Some("Sky".into()),
                visible: Some(false),
                opacity: Some(93),
            },
            Op::LayerSetBlendMode {
                layer: LayerRef::Id(session.document().roots[0]),
                mode,
            },
        ],
        digest: "e1".repeat(32),
    }
}
#[test]
fn direct_and_prepared_ui_pairs_keep_one_properties_label_while_generic_pair_stays_batch() {
    for prepared in [false, true] {
        for mode in [LayerBlendMode::Normal, LayerBlendMode::Screen] {
            let mut ui = session();
            let source = ui.snapshot();
            let mut generic = Session::new((*source).clone());
            let batch = batch(&ui, mode);
            let receipt = if prepared {
                let edit = ui
                    .preparation()
                    .unwrap()
                    .apply_layer_properties(&batch, || Ok(()))
                    .unwrap();
                assert!(Arc::ptr_eq(&source, &ui.snapshot()));
                assert_eq!(ui.undo_depth(), 0);
                ui.publish_prepared(edit, || Ok(())).unwrap()
            } else {
                ui.apply_layer_properties(&batch).unwrap()
            };
            let ordinary = generic.apply(&batch).unwrap();
            assert!(receipt.committed);
            assert_eq!(
                receipt, ordinary,
                "Presentation intent does not alter batch/state/history identities"
            );
            assert_eq!(ui.undo_depth(), 1);
            assert_eq!(
                ui.history_view().entries.last().unwrap().label.text(),
                "Layer properties"
            );
            assert_eq!(
                generic.history_view().entries.last().unwrap().label.text(),
                "Batch edit"
            );
            let applied = ui.snapshot();
            assert!(applied.same_semantics(generic.document()));
            assert_eq!(applied.layers[&applied.roots[0]].blend_mode, mode);
            ui.undo().unwrap();
            assert!(ui.document().same_semantics(&source));
            assert_eq!(ui.undo_depth(), 0);
            ui.redo().unwrap();
            assert!(ui.document().same_semantics(&applied));
        }
    }
}
#[test]
fn explicit_intention_rejects_wrong_order_unrelated_alias_unknown_and_cross_layer_pairs() {
    for kind in 0..6 {
        let mut session = session();
        let source = session.snapshot();
        let ticket = session.ticket();
        let mut b = batch(&session, LayerBlendMode::Multiply);
        match kind {
            0 => b.operations.reverse(),
            1 => {
                b.operations.pop();
            }
            2 => b.operations.push(Op::SelectionClear),
            3 => {
                if let Op::LayerSetBlendMode { layer, .. } = &mut b.operations[1] {
                    *layer = LayerRef::Id(uuid::Uuid::from_u128(999));
                }
            }
            4 => {
                for op in &mut b.operations {
                    match op {
                        Op::LayerSetProperties { layer, .. }
                        | Op::LayerSetBlendMode { layer, .. } => {
                            *layer = LayerRef::Alias("forged".into())
                        }
                        _ => unreachable!(),
                    }
                }
            }
            5 => {
                for op in &mut b.operations {
                    match op {
                        Op::LayerSetProperties { layer, .. }
                        | Op::LayerSetBlendMode { layer, .. } => {
                            *layer = LayerRef::Id(uuid::Uuid::from_u128(999))
                        }
                        _ => unreachable!(),
                    }
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(
            session.apply_layer_properties(&b).unwrap_err().code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            session
                .preparation()
                .unwrap()
                .apply_layer_properties(&b, || Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert!(Arc::ptr_eq(&source, &session.snapshot()));
        assert_eq!(session.ticket(), ticket);
        assert_eq!(session.undo_depth(), 0);
    }
}
fn reject(_: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
    check()?;
    Err(Error::new(
        ErrorCode::ResourceLimit,
        "test persistent boundary",
    ))
}
#[test]
fn noop_cancel_stale_and_persistent_refusal_keep_original_publication_and_history() {
    let mut session = session();
    let source = session.snapshot();
    let mut noop = batch(&session, LayerBlendMode::Normal);
    let layer = &source.layers[&source.roots[0]];
    noop.operations[0] = Op::LayerSetProperties {
        layer: LayerRef::Id(layer.id),
        name: Some(layer.name.clone()),
        visible: Some(layer.visible),
        opacity: Some(layer.opacity),
    };
    assert!(!session.apply_layer_properties(&noop).unwrap().committed);
    assert_eq!(session.undo_depth(), 0);
    assert!(Arc::ptr_eq(&source, &session.snapshot()));
    let mut stale = batch(&session, LayerBlendMode::Screen);
    stale.base_epoch += 1;
    assert_eq!(
        session.apply_layer_properties(&stale).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let edit = batch(&session, LayerBlendMode::Screen);
    assert_eq!(
        session
            .preparation()
            .unwrap()
            .apply_layer_properties(&edit, || Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel"
            )))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert!(Arc::ptr_eq(&source, &session.snapshot()));
    session = session.with_persistent_validator(Some(reject));
    assert_eq!(
        session.apply_layer_properties(&edit).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let prepared = session
        .preparation()
        .unwrap()
        .apply_layer_properties(&edit, || Ok(()))
        .unwrap();
    assert_eq!(
        session
            .publish_prepared(prepared, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(Arc::ptr_eq(&source, &session.snapshot()));
    assert_eq!(session.undo_depth(), 0);
}
