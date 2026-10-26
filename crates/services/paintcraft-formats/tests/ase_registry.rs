use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};

#[test]
fn palette_exchange_is_a_studio_asset_workflow_on_both_targets() {
    for platform in [Platform::Native, Platform::Web] {
        for operation in [Operation::ImportAsset, Operation::ExportAsset] {
            let capability = registry::capability(Family::Ase, operation, platform);
            assert_eq!(capability.backend, Some("ag-psd-ase-finite-f32"));
            assert!(capability.qualified);
            let mut request = Request::new(Frontend::Studio, operation, Family::Ase, platform);
            request.extension = Some("ASE");
            assert!(registry::admission(request).available());
            assert!(
                registry::enabled(Frontend::Studio, operation, platform).contains(&Family::Ase)
            );
            for frontend in [
                Frontend::ClassicV7,
                Frontend::ClassicV10,
                Frontend::ClassicV11,
            ] {
                request.frontend = frontend;
                assert!(!registry::admission(request).available());
            }
            request.frontend = Frontend::Studio;
            request.extension = Some("aco");
            assert!(!registry::admission(request).available());
            request.extension = Some("ase");
            request.variant = Some("nested-groups");
            assert!(!registry::admission(request).available());
            request.variant = None;
            request.backend_available = false;
            assert!(!registry::admission(request).available());
        }
    }
}

#[test]
fn ase_never_expands_document_open_conversion_or_save_inventory() {
    for operation in [
        Operation::Open,
        Operation::OpenProject,
        Operation::OpenRaster,
        Operation::ImportConvert,
        Operation::SavePreserving,
        Operation::SaveAsCopy,
        Operation::ExportFlat,
    ] {
        let request = Request::new(Frontend::Studio, operation, Family::Ase, Platform::Native);
        assert!(!registry::admission(request).available(), "{operation:?}");
        assert!(
            !registry::enabled(Frontend::Studio, operation, Platform::Native)
                .contains(&Family::Ase)
        );
    }
    let bytes = include_bytes!("ase-fixtures/mixed.ase");
    assert!(
        registry::admit_open(Frontend::Studio, "palette.ase", bytes, Platform::Native).is_err()
    );
    assert!(
        registry::admit_import(Frontend::Studio, "palette.ase", bytes, Platform::Native).is_err()
    );
}
