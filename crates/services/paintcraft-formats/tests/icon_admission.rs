use paintcraft_formats::registry::*;

#[test]
fn selected_entry_import_is_studio_only_and_never_open_or_preserving_save() {
    for (bytes, suffix, family) in [
        (
            &include_bytes!("icon-fixtures/mixed.ico")[..],
            "ico",
            Family::Ico,
        ),
        (
            &include_bytes!("icon-fixtures/mixed.cur")[..],
            "cur",
            Family::Cur,
        ),
    ] {
        for platform in [Platform::Native, Platform::Web] {
            let filename = format!("image.{suffix}");
            assert_eq!(sniff(bytes), Some(family));
            assert!(admit_open(Frontend::Studio, &filename, bytes, platform).is_err());
            assert!(admit_import(Frontend::Studio, &filename, bytes, platform).is_err());
            for entry in 0..6 {
                assert_eq!(
                    admit_import_entry(Frontend::Studio, &filename, bytes, platform, entry)
                        .unwrap(),
                    family
                );
            }
            assert!(admit_import_entry(Frontend::Studio, &filename, bytes, platform, 6).is_err());
            for frontend in [
                Frontend::ClassicV7,
                Frontend::ClassicV10,
                Frontend::ClassicV11,
            ] {
                assert!(admit_import_entry(frontend, &filename, bytes, platform, 0).is_err());
            }
            for operation in [
                Operation::Open,
                Operation::ExportFlat,
                Operation::SavePreserving,
                Operation::SaveAsCopy,
            ] {
                assert!(
                    !admission(Request::new(Frontend::Studio, operation, family, platform))
                        .available()
                );
            }
            assert!(
                !enabled_extensions(Frontend::Studio, Operation::ImportConvert, platform)
                    .contains(&suffix)
            );
            assert!(
                admit_import_entry(Frontend::Studio, "disguised.png", bytes, platform, 0).is_err()
            );
            assert!(admit_import_page(Frontend::Studio, &filename, bytes, platform, 0).is_err());
        }
    }
}

#[test]
fn fresh_entry_set_export_requires_its_own_variant_and_studio_frontend() {
    for platform in [Platform::Native, Platform::Web] {
        for family in [Family::Ico, Family::Cur] {
            for frontend in [
                Frontend::ClassicV7,
                Frontend::ClassicV10,
                Frontend::ClassicV11,
                Frontend::Studio,
            ] {
                let mut request = Request::new(frontend, Operation::SaveAsCopy, family, platform);
                assert!(!admission(request).available());
                request.variant = Some("icon-fresh-entry-set");
                assert_eq!(admission(request).available(), frontend == Frontend::Studio);
                request.operation = Operation::SavePreserving;
                assert!(!admission(request).available());
                request.operation = Operation::ExportFlat;
                assert!(!admission(request).available());
                request.operation = Operation::ImportConvert;
                assert!(!admission(request).available());
            }
        }
    }
}
