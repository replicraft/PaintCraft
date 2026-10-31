//! Exact advanced browser DDS admission; generic RGBA8 remains a separate route.
use paintcraft_formats::registry::{
    self, Decision, Family, Frontend, Operation, Platform, Reason, Request,
};
#[test]
fn web_texture_gate_is_exact_and_requires_preflight() {
    for variant in [
        paintcraft_formats::dds::web_export::EXPORT_VARIANT,
        paintcraft_formats::dds::web_export::v5::EXPORT_VARIANT,
        paintcraft_formats::dds::web_export::v6::EXPORT_VARIANT,
        paintcraft_formats::dds::web_export::v7::EXPORT_VARIANT,
    ] {
        for family in Family::ALL {
            for frontend in [
                Frontend::Studio,
                Frontend::ClassicV7,
                Frontend::ClassicV10,
                Frontend::ClassicV11,
            ] {
                for platform in [Platform::Web, Platform::Native] {
                    for operation in [
                        Operation::Open,
                        Operation::OpenProject,
                        Operation::OpenRaster,
                        Operation::ImportConvert,
                        Operation::ImportAsset,
                        Operation::SavePreserving,
                        Operation::SaveAsCopy,
                        Operation::ExportFlat,
                        Operation::ExportAsset,
                    ] {
                        for extension in [None, Some("dds"), Some("DDS"), Some(".dds"), Some("png")]
                        {
                            let mut request = Request::new(frontend, operation, *family, platform);
                            request.variant = Some(variant);
                            request.extension = extension;
                            let expected = *family == Family::Dds
                                && frontend == Frontend::Studio
                                && platform == Platform::Web
                                && operation == Operation::ExportFlat
                                && matches!(extension, Some("dds" | "DDS" | ".dds"));
                            assert_eq!(
                                registry::admission(request).available(),
                                expected,
                                "{request:?}"
                            );
                            if expected {
                                assert_eq!(
                                    registry::admission(request),
                                    Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
                                );
                                request.backend_available = false;
                                assert_eq!(
                                    registry::admission(request),
                                    Decision::TemporarilyUnavailable(Reason::BackendUnavailable)
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn unrelated_variants_refuse_while_absent_options_keep_the_legacy_route() {
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ExportFlat,
        Family::Dds,
        Platform::Web,
    );
    request.extension = Some("dds");
    for variant in [
        "native-dds-texture-v1",
        "web-dds-texture-worker-v1",
        "web-dds-texture-worker-v2",
        "web-dds-texture-worker-v8",
        "web-msp2-row-rle-worker-v5",
        "dds-guessed",
    ] {
        request.variant = Some(variant);
        assert!(!registry::admission(request).available());
    }
    request.variant = None;
    assert_eq!(
        registry::admission(request),
        Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
    );
    assert_eq!(
        registry::capability(Family::Dds, Operation::ExportFlat, Platform::Web).backend,
        Some("paintcraft-dds-rgba8")
    );
}
