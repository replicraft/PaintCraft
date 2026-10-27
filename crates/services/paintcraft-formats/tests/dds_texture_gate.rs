//! Advanced DDS export has one explicit route, independent of generic RGBA8.
use paintcraft_formats::{
    dds,
    registry::{self, Decision, Family, Frontend, Operation, Platform, Reason, Request},
};
#[test]
fn texture_variant_is_only_native_studio_dds_flat_export_and_requires_preflight() {
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
                for extension in [None, Some("dds"), Some("DDS"), Some(".dds"), Some("png")] {
                    let mut request = Request::new(frontend, operation, Family::Dds, platform);
                    request.variant = Some(dds::NATIVE_TEXTURE_EXPORT_VARIANT);
                    request.extension = extension;
                    let expected = frontend == Frontend::Studio
                        && platform == Platform::Native
                        && operation == Operation::ExportFlat
                        && matches!(extension, Some("dds" | "DDS" | ".dds"));
                    let decision = registry::admission(request);
                    assert_eq!(decision.available(), expected, "{request:?}: {decision:?}");
                    if expected {
                        assert_eq!(
                            decision,
                            Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
                        );
                        assert!(decision.require_allowed(request).is_err());
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
#[test]
fn cross_family_and_unknown_variants_refuse_without_widening_generic_dds() {
    for family in Family::ALL {
        let mut request = Request::new(
            Frontend::Studio,
            Operation::ExportFlat,
            *family,
            Platform::Native,
        );
        request.variant = Some(dds::NATIVE_TEXTURE_EXPORT_VARIANT);
        request.extension = family.extensions().first().copied();
        assert_eq!(
            registry::admission(request).available(),
            *family == Family::Dds,
            "{request:?}"
        );
    }
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ExportFlat,
        Family::Dds,
        Platform::Native,
    );
    request.extension = Some("dds");
    request.variant = Some("dds-guessed-compression");
    assert!(!registry::admission(request).available());
    for platform in [Platform::Native, Platform::Web] {
        request.platform = platform;
        request.variant = None;
        assert_eq!(
            registry::admission(request),
            Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
        );
        assert_eq!(
            registry::capability(Family::Dds, Operation::ExportFlat, platform).backend,
            Some("paintcraft-dds-rgba8")
        );
    }
}
