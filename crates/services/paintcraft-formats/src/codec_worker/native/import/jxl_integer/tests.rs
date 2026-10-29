use super::*;
use paintcraft_core::types::Rgba;

fn with_prepared(depth: u8, run: impl FnOnce(PreparedImport, Owner, &Path)) {
    let root = std::env::temp_dir().join(format!("paintcraft-jxl-consent-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("source.jxl");
    std::fs::write(&input, b"source").unwrap();
    let resident = SharedBudget::new(1 << 20);
    let preparation = SharedBudget::new(HEAP_BYTES);
    let source_sha256 = Sha256::digest(b"source").into();
    let rgba = [17, 31, 99, 0];
    let report = Report {
        source_depth: depth,
        width: 1,
        height: 1,
        source_sha256,
        output_sha256: Sha256::digest(rgba).into(),
        changed_rgb_components: u32::from(depth == 16),
        changed_alpha: 0,
    };
    let mut admission = Admission::with_residency(1 << 20, resident.clone());
    let document = paintcraft_document::import_image_with_admission_checked(
        1,
        1,
        &rgba,
        &hex(&source_sha256),
        &mut admission,
        &|| Ok(()),
    )
    .unwrap();
    drop(admission);
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 3,
        frontend: 3,
    };
    let pending = PreparedImport {
        document,
        input: input.clone(),
        identity: SourceIdentity::existing_destination(&input)
            .unwrap()
            .unwrap(),
        length: 6,
        sha256: source_sha256,
        owner,
        profile: PreparedProfile::JxlInteger(report),
        _lease: ResourceLease::new(preparation.reserve(HEAP_BYTES).unwrap()),
    };
    run(pending, owner, &input);
    assert_eq!(
        resident.usage().reserved,
        0,
        "all private document owners released"
    );
    assert_eq!(
        preparation.usage().reserved,
        0,
        "no preparation charge escaped"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn integer_adoption_requires_exact_report_digest_and_actual_loss_consent() {
    for case in 0..4 {
        with_prepared(16, |pending, owner, _| {
            let mut digest = pending.jxl_integer_assessment_digest().unwrap();
            let allowed = match case {
                0 => vec![],
                1 => {
                    digest[0] ^= 1;
                    vec![crate::jxl::DEPTH16_LOSS.to_owned()]
                }
                2 => vec!["unknown-loss".to_owned()],
                _ => vec![crate::jxl::DEPTH16_LOSS.to_owned()],
            };
            let result = pending.adopt_jxl_integer(digest, &allowed, &|| Ok(()), &|| Ok(owner));
            match case {
                0 => assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnsupported),
                1 | 2 => assert_eq!(result.unwrap_err().code, ErrorCode::InputSchema),
                _ => {
                    let adopted = result.unwrap();
                    assert_eq!(adopted.jxl_integer_assessment.unwrap().source_depth, 16);
                    let layer = &adopted.document.layers[&adopted.document.roots[0]];
                    assert_eq!(layer.raster.get(0, 0), Rgba([17, 31, 99, 0]));
                }
            }
        });
    }
}

#[test]
fn generic_adoption_cannot_bypass_integer_assessment_binding() {
    with_prepared(16, |pending, owner, _| {
        let allowed = [crate::jxl::DEPTH16_LOSS.to_owned()];
        assert_eq!(
            pending
                .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(owner))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    });
    with_prepared(8, |pending, owner, _| {
        assert!(pending.losses().is_empty());
        let digest = pending.jxl_integer_assessment_digest().unwrap();
        let adopted = pending
            .adopt_jxl_integer(digest, &[], &|| Ok(()), &|| Ok(owner))
            .unwrap();
        assert_eq!(adopted.jxl_integer_assessment.unwrap().source_depth, 8);
    });
}

#[test]
fn integer_assessment_hash_binds_statistics_and_output_not_only_source() {
    with_prepared(16, |pending, owner, _| {
        let mut changed = *pending.jxl_integer_assessment().unwrap();
        changed.changed_rgb_components += 1;
        let changed_digest = Sha256::digest(changed.encode().unwrap()).into();
        assert_ne!(
            pending.jxl_integer_assessment_digest().unwrap(),
            changed_digest
        );
        assert_eq!(
            pending
                .adopt_jxl_integer(
                    changed_digest,
                    &[crate::jxl::DEPTH16_LOSS.to_owned()],
                    &|| Ok(()),
                    &|| Ok(owner)
                )
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    });
}

#[test]
fn source_owner_and_cancellation_refusals_retire_all_private_resources() {
    for case in 0..3 {
        with_prepared(16, |pending, owner, input| {
            let digest = pending.jxl_integer_assessment_digest().unwrap();
            let mut current = owner;
            if case == 0 {
                current.epoch += 1;
            }
            if case == 1 {
                std::fs::write(input, b"mutate").unwrap();
            }
            let result = pending.adopt_jxl_integer(
                digest,
                &[crate::jxl::DEPTH16_LOSS.to_owned()],
                &|| {
                    if case == 2 {
                        Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                    } else {
                        Ok(())
                    }
                },
                &|| Ok(current),
            );
            let error = result.unwrap_err();
            assert_eq!(
                error.code,
                if case == 2 {
                    ErrorCode::OperationCancelled
                } else {
                    ErrorCode::ConflictStaleBase
                }
            );
            if case != 1 {
                assert_eq!(std::fs::read(input).unwrap(), b"source");
            }
        });
    }
}
