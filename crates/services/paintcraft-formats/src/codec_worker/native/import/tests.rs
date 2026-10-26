use super::*;
use paintcraft_editor::session::Session;
fn prepared(
    residency: SharedBudget,
    preparation: SharedBudget,
    path: &Path,
    owner: Owner,
) -> PreparedImport {
    let mut admission = Admission::with_residency(1 << 20, residency);
    let document = paintcraft_document::import_image_with_admission_checked(
        257,
        1,
        &vec![255; 257 * 4],
        "source",
        &mut admission,
        &|| Ok(()),
    )
    .unwrap();
    PreparedImport {
        document,
        input: path.to_owned(),
        identity: SourceIdentity::existing_destination(path).unwrap().unwrap(),
        length: 6,
        sha256: Sha256::digest(b"source").into(),
        owner,
        _lease: ResourceLease::new(preparation.reserve(HEAP_BYTES).unwrap()),
    }
}
#[test]
fn resident_tiles_survive_session_snapshots_and_stale_results_release_both_realms() {
    let root = std::env::temp_dir().join(format!("paintcraft-codec-adopt-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("source.jxl");
    std::fs::write(&input, b"source").unwrap();
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let resident = SharedBudget::new(1 << 20);
    let preparation = SharedBudget::new(HEAP_BYTES);
    let pending = prepared(resident.clone(), preparation.clone(), &input, owner);
    let expected = 2 * paintcraft_raster::RasterLayer::TILE_BYTES as u64;
    assert_eq!(resident.usage().reserved, expected);
    assert_eq!(preparation.usage().reserved, HEAP_BYTES);
    let adopted = pending.adopt(&|| Ok(()), &|| Ok(owner)).unwrap();
    assert_eq!(preparation.usage().reserved, 0);
    assert_eq!(resident.usage().reserved, expected);
    let mut session = Session::new(adopted.document);
    let ticket = session.ticket();
    let layer = session.document().roots[0];
    let batch = format!(
        r#"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"layer.delete","layer_ref":{{"id":"{}"}}}}]}}"#,
        ticket.state_id, ticket.publication_epoch, layer
    );
    session.apply_json(batch.as_bytes()).unwrap();
    assert!(session.document().roots.is_empty());
    assert_eq!(
        resident.usage().reserved,
        expected,
        "history owns the removed imported tiles"
    );
    session.undo().unwrap().unwrap();
    assert_eq!(session.document().roots, [layer]);
    let snapshot = session.snapshot();
    drop(session);
    assert_eq!(resident.usage().reserved, expected);
    drop(snapshot);
    assert_eq!(resident.usage().reserved, 0);
    let pending = prepared(resident.clone(), preparation.clone(), &input, owner);
    let mut foreign = owner;
    foreign.epoch += 1;
    assert_eq!(
        pending.adopt(&|| Ok(()), &|| Ok(foreign)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(resident.usage().reserved, 0);
    assert_eq!(preparation.usage().reserved, 0);
    let pending = prepared(resident.clone(), preparation.clone(), &input, owner);
    std::fs::write(&input, b"mutate").unwrap();
    assert!(pending.adopt(&|| Ok(()), &|| Ok(owner)).is_err());
    assert_eq!(resident.usage().reserved, 0);
    assert_eq!(preparation.usage().reserved, 0);
    std::fs::remove_dir_all(root).unwrap();
}
