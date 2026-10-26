//! Prelaunch admission uses the same captured source and sealed selected policy.
use super::*;
use std::{cell::Cell, fs};
#[test]
fn family_admission_precedes_discovery_and_preserves_source_owner_cancel_guards() {
    let bytes = include_bytes!("../../../../tests/avif-sdr-fixtures/named-both.avif");
    let root = std::env::temp_dir().join(format!("paintcraft-family-admission-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let path = root.join("source.avif");
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    for case in 0..5 {
        fs::write(&path, bytes).unwrap();
        let preparation = SharedBudget::new(HEAP_BYTES);
        let residency = SharedBudget::new(1 << 20);
        let epoch = Cell::new(0);
        let cancel = Cell::new(false);
        let admitted = Cell::new(false);
        let job = Job {
            input: path.clone(),
            identity: SourceIdentity::from_file(fs::File::open(&path).unwrap()).unwrap(),
            length: bytes.len() as u64,
            codec: Codec::AvifSrgb8,
            owner,
            pixels: PIXELS_MAX,
            deadline: Duration::from_secs(10),
            application_version: env!("CARGO_PKG_VERSION").into(),
            expected_sha256: Some(if case == 0 {
                "0".repeat(64)
            } else {
                hex(&Sha256::digest(bytes))
            }),
        };
        let error = job
            .prepare_avif_family(
                residency.clone(),
                ResourceLease::new(preparation.reserve(HEAP_BYTES).unwrap()),
                &|| {
                    if cancel.get() {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "cancel after admission",
                        ))
                    } else {
                        Ok(())
                    }
                },
                &|| {
                    Ok(Owner {
                        epoch: epoch.get(),
                        ..owner
                    })
                },
                |codec, assessment| {
                    admitted.set(true);
                    assert_eq!(codec, Codec::AvifSdr8);
                    let crate::avif::FamilyAssessment::Sdr(assessment) = assessment else {
                        panic!("not validated SDR")
                    };
                    assert_eq!(assessment.sha256(), Sha256::digest(bytes).as_slice());
                    assert_eq!(assessment.primary_name(), "Color image");
                    assert_eq!(
                        assessment.losses(),
                        ["avif-color-reconstruction", "avif-item-names-not-retained"]
                    );
                    match case {
                        1 => {
                            return Err(Error::new(
                                ErrorCode::CapabilityUnsupported,
                                "frontend rejects selected SDR variant",
                            ));
                        }
                        2 => fs::write(&path, b"changed in admission").unwrap(),
                        3 => epoch.set(1),
                        4 => cancel.set(true),
                        _ => panic!("expected SHA must be checked before selection"),
                    }
                    Ok(())
                },
                |_, _| panic!("refused family must never discover/launch another helper"),
            )
            .unwrap_err();
        assert_eq!(admitted.get(), case != 0);
        assert_eq!(
            error.code,
            match case {
                1 => ErrorCode::CapabilityUnsupported,
                4 => ErrorCode::OperationCancelled,
                _ => ErrorCode::ConflictStaleBase,
            }
        );
        if case == 1 {
            assert_eq!(error.message, "frontend rejects selected SDR variant");
        }
        assert_eq!(preparation.usage().reserved, 0);
        assert_eq!(residency.usage().reserved, 0);
    }
    fs::remove_dir_all(root).unwrap();
}
