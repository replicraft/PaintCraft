use paintcraft_formats::{
    msp::web_export,
    registry::{self, Decision, Family, Frontend, Operation, Platform, Reason, Request},
};
#[test]
fn worker_msp_variant_is_exact_web_studio_flat_copy_with_preflight() {
    for variant in [
        paintcraft_formats::msp::web_export::EXPORT_VARIANT,
        paintcraft_formats::msp::web_export::v5::EXPORT_VARIANT,
    ] {
        for frontend in [
            Frontend::Studio,
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            for platform in [Platform::Native, Platform::Web] {
                for operation in [
                    Operation::Open,
                    Operation::OpenProject,
                    Operation::OpenRaster,
                    Operation::ImportConvert,
                    Operation::SavePreserving,
                    Operation::SaveAsCopy,
                    Operation::ExportFlat,
                    Operation::ImportAsset,
                    Operation::ExportAsset,
                ] {
                    for extension in [None, Some("msp"), Some("MSP"), Some(".msp"), Some("png")] {
                        let mut r = Request::new(frontend, operation, Family::Msp, platform);
                        r.extension = extension;
                        r.variant = Some(variant);
                        let expected = frontend == Frontend::Studio
                            && platform == Platform::Web
                            && operation == Operation::ExportFlat
                            && matches!(extension, Some("msp" | "MSP" | ".msp"));
                        let decision = registry::admission(r);
                        assert_eq!(decision.available(), expected, "{r:?}: {decision:?}");
                        if expected {
                            assert_eq!(
                                decision,
                                Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
                            );
                            assert!(decision.require_allowed(r).is_err());
                            r.backend_available = false;
                            assert_eq!(
                                registry::admission(r),
                                Decision::TemporarilyUnavailable(Reason::BackendUnavailable)
                            );
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn generic_msp1_keeps_its_old_route_and_misspelled_worker_variants_are_denied() {
    let mut r = Request::new(
        Frontend::Studio,
        Operation::ExportFlat,
        Family::Msp,
        Platform::Web,
    );
    assert!(registry::admission(r).available());
    r.extension = Some("msp");
    r.family = Family::Png;
    r.variant = Some(web_export::EXPORT_VARIANT);
    assert!(!registry::admission(r).available());
    r.family = Family::Msp;
    for variant in [
        "web-msp2-row-rle-worker-v0",
        "web-msp2-row-rle-worker-v1",
        "web-msp2-row-rle-worker-v2",
        "web-msp2-row-rle-worker-v6",
        "web-dds-texture-worker-v5",
        "native-msp2-row-rle-v1",
        "web-dds-texture-worker-v1",
        "web-dds-texture-worker-v2",
    ] {
        r.variant = Some(variant);
        assert!(!registry::admission(r).available());
    }
}
