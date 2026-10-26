//! Exact native conversion gate never expands generic Open or browser reading.
use paintcraft_formats::{
    codec_worker as wire,
    registry::{self, Decision, Family, Frontend, Operation, Platform, Reason, Request},
};
#[test]
fn avif_native_studio_conversion_is_variant_bound_and_export_stays_independent() {
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
                Operation::ExportAsset,
            ] {
                for variant in [
                    None,
                    Some(wire::NATIVE_AVIF_IMPORT_VARIANT),
                    Some(wire::NATIVE_AVIF_SDR_IMPORT_VARIANT),
                    Some(wire::NATIVE_HEIF_IMPORT_VARIANT),
                    Some("guess-rgb"),
                ] {
                    let mut request = Request::new(frontend, operation, Family::Avif, platform);
                    request.variant = variant;
                    request.extension = Some("avif");
                    let import = frontend == Frontend::Studio
                        && platform == Platform::Native
                        && operation == Operation::ImportConvert
                        && matches!(
                            variant,
                            Some(wire::NATIVE_AVIF_IMPORT_VARIANT)
                                | Some(wire::NATIVE_AVIF_SDR_IMPORT_VARIANT)
                        );
                    let export = frontend == Frontend::Studio
                        && operation == Operation::ExportFlat
                        && variant.is_none();
                    let decision = registry::admission(request);
                    assert_eq!(decision.available(), import || export, "{request:?}");
                    if import {
                        assert_eq!(
                            decision,
                            Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent)
                        );
                        assert!(decision.require_allowed(request).is_err());
                    }
                }
            }
        }
    }
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ImportConvert,
        Family::Avif,
        Platform::Native,
    );
    request.variant = Some(wire::NATIVE_AVIF_IMPORT_VARIANT);
    request.extension = Some("png");
    assert!(!registry::admission(request).available());
    request.extension = Some("avif");
    request.backend_available = false;
    assert!(!registry::admission(request).available());
    let source = include_bytes!("avif-fixtures/alpha.avif");
    for platform in [Platform::Native, Platform::Web] {
        assert!(
            !registry::enabled(Frontend::Studio, Operation::Open, platform).contains(&Family::Avif)
        );
        assert!(
            !registry::enabled(Frontend::Studio, Operation::ImportConvert, platform)
                .contains(&Family::Avif)
        );
        assert!(registry::admit_open(Frontend::Studio, "source.avif", source, platform).is_err());
        assert!(registry::admit_import(Frontend::Studio, "source.avif", source, platform).is_err());
    }
}
