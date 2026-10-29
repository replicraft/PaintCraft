//! Actual cold reconstruction and filesystem tile spilling retain one ICC lease.
use super::*;
use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{Rect, resources::SharedBudget};
use paintcraft_document::source_metadata::BYTES_MAX;
use paintcraft_editor::session::Session;
use std::{path::PathBuf, sync::Arc};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn apply(session: &mut Session, operation: Op, index: u64) {
    let batch = Batch {
        version: 2,
        policy: paintcraft_document::POLICY.into(),
        base_state: session.document().state_id,
        base_epoch: session.ticket().publication_epoch,
        seed: None,
        digest: format!("{index:064x}"),
        operations: vec![operation],
    };
    assert!(session.apply(&batch).unwrap().committed);
}
#[test]
fn hot_cold_actual_spill_restore_and_last_drop_share_source_attachment_charge() {
    let parent = Scratch(std::env::temp_dir().join(format!(
        "paintcraft-source-metadata-history-{}",
        Uuid::new_v4()
    )));
    let budget = SharedBudget::new(BYTES_MAX);
    let bytes = include_bytes!("../../tests/pcnd-metadata-fixtures/rgb-profile.pcnd");
    let doc = read_with_metadata_admission_checked(
        bytes,
        u64::MAX,
        STAGED_PIXELS_MAX,
        LAYER_COUNT_MAX,
        MetadataAdmission {
            bytes: BYTES_MAX,
            budget: &budget,
        },
        &|| Ok(()),
    )
    .unwrap();
    let attachment = doc.source_metadata.clone().unwrap();
    let storage = attachment.storage_usage();
    let mut session = Session::new(doc);
    apply(
        &mut session,
        Op::LayerAdd {
            alias: "pixels".into(),
            name: "Pixels".into(),
            width: 2,
            height: 3,
            origin: (0, 0),
            default: Rgba::TRANSPARENT,
        },
        1,
    );
    let layer = session.document().roots[0];
    let store = Arc::new(
        paintcraft_platform::spill::NativeSpillStore::new(&parent.0, 8 << 20, 1 << 20).unwrap(),
    );
    session.set_history_spill_store(store.clone());
    let mut recorded = vec![];
    for (index, color) in [
        Rgba([13, 27, 41, 0]),
        Rgba([51, 61, 71, 127]),
        Rgba([91, 101, 111, 255]),
    ]
    .into_iter()
    .enumerate()
    {
        apply(
            &mut session,
            Op::FillRect {
                layer: LayerRef::Id(layer),
                rect: Rect::new(0, 0, 1, 1).unwrap(),
                color,
                mode: Mode::Replace,
            },
            index as u64 + 2,
        );
        recorded.push((session.document().state_id, color));
        assert_eq!(session.history_memory().source_metadata_bytes, storage.1);
        assert_eq!(budget.usage().reserved, storage.1);
    }
    let compact = session.compact_history(1, 8 << 20, || Ok(())).unwrap();
    assert!(compact.packed_entries >= 2);
    let spilled = session.spill_history(8 << 20, 32, || Ok(())).unwrap();
    assert!(spilled.payloads > 0);
    assert!(session.history_memory().disk_payloads > 0);
    assert_eq!(session.history_memory().source_metadata_bytes, storage.1);
    for (state, color) in recorded[..recorded.len() - 1].iter().rev() {
        session.undo().unwrap().unwrap();
        assert_eq!(session.document().state_id, *state);
        assert_eq!(session.document().layers[&layer].raster.get(0, 0), *color);
        assert_eq!(
            session
                .document()
                .source_metadata
                .as_ref()
                .unwrap()
                .storage_usage(),
            storage
        );
        let reopened = read(&write_compact(session.document()).unwrap()).unwrap();
        assert!(reopened.same_semantics(session.document()));
    }
    for (state, color) in &recorded[1..] {
        session.redo().unwrap().unwrap();
        assert_eq!(session.document().state_id, *state);
        assert_eq!(session.document().layers[&layer].raster.get(0, 0), *color);
        assert_eq!(
            session
                .document()
                .source_metadata
                .as_ref()
                .unwrap()
                .storage_usage(),
            storage
        );
    }
    drop(session);
    drop(store);
    assert_eq!(budget.usage().reserved, storage.1);
    drop(attachment);
    assert_eq!(budget.usage().reserved, 0);
}
