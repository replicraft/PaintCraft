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
        profile: PreparedProfile::Jxl,
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

fn prepared_jxr(
    residency: SharedBudget,
    preparation: SharedBudget,
    path: &Path,
    owner: Owner,
) -> PreparedImport {
    let source = include_bytes!("../../../../tests/jxr-fixtures/rgb8-dpi96x144.jxr");
    let assessment = crate::jxr::assess_import(source, PIXELS_MAX, &|| Ok(())).unwrap();
    let rgba: Vec<_> = (0..3)
        .flat_map(|y| {
            (0..4).flat_map(move |x| {
                [
                    ((x * 47 + y * 13) % 256) as u8,
                    ((x * 47 + y * 13 + 71) % 256) as u8,
                    ((x * 47 + y * 13 + 142) % 256) as u8,
                    255,
                ]
            })
        })
        .collect();
    let mut admission = Admission::with_residency(PIXELS_MAX, residency);
    let document = paintcraft_document::import_image_with_admission_checked(
        4,
        3,
        &rgba,
        "JXR source",
        &mut admission,
        &|| Ok(()),
    )
    .unwrap();
    PreparedImport {
        document,
        input: path.to_owned(),
        identity: SourceIdentity::existing_destination(path).unwrap().unwrap(),
        length: source.len() as u64,
        sha256: Sha256::digest(source).into(),
        owner,
        profile: PreparedProfile::Jxr(assessment),
        _lease: ResourceLease::new(preparation.reserve(HEAP_BYTES).unwrap()),
    }
}
#[test]
fn jxr_consent_metadata_and_resident_adoption_are_atomic_and_never_bypassable() {
    let source = include_bytes!("../../../../tests/jxr-fixtures/rgb8-dpi96x144.jxr");
    let root = std::env::temp_dir().join(format!("paintcraft-jxr-adoption-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("source.jxr");
    std::fs::write(&input, source).unwrap();
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let resident = SharedBudget::new(1 << 20);
    let preparation = SharedBudget::new(HEAP_BYTES);
    let current = Session::new(
        paintcraft_document::new_document(1, 1, None, 0, paintcraft_document::POLICY).unwrap(),
    );
    let ticket = current.ticket();
    let make = || prepared_jxr(resident.clone(), preparation.clone(), &input, owner);
    let pending = make();
    assert_eq!(pending.losses(), ["jxr-resolution-not-retained"]);
    assert_eq!(pending.decoder_profile(), NATIVE_JXR_IMPORT_VARIANT);
    assert_eq!(pending.document().canvas, (4, 3));
    assert_eq!(
        pending.adopt(&|| Ok(()), &|| Ok(owner)).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(resident.usage().reserved, 0);
    assert_eq!(preparation.usage().reserved, 0);
    assert_eq!(current.ticket(), ticket);
    assert_eq!(
        make()
            .adopt_with_losses(&["unknown".into()], &|| Ok(()), &|| Ok(owner))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let allowed = vec!["jxr-resolution-not-retained".into()];
    let mut foreign = owner;
    foreign.epoch += 1;
    assert_eq!(
        make()
            .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(foreign))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(
        make()
            .adopt_with_losses(
                &allowed,
                &|| Err(Error::new(ErrorCode::OperationCancelled, "cancel")),
                &|| Ok(owner)
            )
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(current.ticket(), ticket);
    assert_eq!(resident.usage().reserved, 0);
    assert_eq!(preparation.usage().reserved, 0);
    let adopted = make()
        .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(owner))
        .unwrap();
    let metadata = adopted.jxr_assessment.as_ref().unwrap();
    assert_eq!(metadata.resolution_x().raw_bits, Some(96f32.to_bits()));
    assert_eq!(metadata.resolution_y().raw_bits, Some(144f32.to_bits()));
    assert!(!metadata.resolution_y().inferred_96);
    assert_eq!(preparation.usage().reserved, 0);
    assert!(resident.usage().reserved > 0);
    assert_eq!(
        adopted.document.layers[&adopted.document.roots[0]]
            .raster
            .get(1, 0)
            .0,
        [47, 118, 189, 255]
    );
    drop(adopted);
    assert_eq!(resident.usage().reserved, 0);
    assert_eq!(std::fs::read(&input).unwrap(), source);
    std::fs::remove_dir_all(root).unwrap();
}
