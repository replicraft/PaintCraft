//! The existing recovery journal carries canonical PCND5 without a second codec.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
#[test]
fn pcnd5_source_mask_and_disabled_zero_box_recovery_roundtrip() {
    for name in ["box-disabled-r0.pcnd", "source-box-mask.pcnd"] {
        let original = fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/pcnd-spatial-fixtures")
                .join(name),
        )
        .unwrap();
        let profile = SharedBudget::new(2 << 20);
        let coverage = SharedBudget::new(2 << 20);
        let doc = crate::pcnd::read_with_admission_checked(
            &original,
            2,
            65536,
            1,
            crate::pcnd::ReadAdmission {
                metadata: crate::pcnd::MetadataAdmission {
                    bytes: 2 << 20,
                    budget: &profile,
                },
                coverage: crate::pcnd::CoverageAdmission {
                    bytes: 2 << 20,
                    budget: &coverage,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap();
        let dir = scratch();
        let owner = Recovery::start(&dir).unwrap();
        owner.checkpoint_document(&doc, None).unwrap();
        drop(owner);
        let found = only(&dir);
        assert_eq!(
            crate::pcnd::probe(&fs::read(&found.checkpoint).unwrap())
                .unwrap()
                .container_version,
            5
        );
        let restored = Recovery::read_checkpoint_with_metadata_admission_checked(
            &found.checkpoint,
            crate::pcnd::MetadataAdmission {
                bytes: 2 << 20,
                budget: &profile,
            },
            &|| Ok(()),
        )
        .unwrap();
        assert!(doc.same_semantics(&restored));
        assert_eq!(crate::pcnd::write_compact(&restored).unwrap(), original);
        drop(restored);
        drop(doc);
        assert_eq!(profile.usage().reserved, 0);
        assert_eq!(coverage.usage().reserved, 0);
        Recovery::discard(&dir, found.session).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
