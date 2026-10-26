//! Internal codec qualification never silently becomes image Open or Save.
use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};
#[test]
fn icns_remains_unavailable_to_frontends_until_selected_entry_consumers_are_qualified() {
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
                Operation::ExportFlat,
                Operation::SavePreserving,
                Operation::SaveAsCopy,
            ] {
                assert!(
                    !registry::admission(Request::new(frontend, operation, Family::Icns, platform))
                        .available()
                );
            }
        }
    }
}

#[test]
fn only_explicit_studio_entry_conversion_and_fresh_typed_set_are_admitted() {
    let bytes = include_bytes!("icns-fixtures/modern.icns");
    assert_eq!(registry::sniff(bytes), Some(Family::Icns));
    for platform in [Platform::Native, Platform::Web] {
        for (operation, variant) in [
            (Operation::ImportConvert, "icon-selected-entry"),
            (Operation::SaveAsCopy, "icon-fresh-entry-set"),
        ] {
            for frontend in [
                Frontend::Studio,
                Frontend::ClassicV7,
                Frontend::ClassicV10,
                Frontend::ClassicV11,
            ] {
                let mut request = Request::new(frontend, operation, Family::Icns, platform);
                request.extension = Some("icns");
                request.variant = Some(variant);
                assert_eq!(
                    registry::admission(request).available(),
                    frontend == Frontend::Studio
                );
                request.variant = Some("guess-largest");
                assert!(!registry::admission(request).available());
                request.variant = Some(variant);
                request.extension = Some("ico");
                assert!(!registry::admission(request).available());
            }
        }
        assert_eq!(
            registry::admit_import_entry(Frontend::Studio, "source.icns", bytes, platform, 6)
                .unwrap(),
            Family::Icns
        );
        assert!(
            registry::admit_import_entry(
                Frontend::Studio,
                "source.icns",
                bytes,
                platform,
                u16::MAX
            )
            .is_err()
        );
        assert!(
            registry::admit_import_entry(
                Frontend::Studio,
                "source.icns",
                include_bytes!("icns-fixtures/mixed-jp2.icns"),
                platform,
                1
            )
            .is_err()
        );
        assert!(
            registry::admit_import_entry(Frontend::Studio, "source.ico", bytes, platform, 0)
                .is_err()
        );
    }
}
