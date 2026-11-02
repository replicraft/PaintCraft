//! Actual Stage/Session/compact metadata paths, with visual rendering still closed.
use paintcraft_commands::{Batch, LayerRef, Op};
use paintcraft_core::types::{LayerBlendMode, Rgba};
use paintcraft_document::{Document, POLICY};
use paintcraft_editor::{execute, session::Session};
use paintcraft_raster::{Admission, Selection};
fn batch(d: &Document, epoch: u64, mode: LayerBlendMode) -> Batch {
    Batch {
        version: 8,
        policy: POLICY.into(),
        base_state: d.state_id,
        base_epoch: epoch,
        seed: None,
        operations: vec![Op::LayerSetBlendMode {
            layer: LayerRef::Id(d.roots[0]),
            mode,
        }],
        digest: match mode {
            LayerBlendMode::Normal => "0",
            LayerBlendMode::Multiply => "1",
            LayerBlendMode::Screen => "2",
        }
        .repeat(64),
    }
}
#[test]
fn real_stage_replay_session_history_and_compact_metadata_preserve_modes_and_raw_pixels() {
    let doc = paintcraft_document::import_image(2, 1, &[17, 32, 255, 0, 0, 127, 200, 255], "blend")
        .unwrap();
    let id = doc.roots[0];
    let b = batch(&doc, 0, LayerBlendMode::Multiply);
    let first = execute::execute_with_admission_checked(
        &doc,
        Selection::Inactive,
        &b,
        Admission::new(1 << 20),
        &|| Ok(()),
    )
    .unwrap();
    let replay = execute::execute_with_admission_checked(
        &doc,
        Selection::Inactive,
        &b,
        Admission::new(1 << 20),
        &|| Ok(()),
    )
    .unwrap();
    assert!(first.doc.same_semantics(&replay.doc));
    assert_eq!(first.doc.layers[&id].blend_mode, LayerBlendMode::Multiply);
    let mut compact_base = first.doc.clone();
    let filter_id = uuid::Uuid::from_u128(91);
    compact_base.layers.get_mut(&id).unwrap().filters =
        paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
            filter_id,
            true,
            paintcraft_core::types::filters::Descriptor::InvertRgbV1,
        )])
        .unwrap();
    let mut filter_batch = batch(&compact_base, 0, LayerBlendMode::Normal);
    filter_batch.operations = vec![Op::Filter(paintcraft_commands::FilterOp::SetEnabled {
        layer: LayerRef::Id(id),
        filter: paintcraft_commands::FilterRef::Id(filter_id),
        enabled: false,
    })];
    let scope = paintcraft_core::types::resources::SharedBudget::new(8 << 20)
        .scope(paintcraft_core::types::resources::AllocationClass::PaintMetadata);
    let compact = execute::metadata_clone::stage_checked(
        &compact_base,
        &filter_batch,
        Admission::new(1 << 20),
        &scope,
        &|| Ok(()),
        &|_| Ok(()),
    )
    .unwrap();
    assert_eq!(
        compact.document().layers[&id].blend_mode,
        LayerBlendMode::Multiply
    );
    assert_eq!(
        compact.document().layers[&id].raster.get(0, 0),
        first.doc.layers[&id].raster.get(0, 0)
    );
    let mut session = Session::new(doc);
    assert!(session.apply(&b).unwrap().committed);
    let saved = session.snapshot();
    let b2 = batch(
        session.document(),
        session.ticket().publication_epoch,
        LayerBlendMode::Screen,
    );
    assert!(session.apply(&b2).unwrap().committed);
    assert_eq!(
        session.document().layers[&id].blend_mode,
        LayerBlendMode::Screen
    );
    session.undo().unwrap();
    assert!(session.document().same_semantics(&saved));
    session.redo().unwrap();
    assert_eq!(
        session.document().layers[&id].blend_mode,
        LayerBlendMode::Screen
    );
    assert_eq!(
        session.document().layers[&id].raster.get(0, 0),
        Rgba([17, 32, 255, 0])
    );
    let before = session.snapshot();
    assert!(session.apply(&b).is_err(), "stale ticket cannot publish");
    assert!(session.document().same_semantics(&before));
}
