use paintcraft_formats::{
    ora,
    registry::{self, Family, Frontend, Operation, Platform, Request},
};
const SOURCE: &[u8] = include_bytes!("ora-group-fixtures/neutral.ora");
#[test]
fn neutral_group_copy_is_only_the_explicit_studio_import_variant() {
    for platform in [Platform::Native, Platform::Web] {
        for frontend in [
            Frontend::Studio,
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            for operation in [
                Operation::Open,
                Operation::ImportConvert,
                Operation::SavePreserving,
                Operation::SaveAsCopy,
                Operation::ExportFlat,
            ] {
                let mut request = Request::new(frontend, operation, Family::Ora, platform);
                request.variant = Some(ora::NEUTRAL_GROUP_IMPORT_VARIANT);
                assert_eq!(
                    registry::admission(request).available(),
                    frontend == Frontend::Studio && operation == Operation::ImportConvert,
                    "{request:?}"
                );
            }
        }
        assert_eq!(
            registry::admit_import_ora(Frontend::Studio, "layers.ora", SOURCE, platform).unwrap(),
            Family::Ora
        );
        assert_eq!(
            registry::admit_import_ora(Frontend::Studio, "extensionless", SOURCE, platform)
                .unwrap(),
            Family::Ora
        );
        assert!(
            registry::admit_import_ora(Frontend::Studio, "wrong.png", SOURCE, platform).is_err()
        );
        assert!(
            registry::admit_import_ora(
                Frontend::Studio,
                "unrelated",
                b"PK\x03\x04not OpenRaster",
                platform
            )
            .is_err()
        );
        assert!(registry::admit_open(Frontend::Studio, "layers.ora", SOURCE, platform).is_err());
    }
}
